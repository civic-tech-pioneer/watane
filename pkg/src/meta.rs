use crate::{
    HttpHost, ModError, ReleaseRef, Rule,
    folder::{invalid, read_file},
};
use serde::Deserialize;
use std::{collections::BTreeSet, path::Path};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Meta {
    pub name: String,
    pub summary: String,
    #[serde(default, rename = "http")]
    http: Option<Http>,
    #[serde(default, rename = "origin")]
    origin_table: Option<Origin>,
    #[serde(skip)]
    pub http_hosts: Vec<HttpHost>,
    #[serde(skip)]
    pub origin: Option<ReleaseRef>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Http {
    hosts: Vec<HttpHost>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Origin {
    release: ReleaseRef,
}

pub(crate) fn read(dir: &Path) -> Result<Meta, ModError> {
    let bytes = read_file(dir, "mod.toml", 16 * 1024, Rule::ModToml, false)?.unwrap_or_default();
    let bad = |message: String| invalid(Rule::ModToml, "mod.toml", message);
    let text = std::str::from_utf8(&bytes).map_err(|_| bad("use UTF-8 TOML".into()))?;
    let table: toml::Table = toml::from_str(text).map_err(|e| bad(e.to_string()))?;
    for key in ["slug", "version"] {
        if table.contains_key(key) {
            return Err(bad(format!(
                "`{key}` is not a mod.toml key: the slug and the version come from the package line of wit/world.wit"
            )));
        }
    }
    let mut meta: Meta = table
        .try_into()
        .map_err(|e: toml::de::Error| bad(e.to_string()))?;
    for (field, value, max) in [("name", &meta.name, 64), ("summary", &meta.summary, 280)] {
        if value.is_empty()
            || value.chars().count() > max
            || value.trim() != value
            || value.chars().any(char::is_control)
        {
            return Err(bad(format!(
                "{field} must have 1..={max} characters, no control characters or surrounding whitespace"
            )));
        }
    }
    if let Some(http) = meta.http.take() {
        let hosts: BTreeSet<_> = http.hosts.iter().cloned().collect();
        if http.hosts.is_empty() || http.hosts.len() > 16 || hosts.len() != http.hosts.len() {
            return Err(bad("http.hosts must have 1..=16 distinct hosts".into()));
        }
        meta.http_hosts = hosts.into_iter().collect();
    }
    meta.origin = meta.origin_table.take().map(|o| o.release);
    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModError, Rule};
    use std::fs;

    #[test]
    fn ac2_mod_toml() {
        let dir = tempfile::tempdir().unwrap();
        let good = "name = \"Reading list\"\nsummary = \"Keep a list of books.\"\n";
        fs::write(dir.path().join("mod.toml"), good).unwrap();
        let meta = read(dir.path()).unwrap();
        assert_eq!(meta.name, "Reading list");
        assert_eq!(meta.summary, "Keep a list of books.");
        assert!(meta.http_hosts.is_empty());
        assert!(meta.origin.is_none());
        for bad in [
            "",
            "name = 'Hello'",
            "name = ' Hello'\nsummary = 'S'",
            "name = 'Hello'\nsummary = 'S '",
            "name = ''\nsummary = 'S'",
            "name = 'Hello'\nsummary = ''",
            "name = 1\nsummary = 'S'",
            "name = 'Hello'\nsummary = 'S'\nunknown = true",
            "name = 'Hello'\nsummary = 'S'\n[extra]\nx = 1",
            "name = 'Hello'\nsummary = 'S'\n[http]\nhosts = []",
            "name = 'Hello'\nsummary = 'S'\n[http]\nhosts = ['a', 'a']",
            "name = 'Hello'\nsummary = 'S'\n[http]\nhosts = ['A']",
            "name = 'Hello'\nsummary = 'S'\n[http]\nhosts = ['a']\nextra = 1",
            "name = 'Hello'\nsummary = 'S'\n[origin]\nrelease = 'bad'",
            "name = \"a\\n\"\nsummary = 'S'",
        ] {
            fs::write(dir.path().join("mod.toml"), bad).unwrap();
            assert!(
                matches!(read(dir.path()), Err(ModError::Invalid(v)) if v.iter().any(|v| v.rule == Rule::ModToml && v.file == "mod.toml")),
                "{bad}"
            );
        }
        for key in ["slug", "version"] {
            fs::write(
                dir.path().join("mod.toml"),
                format!("{good}{key} = 'hello'\n"),
            )
            .unwrap();
            let err = read(dir.path()).unwrap_err().to_string();
            assert!(err.contains(&format!("`{key}` is not a mod.toml key: the slug and the version come from the package line of wit/world.wit")), "{err}");
        }
        for (name, summary, accepted) in [
            ("é".repeat(64), "S".repeat(280), true),
            ("é".repeat(65), "S".into(), false),
            ("Hello".into(), "S".repeat(281), false),
        ] {
            fs::write(
                dir.path().join("mod.toml"),
                format!("name = '{name}'\nsummary = '{summary}'"),
            )
            .unwrap();
            assert_eq!(read(dir.path()).is_ok(), accepted);
        }
        for bad in [vec![b' '; 16385], vec![0xff]] {
            fs::write(dir.path().join("mod.toml"), bad).unwrap();
            assert!(matches!(read(dir.path()), Err(ModError::Invalid(_))));
        }
        fs::remove_file(dir.path().join("mod.toml")).unwrap();
        assert!(matches!(read(dir.path()), Err(ModError::Invalid(_))));
    }
}
