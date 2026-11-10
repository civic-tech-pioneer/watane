use crate::{
    ModError, PublisherKey, Rule, Slug,
    folder::{invalid, io, read_file},
};
use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use std::{collections::BTreeMap, fmt, fs, path::Path};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Deps(BTreeMap<Slug, Option<PublisherKey>>);

impl<'de> Deserialize<'de> for Deps {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Members;
        impl<'de> Visitor<'de> for Members {
            type Value = Deps;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object of at most 32 mods:<slug> providers")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Deps, M::Error> {
                let mut deps = Deps::default();
                while let Some(key) = map.next_key::<String>()? {
                    if deps.0.len() >= 32 {
                        return Err(serde::de::Error::custom("at most 32 providers"));
                    }
                    let slug: Slug = key
                        .strip_prefix("mods:")
                        .ok_or_else(|| {
                            serde::de::Error::custom("provider key must be mods:<slug>")
                        })?
                        .parse()
                        .map_err(serde::de::Error::custom)?;
                    let publisher = map.next_value::<Option<PublisherKey>>()?;
                    if deps.0.insert(slug, publisher).is_some() {
                        return Err(serde::de::Error::custom("duplicate provider key"));
                    }
                }
                Ok(deps)
            }
        }
        deserializer.deserialize_map(Members)
    }
}
impl Deps {
    pub fn read(dir: &Path) -> Result<Deps, ModError> {
        let Some(bytes) = read_file(dir, "deps.json", 64 * 1024, Rule::DepsJson, true)? else {
            return Ok(Self::default());
        };
        serde_json::from_slice(&bytes)
            .map_err(|e| invalid(Rule::DepsJson, "deps.json", e.to_string()))
    }
    pub fn write(&self, dir: &Path) -> Result<(), ModError> {
        // The setters cannot introduce a malformed key, but can exceed the file schema.
        if self.0.len() > 32 {
            return Err(invalid(Rule::DepsJson, "deps.json", "at most 32 providers"));
        }
        let map: BTreeMap<_, _> = self.iter().map(|(s, p)| (format!("mods:{s}"), p)).collect();
        let mut text = serde_json::to_string_pretty(&map)
            .map_err(|e| invalid(Rule::DepsJson, "deps.json", e.to_string()))?;
        text.push('\n');
        let temp = dir.join("deps.json.tmp");
        // A stale temporary file (or link) is removed, never followed, and never blocks the write.
        match fs::remove_file(&temp) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(io(&temp, e)),
            _ => (),
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| io(&temp, e))?;
        use std::io::Write;
        let result = file
            .write_all(text.as_bytes())
            .map_err(|e| io(&temp, e))
            .and_then(|()| {
                fs::rename(&temp, dir.join("deps.json")).map_err(|e| io(&dir.join("deps.json"), e))
            });
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
    pub fn get(&self, slug: &Slug) -> Option<Option<&PublisherKey>> {
        self.0.get(slug).map(Option::as_ref)
    }
    pub fn set(&mut self, slug: Slug, publisher: Option<PublisherKey>) {
        self.0.insert(slug, publisher);
    }
    pub fn remove(&mut self, slug: &Slug) -> bool {
        self.0.remove(slug).is_some()
    }
    pub fn iter(&self) -> impl Iterator<Item = (&Slug, Option<&PublisherKey>)> {
        self.0.iter().map(|(s, p)| (s, p.as_ref()))
    }
}
