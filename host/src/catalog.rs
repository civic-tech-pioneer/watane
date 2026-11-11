use crate::{HostError, LocalName, home};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
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

#[derive(Clone, Debug, Default)]
pub(crate) struct Catalog {
    generation: u64,
    pub components: BTreeMap<LocalName, std::sync::Arc<crate::runtime::Compiled>>,
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

fn preview_mod(dir: &Path) -> Result<Mod, HostError> {
    Mod::read(dir).map_err(|e| invalid(dir, e))
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
        has_logic: m.logic().is_some(),
        // A relative marker becomes the origin URL at the transport boundary.
        page: m.page().map(|_| "/".into()),
        grants: m
            .grants()
            .into_iter()
            .map(|grant| GrantState {
                grant,
                granted: true,
            })
            .collect(),
        links: vec![],
        error: None,
    }
}

impl Catalog {
    pub fn open(home: &Path, engine: &wasmtime::Engine) -> Result<Self, HostError> {
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
            if r.release.is_some() {
                return Err(bad("this preview supports only local mods"));
            }
            let current = home.join("mods").join(r.local.as_ref());
            let old = current.with_extension("old");
            if old.is_dir()
                && watane_pkg::pack(&current, false)
                    .map(|p| p.id())
                    .ok()
                    .as_ref()
                    != Some(&r.package)
                && watane_pkg::pack(&old, false).map(|p| p.id()).ok().as_ref() == Some(&r.package)
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
        let mut components = BTreeMap::new();
        for r in disk.mods {
            let slug: Slug = r
                .identity
                .strip_prefix("local/")
                .ok_or_else(|| bad("nonlocal preview identity"))?
                .parse()
                .map_err(|_| bad("invalid preview identity"))?;
            let dir = home.join("mods").join(r.local.as_ref());
            let mut installed = match preview_mod(&dir) {
                Ok(m) if m.slug() == &slug => {
                    let mut installed = view(&m, r.local.clone(), r.package.clone());
                    match crate::runtime::compile(engine, &m) {
                        Ok(Some(component)) => {
                            components.insert(installed.local.clone(), component);
                        }
                        Ok(None) => (),
                        Err(e) => installed.error = Some(e.to_string()),
                    }
                    installed
                }
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
            installed.grants = r.grants;
            installed.links = r.links;
            mods.push(installed);
        }
        // Keep the committed provider-first order.
        Ok(Self {
            generation: disk.generation,
            components,
            mods,
        })
    }

    pub fn install(
        &mut self,
        home: &Path,
        folders: &[PathBuf],
        port: u16,
        engine: &wasmtime::Engine,
        mode: crate::Mode,
        harness: bool,
    ) -> Result<Vec<InstalledMod>, HostError> {
        let mut prepared = Vec::new();
        let mut seen = BTreeSet::new();
        let mut worlds = BTreeMap::new();
        for installed in &self.mods {
            if let Ok(m) = Mod::read(&home.join("mods").join(installed.local.as_ref())) {
                worlds.insert(installed.local.clone(), m.world().clone());
            }
        }
        let mut names: BTreeSet<_> = self.mods.iter().map(|m| m.local.clone()).collect();
        for folder in folders {
            let package = watane_pkg::pack(folder, false).map_err(|e| invalid(folder, e))?;
            let staging = Stage::new(home)?;
            let m = package
                .unpack(&staging.0.join("mod"))
                .map_err(|e| invalid(folder, e))?;
            let identity = format!("local/{}", m.slug());
            if !seen.insert(identity.clone()) {
                return Err(invalid(folder, "twice in one plan"));
            }
            let component = crate::runtime::compile(engine, &m)?;
            let package = package.id();
            let old = self.mods.iter().find(|m| m.identity == identity);
            if mode == crate::Mode::Home
                && old.is_none_or(|old| {
                    m.grants().iter().any(|grant| {
                        !old.grants
                            .iter()
                            .any(|previous| previous.grant.same_authority(grant))
                    })
                })
            {
                return Err(HostError::Refused(
                    "no app is attached to confirm the install".into(),
                ));
            }
            let local = match old {
                Some(m) => m.local.clone(),
                None => derive_local(m.slug().as_str(), &names)?,
            };
            worlds.insert(local.clone(), m.world().clone());
            names.insert(local.clone());
            let unchanged = old.is_some_and(|m| m.package == package);
            let mut installed = view(&m, local, package);
            if let Some(old) = old {
                installed.enabled = old.enabled;
                if mode == crate::Mode::Home {
                    for grant in &mut installed.grants {
                        grant.granted = old.grants.iter().any(|previous| {
                            previous.granted && previous.grant.same_authority(&grant.grant)
                        });
                    }
                }
            }
            prepared.push((staging, installed, unchanged, component));
        }
        let mut next = self.mods.clone();
        for (_, installed, _, _) in &prepared {
            next.retain(|m| m.identity != installed.identity);
            next.push(installed.clone());
        }
        bind(&mut next, &worlds, &seen, harness)?;
        let order = provider_order(&next)?;
        next.sort_by_key(|m| order.iter().position(|n| n == &m.local).unwrap());
        for (_, installed, _, _) in &mut prepared {
            installed.links = next
                .iter()
                .find(|m| m.local == installed.local)
                .unwrap()
                .links
                .clone();
        }
        prepared.sort_by_key(|p| order.iter().position(|n| n == &p.1.local).unwrap());
        let mut moved: Vec<(PathBuf, bool)> = Vec::new();
        let changed = prepared.iter().any(|(_, _, unchanged, _)| !unchanged);
        let transaction = (|| -> Result<(), HostError> {
            for (staging, installed, unchanged, _) in &prepared {
                if *unchanged {
                    continue;
                }
                let dest = home.join("mods").join(installed.local.as_ref());
                let new = dest.with_extension("new");
                let old = dest.with_extension("old");
                fs::rename(staging.0.join("mod"), &new)?;
                let update = dest.exists();
                if update && let Err(e) = fs::rename(&dest, &old) {
                    let _ = fs::remove_dir_all(&new);
                    return Err(e.into());
                }
                moved.push((dest.clone(), update));
                fs::rename(new, &dest)?;
            }
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
        for (_, installed, unchanged, component) in &prepared {
            if !unchanged {
                self.components.remove(&installed.local);
                if let Some(component) = component {
                    self.components
                        .insert(installed.local.clone(), component.clone());
                }
            }
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
            .map(|(_, mut m, _, _)| {
                m.page = m
                    .page
                    .map(|_| format!("http://{}.localhost:{port}/", m.local));
                m
            })
            .collect())
    }
}

fn bind(
    mods: &mut [InstalledMod],
    worlds: &BTreeMap<LocalName, watane_pkg::World>,
    supplied: &BTreeSet<String>,
    harness: bool,
) -> Result<(), HostError> {
    let providers = mods.to_vec();
    let mut missing = BTreeSet::new();
    for m in mods {
        m.links.clear();
        for g in &m.grants {
            let (kind, interface, provider) = match &g.grant {
                Grant::Call {
                    interface,
                    provider,
                } => (LinkKind::Call, interface, provider),
                Grant::Subscribe {
                    interface,
                    provider,
                } => (LinkKind::Subscribe, interface, provider),
                _ => continue,
            };
            let mut candidates = Vec::new();
            for p in &providers {
                if p.slug != provider.slug || p.publisher != provider.publisher {
                    continue;
                }
                let Some(world) = worlds.get(&p.local) else {
                    continue;
                };
                let offered = match kind {
                    LinkKind::Call => &world.exports,
                    LinkKind::Subscribe => &world.events,
                };
                for i in offered {
                    if interface.satisfied_by(i) {
                        candidates.push((p.local.clone(), i.clone()));
                    }
                }
            }
            if candidates.len() > 1 {
                return Err(HostError::AmbiguousProvider(interface.to_string()));
            }
            let candidate = candidates.pop();
            if candidate.is_none() && supplied.contains(&m.identity) && !harness {
                missing.insert(format!("{} at {}", provider, interface.version()));
            }
            m.links.push(LinkView {
                kind,
                interface: interface.clone(),
                provider: candidate.as_ref().map(|c| c.0.clone()),
                provider_interface: candidate.map(|c| c.1),
            });
        }
    }
    if !missing.is_empty() {
        return Err(HostError::MissingProviders(
            missing.into_iter().collect::<Vec<_>>().join(", "),
        ));
    }
    Ok(())
}

fn provider_order(mods: &[InstalledMod]) -> Result<Vec<LocalName>, HostError> {
    fn visit(
        local: &LocalName,
        mods: &[InstalledMod],
        path: &mut Vec<LocalName>,
        done: &mut BTreeSet<LocalName>,
    ) -> Result<(), HostError> {
        if let Some(start) = path.iter().position(|n| n == local) {
            let mut cycle = path[start..].to_vec();
            cycle.push(local.clone());
            return Err(HostError::Cycle(
                cycle
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" -> "),
            ));
        }
        if done.contains(local) {
            return Ok(());
        }
        path.push(local.clone());
        let m = mods
            .iter()
            .find(|m| &m.local == local)
            .expect("linked mod exists");
        let mut providers: Vec<_> = m
            .links
            .iter()
            .filter(|l| matches!(l.kind, LinkKind::Call))
            .filter_map(|l| l.provider.as_ref())
            .collect();
        providers.sort();
        providers.dedup();
        for p in providers {
            visit(p, mods, path, done)?;
        }
        path.pop();
        done.insert(local.clone());
        Ok(())
    }
    let mut names: Vec<_> = mods.iter().map(|m| &m.local).collect();
    names.sort();
    let mut done = BTreeSet::new();
    for n in names {
        visit(n, mods, &mut Vec::new(), &mut done)?;
    }
    // Call edges must be acyclic. Subscription edges only prefer providers
    // first; a subscription cycle cannot impose a total dependency order.
    let mut remaining: BTreeSet<_> = mods.iter().map(|m| m.local.clone()).collect();
    let mut ordered = Vec::new();
    while !remaining.is_empty() {
        let ready = |local: &&LocalName, calls_only: bool| {
            let m = mods.iter().find(|m| &m.local == *local).unwrap();
            m.links
                .iter()
                .filter(|l| !calls_only || matches!(l.kind, LinkKind::Call))
                .filter_map(|l| l.provider.as_ref())
                .all(|p| !remaining.contains(p))
        };
        let next = remaining
            .iter()
            .find(|n| ready(n, false))
            .or_else(|| remaining.iter().find(|n| ready(n, true)))
            .expect("call cycle was checked")
            .clone();
        remaining.remove(&next);
        ordered.push(next);
    }
    Ok(ordered)
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
