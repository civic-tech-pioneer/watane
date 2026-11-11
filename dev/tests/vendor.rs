use std::{fs, path::Path, process::Command};

#[test]
fn ac3_vendor_script_writes_only_to_callers_output() {
    let root = tempfile::tempdir().unwrap();
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    // Probe the registry cache without permitting a download.
    for file in ["Cargo.toml", "Cargo.lock", "lib.rs"] {
        fs::copy(
            manifest.join("allowlist").join(file),
            root.path().join(file),
        )
        .unwrap();
    }
    let available = Command::new("cargo")
        .args([
            "metadata",
            "--offline",
            "--locked",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(root.path().join("Cargo.toml"))
        .output()
        .unwrap();
    if !available.status.success() {
        eprintln!(
            "registry cache unavailable; run ignored ac3_vendor_script_with_registry_downloads"
        );
        return;
    }
    let listing = |dir: &Path| {
        let mut names: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        names.sort();
        names
    };
    let (dev, allowlist) = (listing(manifest), listing(&manifest.join("allowlist")));
    run_script(root.path(), true);
    // Nothing new in the checkout: no vendor/, target/ or lock beside the allowlist.
    assert_eq!(listing(manifest), dev);
    assert_eq!(listing(&manifest.join("allowlist")), allowlist);
}

#[test]
#[ignore = "slow: vendor bootstrap may download crates when the registry cache is absent"]
fn ac3_vendor_script_with_registry_downloads() {
    let root = tempfile::tempdir().unwrap();
    run_script(root.path(), false);
}

fn run_script(root: &Path, offline: bool) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(manifest.join("scripts/vendor.sh"))
        .current_dir(root)
        .env("CARGO_NET_OFFLINE", if offline { "true" } else { "false" })
        .arg("own vendor")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Independent oracle: registry packages and checksums from the committed lock.
    let lock: toml::Table =
        toml::from_str(&fs::read_to_string(manifest.join("allowlist/Cargo.lock")).unwrap())
            .unwrap();
    let vendor = root.join("own vendor");
    let mut expected = vec![];
    for package in lock["package"].as_array().unwrap() {
        let Some(checksum) = package.get("checksum") else {
            continue;
        };
        let name = format!(
            "{}-{}",
            package["name"].as_str().unwrap(),
            package["version"].as_str().unwrap()
        );
        let value: serde_json::Value = serde_json::from_slice(
            &fs::read(vendor.join(&name).join(".cargo-checksum.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(value["package"].as_str(), checksum.as_str());
        expected.push(name);
    }
    expected.sort();
    let mut actual: Vec<_> = fs::read_dir(vendor)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    actual.sort();
    assert_eq!(actual, expected);
}
