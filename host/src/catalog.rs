use crate::{HostError, LocalName, home};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use watane_pkg::{Grant, InterfaceName, Mod, PackageId, PublisherKey, ReleaseRef, Slug, Version};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstalledMod {
    pub local: LocalName,
    pub identity: String,
    pub slug: Slug,
    pub name: String,
    pub summary: String,
    pub version: Version,
    pub publisher: Option<PublisherKey>,
    pub package: PackageId,
    pub release: Option<ReleaseRef>,
    pub origin: Option<ReleaseRef>,
    pub enabled: bool,
    pub has_logic: bool,
    pub page: Option<String>,
    pub grants: Vec<GrantState>,
    pub links: Vec<LinkView>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GrantState {
    pub grant: Grant,
    pub granted: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LinkView {
    pub kind: LinkKind,
    pub interface: InterfaceName,
    pub provider: Option<LocalName>,
    pub provider_interface: Option<InterfaceName>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkKind {
    Call,
    Subscribe,
}

#[derive(Debug, Default)]
pub(crate) struct Catalog {
    generation: u64,
    pub mods: Vec<InstalledMod>,
}

#[derive(Serialize, Deserialize)]
struct DiskCatalog {
    watane_installed: u32,
    generation: u64,
    mods: Vec<Record>,
}
#[derive(Serialize, Deserialize)]
struct Record {
    local: LocalName,
    identity: String,
    package: PackageId,
    release: Option<ReleaseRef>,
    enabled: bool,
    grants: Vec<GrantState>,
    links: Vec<LinkView>,
}

impl From<&InstalledMod> for Record {
    fn from(m: &InstalledMod) -> Self {
        Self {
            local: m.local.clone(),
            identity: m.identity.clone(),
            package: m.package.clone(),
            release: m.release.clone(),
            enabled: m.enabled,
            grants: m.grants.clone(),
            links: m.links.clone(),
        }
    }
}

fn invalid(input: &Path, reason: impl ToString) -> HostError {
    HostError::InvalidMod {
        input: input.display().to_string(),
        reason: reason.to_string(),
    }
}

fn page_mod(dir: &Path) -> Result<Mod, HostError> {
    let m = Mod::read(dir).map_err(|e| invalid(dir, e))?;
    if m.logic().is_some() || !m.world().exports.is_empty() || !m.world().subscriptions.is_empty() {
        return Err(invalid(dir, "logic parts arrive in a later step"));
    }
    Ok(m)
}

fn view(m: &Mod, local: LocalName, package: PackageId) -> InstalledMod {
    InstalledMod {
        local,
        identity: format!("local/{}", m.slug()),
        slug: m.slug().clone(),
        name: m.name().into(),
        summary: m.summary().into(),
        version: m.version().clone(),
        publisher: None,
        package,
        release: None,
        origin: m.origin().cloned(),
        enabled: true,
        has_logic: false,
        // A relative marker becomes the origin URL at the transport boundary.
        page: m.page().map(|_| "/".into()),
        grants: vec![],
        links: vec![],
        error: None,
    }
}

impl Catalog {
    pub fn open(home: &Path) -> Result<Self, HostError> {
        let disk = match fs::read(home.join("installed.json")) {
            Ok(bytes) => serde_json::from_slice::<DiskCatalog>(&bytes)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => DiskCatalog {
                watane_installed: 1,
                generation: 0,
                mods: vec![],
            },
            Err(e) => return Err(e.into()),
        };
        let bad = |reason: &str| HostError::BadHome {
            path: home.into(),
            reason: reason.into(),
        };
        if disk.watane_installed != 1 {
            return Err(bad("unknown installed.json version"));
        }
        let mut names = BTreeSet::new();
        let mut identities = BTreeSet::new();
        for r in &disk.mods {
            if !names.insert(r.local.clone()) || !identities.insert(r.identity.clone()) {
                return Err(bad("duplicate installed mod"));
            }
            if r.release.is_some() || !r.grants.is_empty() || !r.links.is_empty() {
                return Err(bad("this preview supports only page-only local mods"));
            }
            let current = home.join("mods").join(r.local.as_ref());
            let old = current.with_extension("old");
            if old.is_dir()
                && digest(&current).ok().as_ref() != Some(&r.package)
                && digest(&old).ok().as_ref() == Some(&r.package)
            {
                if current.exists() {
                    home::remove_entry(&current)?;
                }
                fs::rename(old, current)?;
            }
        }
        for entry in fs::read_dir(home.join("mods"))? {
            let entry = entry?;
            if !names.iter().any(|n| entry.file_name() == n.as_ref()) {
                home::remove_entry(&entry.path())?;
            }
        }
        let mut mods = Vec::new();
        for r in disk.mods {
            let slug: Slug = r
                .identity
                .strip_prefix("local/")
                .ok_or_else(|| bad("nonlocal preview identity"))?
                .parse()
                .map_err(|_| bad("invalid preview identity"))?;
            let dir = home.join("mods").join(r.local.as_ref());
            let mut installed = match page_mod(&dir) {
                Ok(m) if m.slug() == &slug => view(&m, r.local.clone(), r.package.clone()),
                result => InstalledMod {
                    local: r.local,
                    identity: r.identity,
                    slug: slug.clone(),
                    name: slug.to_string(),
                    summary: String::new(),
                    version: Version {
                        major: 0,
                        minor: 0,
                        patch: 0,
                    },
                    publisher: None,
                    package: r.package,
                    release: None,
                    origin: None,
                    enabled: r.enabled,
                    has_logic: false,
                    page: None,
                    grants: vec![],
                    links: vec![],
                    error: Some(match result {
                        Err(e) => e.to_string(),
                        Ok(_) => "installed slug differs from identity".into(),
                    }),
                },
            };
            installed.enabled = r.enabled;
            mods.push(installed);
        }
        mods.sort_by(|a, b| a.local.cmp(&b.local));
        Ok(Self {
            generation: disk.generation,
            mods,
        })
    }

    pub fn install(
        &mut self,
        home: &Path,
        folders: &[PathBuf],
        port: u16,
    ) -> Result<Vec<InstalledMod>, HostError> {
        let mut prepared = Vec::new();
        let mut seen = BTreeSet::new();
        let mut names: BTreeSet<_> = self.mods.iter().map(|m| m.local.clone()).collect();
        for folder in folders {
            let source = page_mod(folder)?;
            let identity = format!("local/{}", source.slug());
            if !seen.insert(identity.clone()) {
                return Err(invalid(folder, "twice in one plan"));
            }
            let staging = Stage::new(home)?;
            // ponytail: replace this copy and reread with pack(dir,false) + unpack.
            copy_folder(folder, &staging.0)?;
            let m = page_mod(&staging.0)?;
            if m.slug() != source.slug() {
                return Err(invalid(folder, "slug changed during staging"));
            }
            let package = digest(&staging.0)?;
            let old = self.mods.iter().find(|m| m.identity == identity);
            let local = match old {
                Some(m) => m.local.clone(),
                None => derive_local(m.slug().as_str(), &names)?,
            };
            names.insert(local.clone());
            let unchanged = old.is_some_and(|m| m.package == package);
            let mut installed = view(&m, local, package);
            if let Some(old) = old {
                installed.enabled = old.enabled;
            }
            prepared.push((staging, installed, unchanged));
        }
        prepared.sort_by(|a, b| a.1.local.cmp(&b.1.local));
        let mut next = self.mods.clone();
        let mut moved: Vec<(PathBuf, bool)> = Vec::new();
        let changed = prepared.iter().any(|(_, _, unchanged)| !unchanged);
        let transaction = (|| -> Result<(), HostError> {
            for (staging, installed, unchanged) in &prepared {
                if *unchanged {
                    continue;
                }
                let dest = home.join("mods").join(installed.local.as_ref());
                let new = dest.with_extension("new");
                let old = dest.with_extension("old");
                fs::rename(&staging.0, &new)?;
                let update = dest.exists();
                if update && let Err(e) = fs::rename(&dest, &old) {
                    let _ = fs::remove_dir_all(&new);
                    return Err(e.into());
                }
                moved.push((dest.clone(), update));
                fs::rename(new, &dest)?;
                next.retain(|m| m.identity != installed.identity);
                next.push(installed.clone());
            }
            next.sort_by(|a, b| a.local.cmp(&b.local));
            if changed {
                home::write_json(
                    &home.join("installed.json"),
                    &DiskCatalog {
                        watane_installed: 1,
                        generation: self.generation + 1,
                        mods: next.iter().map(Record::from).collect(),
                    },
                )?;
            }
            Ok(())
        })();
        if let Err(error) = transaction {
            for (dest, update) in moved.iter().rev() {
                if dest.exists() {
                    home::remove_entry(dest)?;
                }
                if *update {
                    fs::rename(dest.with_extension("old"), dest)?;
                }
                let new = dest.with_extension("new");
                if new.exists() {
                    home::remove_entry(&new)?;
                }
            }
            return Err(error);
        }
        self.mods = next;
        if changed {
            self.generation += 1;
        }
        for (dest, update) in moved {
            if update && let Err(e) = fs::remove_dir_all(dest.with_extension("old")) {
                eprintln!("watane: remove old mod: {e}");
            }
        }
        Ok(prepared
            .into_iter()
            .map(|(_, mut m, _)| {
                m.page = m
                    .page
                    .map(|_| format!("http://{}.localhost:{port}/", m.local));
                m
            })
            .collect())
    }
}

fn derive_local(slug: &str, names: &BTreeSet<LocalName>) -> Result<LocalName, HostError> {
    let base = slug[..slug.len().min(21)].trim_end_matches('-');
    let local = LocalName::parse(base)?;
    if !names.contains(&local) {
        return Ok(local);
    }
    for n in 2.. {
        let suffix = format!("-{n}");
        let base = base[..base.len().min(24 - suffix.len())].trim_end_matches('-');
        let name = LocalName::parse(&format!("{base}{suffix}"))?;
        if !names.contains(&name) {
            return Ok(name);
        }
    }
    unreachable!()
}

struct Stage(PathBuf);
impl Stage {
    fn new(home: &Path) -> Result<Self, HostError> {
        loop {
            let mut random = [0; 8];
            getrandom::fill(&mut random).map_err(|e| HostError::Internal(e.to_string()))?;
            let name: String = random.iter().map(|b| format!("{b:02x}")).collect();
            let path = home.join("staging").join(name);
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn files(dir: &Path) -> Result<Vec<String>, HostError> {
    let mut files = Vec::new();
    let mut stack = vec![
        "mod.toml".to_owned(),
        "deps.json".to_owned(),
        "wit".to_owned(),
        "ui".to_owned(),
    ];
    while let Some(relative) = stack.pop() {
        let path = dir.join(&relative);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && relative == "deps.json" => {
                continue;
            }
            Err(e) => return Err(invalid(dir, e)),
        };
        if metadata.file_type().is_symlink() || (!metadata.is_file() && !metadata.is_dir()) {
            return Err(invalid(
                dir,
                format!("{relative}: links and special files are refused"),
            ));
        }
        if relative.len() > 255 || relative.split('/').count() > 16 {
            return Err(invalid(dir, "path exceeds package limits"));
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| invalid(dir, "path is not UTF-8"))?;
                stack.push(format!("{relative}/{name}"));
            }
        } else {
            files.push(relative);
        }
        if files.len() + stack.len() > 4096 {
            return Err(invalid(dir, "too many package files"));
        }
    }
    files.sort();
    Ok(files)
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, HostError> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(67_108_865)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 67_108_864 {
        return Err(invalid(path, "package exceeds 64 MiB"));
    }
    Ok(bytes)
}

fn copy_folder(source: &Path, dest: &Path) -> Result<(), HostError> {
    let mut total = 0;
    for file in files(source)? {
        let bytes = read_bounded(&source.join(&file))?;
        total += bytes.len();
        if total > 67_108_864 {
            return Err(invalid(source, "package exceeds 64 MiB"));
        }
        let path = dest.join(file);
        fs::create_dir_all(path.parent().expect("staged file has parent"))?;
        fs::write(path, bytes)?;
    }
    Ok(())
}

fn digest(dir: &Path) -> Result<PackageId, HostError> {
    // ponytail: temporary digest until pkg pack supplies the canonical PackageId.
    let mut encoded = Vec::new();
    for file in files(dir)? {
        let bytes = read_bounded(&dir.join(&file))?;
        if encoded.len() + file.len() + 9 + bytes.len() > 67_108_864 {
            return Err(invalid(dir, "package exceeds 64 MiB"));
        }
        encoded.extend_from_slice(file.as_bytes());
        encoded.push(0);
        encoded.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        encoded.extend_from_slice(&bytes);
    }
    Ok(PackageId::of(&encoded))
}
