use crate::outcome::{Code, Problem};
use semver::Version;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub const MANIFEST: &str = include_str!("../allowlist/Cargo.toml");
pub const LOCK: &str = include_str!("../allowlist/Cargo.lock");

pub struct Allowlist {
    direct: BTreeMap<String, Version>,
    packages: BTreeMap<(String, String), String>,
}
impl Allowlist {
    pub fn embedded() -> Self {
        let manifest: toml::Table = toml::from_str(MANIFEST).expect("embedded allowlist manifest");
        let direct = manifest["dependencies"]
            .as_table()
            .unwrap()
            .iter()
            .map(|(name, value)| {
                let version = value
                    .as_str()
                    .or_else(|| value.get("version").and_then(toml::Value::as_str))
                    .unwrap();
                (
                    name.clone(),
                    version
                        .trim_start_matches('=')
                        .parse()
                        .expect("exact allowlist pin"),
                )
            })
            .collect();
        let lock: toml::Table = toml::from_str(LOCK).expect("embedded allowlist lock");
        let packages = lock["package"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p.get("source").is_some())
            .map(|p| {
                (
                    (
                        p["name"].as_str().unwrap().into(),
                        p["version"].as_str().unwrap().into(),
                    ),
                    p["checksum"].as_str().unwrap().into(),
                )
            })
            .collect();
        Self { direct, packages }
    }
    pub fn direct(&self) -> Vec<(String, Version)> {
        self.direct
            .iter()
            .map(|(n, v)| (n.clone(), v.clone()))
            .collect()
    }
    pub fn version_of(&self, name: &str) -> Option<&Version> {
        self.direct.get(name)
    }
    pub fn contains(&self, name: &str, version: &str, checksum: &str) -> bool {
        self.packages
            .get(&(name.into(), version.into()))
            .is_some_and(|c| c == checksum)
    }
    pub fn crates(&self) -> String {
        self.direct
            .iter()
            .map(|(n, v)| format!("{n} {v}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}
fn problem(path: &Path, detail: impl std::fmt::Display) -> Problem {
    Problem::new(
        Code::EnvAllowlist,
        format!(
            "the crate allowlist at {} differs from this watane-dev: {detail}",
            path.display()
        ),
    )
    .at(path.to_string_lossy(), None, None)
    .fix("Reinstall watane-dev, or set WATANE_DEV_VENDOR")
}
pub fn vendor_dir() -> Result<PathBuf, Problem> {
    let path = if let Some(path) = std::env::var_os("WATANE_DEV_VENDOR") {
        PathBuf::from(path)
    } else {
        let installed = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("../share/watane-dev/vendor")));
        installed
            .filter(|p| p.is_dir())
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("allowlist/vendor"))
    };
    if !path.is_dir() {
        return Err(problem(&path, "vendor folder is missing"));
    }
    path.canonicalize().map_err(|e| problem(&path, e))
}
pub fn verify_vendor(vendor: &Path, allow: &Allowlist) -> Result<(), Problem> {
    let mut expected: BTreeMap<_, _> = allow
        .packages
        .iter()
        .map(|((name, version), checksum)| (format!("{name}-{version}"), checksum))
        .collect();
    for entry in fs::read_dir(vendor).map_err(|e| problem(vendor, e))? {
        let entry = entry.map_err(|e| problem(vendor, e))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(checksum) = expected.remove(&name) else {
            return Err(problem(vendor, format!("extra folder {name}")));
        };
        if !entry.file_type().map_err(|e| problem(vendor, e))?.is_dir() {
            return Err(problem(vendor, format!("{name} is not a directory")));
        }
        let path = entry.path().join(".cargo-checksum.json");
        let bytes = fs::read(&path).map_err(|e| problem(vendor, format!("{name}: {e}")))?;
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|e| problem(vendor, format!("{name}: {e}")))?;
        if value["package"].as_str() != Some(checksum.as_str()) {
            return Err(problem(vendor, format!("checksum for {name}")));
        }
    }
    if let Some(name) = expected.keys().next() {
        return Err(problem(vendor, format!("missing folder {name}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ac8_embedded_allowlist_has_exact_direct_pins() {
        let allow = Allowlist::embedded();
        assert_eq!(
            allow.crates(),
            "serde 1.0.229, serde_json 1.0.154, wit-bindgen 0.63.0"
        );
        assert!(
            allow
                .packages
                .contains_key(&("wit-bindgen".into(), "0.63.0".into()))
        );
        assert!(
            allow
                .packages
                .contains_key(&("serde".into(), "1.0.229".into()))
        );
    }
    #[test]
    fn ac8_verify_vendor_rejects_wrong_missing_and_extra_folders() {
        let allow = Allowlist {
            direct: BTreeMap::new(),
            packages: BTreeMap::from([(
                ("serde".into(), "1.0.229".into()),
                "expected-checksum".into(),
            )]),
        };
        for case in ["wrong checksum", "missing", "extra", "valid"] {
            let temp = tempfile::tempdir().unwrap();
            if case != "missing" {
                fs::create_dir(temp.path().join("serde-1.0.229")).unwrap();
                let checksum = if case == "wrong checksum" {
                    "wrong"
                } else {
                    "expected-checksum"
                };
                fs::write(
                    temp.path().join("serde-1.0.229/.cargo-checksum.json"),
                    serde_json::json!({"package": checksum, "files": {}}).to_string(),
                )
                .unwrap();
            }
            if case == "extra" {
                fs::create_dir(temp.path().join("rand-0.9.0")).unwrap();
            }
            if case == "valid" {
                verify_vendor(temp.path(), &allow).unwrap();
                continue;
            }
            let error = verify_vendor(temp.path(), &allow).expect_err(case);
            assert_eq!(error.code.as_str(), "env.allowlist");
        }
    }
}
