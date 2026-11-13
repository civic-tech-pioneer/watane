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
    #[serde(default, rename = "repository")]
    repository_raw: Option<toml::Value>,
    #[serde(skip)]
    pub repository: Option<String>,
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

pub(crate) fn text_violation(field: &str, value: &str, max: usize) -> Option<String> {
    if value.is_empty()
        || value.chars().count() > max
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        Some(format!(
            "{field} must have 1..={max} characters, no control characters or surrounding whitespace"
        ))
    } else {
        None
    }
}

fn repository_violation(value: Option<&toml::Value>) -> Option<&'static str> {
    let Some(url) = value.and_then(toml::Value::as_str) else {
        return Some("`repository` must start with https://");
    };
    if url.len() > 256 {
        return Some("`repository` is longer than 256 bytes");
    }
    let Some(rest) = url.strip_prefix("https://") else {
        return Some("`repository` must start with https://");
    };
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    if authority.contains('@') {
        return Some("`repository` must not hold a user name or password");
    }
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    if host.is_empty() {
        return Some("`repository` has no host");
    }
    let label = |l: &str| {
        (1..=63).contains(&l.len()) && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    };
    let port_ok =
        port.is_none_or(|p| (1..=5).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()));
    let path_ok = path
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"-._~/%".contains(&b));
    if host.split('.').all(label) && port_ok && path_ok {
        None
    } else {
        Some("`repository` may hold only letters, digits and - . _ ~ / % after the host")
    }
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
        if let Some(message) = text_violation(field, value, max) {
            return Err(bad(message));
        }
    }
    if meta.repository_raw.is_some() {
        if let Some(message) = repository_violation(meta.repository_raw.as_ref()) {
            return Err(bad(message.into()));
        }
        meta.repository = meta
            .repository_raw
            .take()
            .and_then(|v| v.try_into::<String>().ok());
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
    fn ac1_ac2_repository_grammar_and_messages() {
        let dir = tempfile::tempdir().unwrap();
        let write = |line: &str| {
            fs::write(
                dir.path().join("mod.toml"),
                format!("name = 'H'\nsummary = 'S'\n{line}\n"),
            )
            .unwrap();
            read(dir.path())
        };
        let long = format!("https://h/{}", "a".repeat(246));
        assert_eq!(long.len(), 256);
        for ok in [
            "https://github.com/alice/reading-list",
            "https://git.example.org:8443/a/b-c.d_e~f%20",
            "https://h",
            &long,
        ] {
            let meta = write(&format!("repository = '{ok}'")).unwrap();
            assert_eq!(meta.repository.as_deref(), Some(ok));
        }
        assert_eq!(write("").unwrap().repository, None);
        let too_long = format!("{long}a");
        let table = [
            (too_long.as_str(), "`repository` is longer than 256 bytes"),
            (
                "http://github.com/a/b",
                "`repository` must start with https://",
            ),
            (
                "https://user@github.com/a/b",
                "`repository` must not hold a user name or password",
            ),
            (
                "https://user:pw@h/x",
                "`repository` must not hold a user name or password",
            ),
            ("https://", "`repository` has no host"),
            ("https:///a", "`repository` has no host"),
            (
                "https://h/a b",
                "`repository` may hold only letters, digits and - . _ ~ / % after the host",
            ),
            (
                "https://h/a?b=c",
                "`repository` may hold only letters, digits and - . _ ~ / % after the host",
            ),
            (
                "https://h/#x",
                "`repository` may hold only letters, digits and - . _ ~ / % after the host",
            ),
            (
                "https://h:",
                "`repository` may hold only letters, digits and - . _ ~ / % after the host",
            ),
            (
                "https://h:123456",
                "`repository` may hold only letters, digits and - . _ ~ / % after the host",
            ),
            (
                "https://a..b",
                "`repository` may hold only letters, digits and - . _ ~ / % after the host",
            ),
            (
                "https://h\u{7}",
                "`repository` may hold only letters, digits and - . _ ~ / % after the host",
            ),
        ];
        for (url, message) in table {
            let line = format!("repository = \"{}\"", url.replace('\u{7}', "\\u0007"));
            expect(write(&line), message);
        }
        for value in ["1", "[\"x\"]"] {
            expect(
                write(&format!("repository = {value}")),
                "`repository` must start with https://",
            );
        }
    }
    fn expect(result: Result<Meta, ModError>, message: &str) {
        match result {
            Err(ModError::Invalid(v)) => {
                assert_eq!(v.len(), 1, "{message}");
                assert_eq!(v[0].rule, Rule::ModToml);
                assert_eq!(v[0].file, "mod.toml");
                assert_eq!(v[0].message, message);
            }
            other => panic!("{message}: {other:?}"),
        }
    }

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
