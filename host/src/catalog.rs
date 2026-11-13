use crate::{HostError, LocalName, VerifyCheck, home};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
use watane_pkg::{
    Grant, InterfaceName, Mod, Package, PackageId, Provider, PublisherKey, RELEASE_KIND, Release,
    ReleaseRef, Slug, Version,
};

// The public source interface carries checked package and entry values directly.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum Source {
    Folder(PathBuf),
    Release {
        package: Package,
        entry: watane_log::Entry,
    },
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Change {
    New,
    Update { from_version: Version },
    Unchanged,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bind {
    pub consumer: String,
    pub provider: Provider,
    pub to: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct PlanEntry {
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
    pub change: Change,
    pub grants: Vec<Grant>,
    pub new_grants: Vec<Grant>,
    pub providers: Vec<LocalName>,
    pub binds: Vec<Bind>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MissingProvider {
    pub provider: Provider,
    pub version: Version,
    pub release: Option<ReleaseRef>,
    pub needed_by: String,
}
#[derive(Debug, Serialize)]
pub struct Plan {
    entries: Vec<PlanEntry>,
    missing: Vec<MissingProvider>,
    #[serde(skip)]
    prepared: Vec<Prepared>,
    #[serde(skip)]
    next: Vec<InstalledMod>,
    #[serde(skip)]
    generation: u64,
    #[serde(skip)]
    home: PathBuf,
    #[serde(skip)]
    confirmation: bool,
}
#[derive(Debug)]
struct Prepared(
    Stage,
    InstalledMod,
    bool,
    Option<std::sync::Arc<crate::runtime::Compiled>>,
);
impl Plan {
    pub fn entries(&self) -> &[PlanEntry] {
        &self.entries
    }
    pub fn missing(&self) -> &[MissingProvider] {
        &self.missing
    }
    pub fn needs_confirmation(&self) -> bool {
        self.confirmation
    }
}

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
    #[serde(default)]
    pub binds: Vec<Bind>,
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
    #[serde(default)]
    binds: Vec<Bind>,
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
            binds: m.binds.clone(),
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
        binds: vec![],
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
            let current = home.join("mods").join(r.local.as_ref());
            let old = current.with_extension("old");
            if old.is_dir()
                && watane_pkg::pack(&current, r.release.is_some())
                    .map(|p| p.id())
                    .ok()
                    .as_ref()
                    != Some(&r.package)
                && watane_pkg::pack(&old, r.release.is_some())
                    .map(|p| p.id())
                    .ok()
                    .as_ref()
                    == Some(&r.package)
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
            let (slug, publisher) = match &r.release {
                Some(release) => {
                    let provider = Provider {
                        slug: release.slug.clone(),
                        publisher: Some(release.publisher.clone()),
                    };
                    if r.identity != provider.to_string() {
                        return Err(bad("installed identity differs from release"));
                    }
                    (release.slug.clone(), Some(release.publisher.clone()))
                }
                None => (
                    r.identity
                        .strip_prefix("local/")
                        .ok_or_else(|| bad("nonlocal identity has no release"))?
                        .parse::<Slug>()
                        .map_err(|_| bad("invalid local identity"))?,
                    None,
                ),
            };
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
                    identity: r.identity.clone(),
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
                    binds: vec![],
                    error: Some(match result {
                        Err(e) => e.to_string(),
                        Ok(_) => "installed slug differs from identity".into(),
                    }),
                },
            };
            installed.identity = r.identity;
            installed.publisher = publisher;
            installed.release = r.release;
            installed.enabled = r.enabled;
            installed.grants = r.grants;
            installed.links = r.links;
            installed.binds = r.binds;
            mods.push(installed);
        }
        // Keep the committed provider-first order.
        Ok(Self {
            generation: disk.generation,
            components,
            mods,
        })
    }

    pub fn plan(
        &mut self,
        home: &Path,
        sources: &[Source],
        engine: &wasmtime::Engine,
        preview: bool,
        harness: bool,
    ) -> Result<Plan, HostError> {
        self.plan_with(home, sources, engine, preview, harness, &[])
    }

    pub fn plan_with(
        &mut self,
        home: &Path,
        sources: &[Source],
        engine: &wasmtime::Engine,
        preview: bool,
        harness: bool,
        binds: &[Bind],
    ) -> Result<Plan, HostError> {
        let mut entries = Vec::new();
        let mut prepared = Vec::new();
        let mut seen = BTreeSet::new();
        let mut worlds = BTreeMap::new();
        for installed in &self.mods {
            if let Ok(m) = Mod::read(&home.join("mods").join(installed.local.as_ref())) {
                worlds.insert(installed.local.clone(), m.world().clone());
            }
        }
        let mut names: BTreeSet<_> = self.mods.iter().map(|m| m.local.clone()).collect();
        for source in sources {
            let (package, release_body, release) = match source {
                Source::Folder(folder) => (
                    watane_pkg::pack(folder, false).map_err(|e| invalid(folder, e))?,
                    None,
                    None,
                ),
                Source::Release { package, entry } => {
                    if entry.kind() != RELEASE_KIND {
                        return Err(HostError::Verify {
                            check: VerifyCheck::Kind,
                            reason: format!("expected {RELEASE_KIND}, found {}", entry.kind()),
                        });
                    }
                    let body = Release::decode(entry.body()).map_err(|e| HostError::Verify {
                        check: VerifyCheck::ReleaseBody,
                        reason: e.to_string(),
                    })?;
                    if body.package != package.id() {
                        return Err(HostError::Verify {
                            check: VerifyCheck::PackageHash,
                            reason: format!(
                                "release names {}, package is {}",
                                body.package,
                                package.id()
                            ),
                        });
                    }
                    let release = ReleaseRef {
                        publisher: PublisherKey::from_bytes(*entry.author().as_bytes()),
                        slug: body.slug.clone(),
                        version: body.version.clone(),
                    };
                    (package.clone(), Some(body), Some(release))
                }
            };
            let input = match source {
                Source::Folder(folder) => folder.clone(),
                Source::Release { .. } => PathBuf::from(release.as_ref().unwrap().to_string()),
            };
            let folder = input.as_path();
            let staging = Stage::new(home)?;
            let m = package
                .unpack(&staging.0.join("mod"))
                .map_err(|e| invalid(folder, e))?;
            if let Some(body) = release_body {
                body.check(&m, &package).map_err(|e| HostError::Verify {
                    check: VerifyCheck::Mismatch,
                    reason: e.to_string(),
                })?;
            }
            let publisher = release.as_ref().map(|r| r.publisher.clone());
            let identity = Provider {
                slug: m.slug().clone(),
                publisher: publisher.clone(),
            }
            .to_string();
            if !seen.insert(identity.clone()) {
                return Err(invalid(folder, "twice in one plan"));
            }
            let component = crate::runtime::compile(engine, &m)?;
            let package = package.id();
            let old = self.mods.iter().find(|m| m.identity == identity);
            let local = match old {
                Some(m) => m.local.clone(),
                None => derive_local(m.slug().as_str(), &names)?,
            };
            worlds.insert(local.clone(), m.world().clone());
            names.insert(local.clone());
            let unchanged = old.is_some_and(|m| m.package == package);
            let mut installed = view(&m, local, package);
            installed.identity = identity.clone();
            installed.publisher = publisher;
            installed.release = release;
            let change = match old {
                None => Change::New,
                Some(_) if unchanged => Change::Unchanged,
                Some(old) => Change::Update {
                    from_version: old.version.clone(),
                },
            };
            let grants = m.grants();
            let new_grants = grants
                .iter()
                .filter(|g| {
                    old.is_none_or(|old| !old.grants.iter().any(|p| p.grant.same_authority(g)))
                })
                .cloned()
                .collect();
            if let Some(old) = old {
                installed.enabled = old.enabled;
            }
            entries.push(PlanEntry {
                local: installed.local.clone(),
                identity: identity.clone(),
                slug: installed.slug.clone(),
                name: installed.name.clone(),
                summary: installed.summary.clone(),
                version: installed.version.clone(),
                publisher: installed.publisher.clone(),
                package: installed.package.clone(),
                release: installed.release.clone(),
                origin: installed.origin.clone(),
                change,
                grants,
                new_grants,
                providers: vec![],
                binds: vec![],
            });
            prepared.push(Prepared(staging, installed, unchanged, component));
        }
        let mut next = self.mods.clone();
        for Prepared(_, installed, _, _) in &prepared {
            next.retain(|m| m.identity != installed.identity);
            next.push(installed.clone());
        }
        for (index, pick) in binds.iter().enumerate() {
            let consumer = next.iter().find(|m| m.identity == pick.consumer);
            let valid = consumer.is_some_and(|m| declares(m, &pick.provider))
                && parse_identity(&pick.to).is_some_and(|p| p.slug == pick.provider.slug)
                && next.iter().any(|m| m.identity == pick.to)
                && !binds[..index]
                    .iter()
                    .any(|b| b.consumer == pick.consumer && b.provider == pick.provider)
                && pick.to != pick.provider.to_string();
            if !valid {
                return Err(HostError::BadRequest(format!(
                    "invalid bind: {} from {} to {}",
                    pick.consumer, pick.provider, pick.to
                )));
            }
        }
        for m in &mut next {
            let old = self.mods.iter().find(|old| old.identity == m.identity);
            let mut picks = Vec::new();
            for pick in binds
                .iter()
                .filter(|b| b.consumer == m.identity)
                .chain(old.into_iter().flat_map(|m| &m.binds))
            {
                if declares(m, &pick.provider)
                    && !picks.iter().any(|b: &Bind| b.provider == pick.provider)
                {
                    picks.push(pick.clone());
                }
            }
            m.binds = picks;
            if let Some(old) = old
                && m.binds != old.binds
                && !seen.contains(&m.identity)
            {
                seen.insert(m.identity.clone());
                entries.push(PlanEntry {
                    local: m.local.clone(),
                    identity: m.identity.clone(),
                    slug: m.slug.clone(),
                    name: m.name.clone(),
                    summary: m.summary.clone(),
                    version: m.version.clone(),
                    publisher: m.publisher.clone(),
                    package: m.package.clone(),
                    release: m.release.clone(),
                    origin: m.origin.clone(),
                    change: Change::Unchanged,
                    grants: m.grants.iter().map(|g| g.grant.clone()).collect(),
                    new_grants: vec![],
                    providers: vec![],
                    binds: vec![],
                });
                prepared.push(Prepared(
                    Stage::new(home)?,
                    m.clone(),
                    true,
                    self.components.get(&m.local).cloned(),
                ));
            }
        }
        let missing = bind(&mut next, &worlds, &seen, harness, preview, true)?;
        let order = provider_order(&next)?;
        next.sort_by_key(|m| order.iter().position(|n| n == &m.local).unwrap());
        for Prepared(_, installed, _, _) in &mut prepared {
            let resolved = next.iter().find(|m| m.local == installed.local).unwrap();
            installed.links = resolved.links.clone();
            installed.binds = resolved.binds.clone();
        }
        for e in &mut entries {
            let resolved = next.iter().find(|m| m.local == e.local).unwrap();
            e.binds = resolved.binds.clone();
            if let Some(old) = self.mods.iter().find(|m| m.identity == e.identity)
                && old.binds != e.binds
                && matches!(e.change, Change::Unchanged)
            {
                e.change = Change::Update {
                    from_version: old.version.clone(),
                };
            }
            e.providers = next
                .iter()
                .find(|m| m.local == e.local)
                .unwrap()
                .links
                .iter()
                .filter_map(|l| l.provider.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
        }
        prepared.sort_by_key(|p| order.iter().position(|n| n == &p.1.local).unwrap());
        entries.sort_by_key(|e| order.iter().position(|n| n == &e.local).unwrap());
        let confirmation = !preview
            && entries.iter().any(|e| {
                matches!(e.change, Change::New)
                    || matches!(e.change, Change::Update { .. }) && !e.new_grants.is_empty()
                    || self
                        .mods
                        .iter()
                        .any(|m| m.identity == e.identity && m.binds != e.binds)
            });
        Ok(Plan {
            entries,
            missing,
            prepared,
            next,
            generation: self.generation,
            home: home.into(),
            confirmation,
        })
    }

    pub fn validate(&self, plan: &Plan, home: &Path) -> Result<(), HostError> {
        if plan.generation != self.generation || plan.home != home {
            return Err(HostError::StalePlan);
        }
        if !plan.missing.is_empty() {
            return Err(HostError::MissingProviders(plan.missing.clone()));
        }
        Ok(())
    }
    pub fn commit(
        &mut self,
        home: &Path,
        mut plan: Plan,
        withhold: &[(LocalName, Grant)],
        preview: bool,
    ) -> Result<Vec<InstalledMod>, HostError> {
        self.validate(&plan, home)?;
        for Prepared(_, installed, _, _) in &mut plan.prepared {
            let old = self.mods.iter().find(|m| m.identity == installed.identity);
            for state in &mut installed.grants {
                state.granted = if preview {
                    true
                } else {
                    match old.and_then(|m| {
                        m.grants
                            .iter()
                            .find(|g| g.grant.same_authority(&state.grant))
                    }) {
                        Some(previous) => previous.granted,
                        None => !withhold
                            .iter()
                            .any(|(local, g)| local == &installed.local && g == &state.grant),
                    }
                };
            }
            *plan
                .next
                .iter_mut()
                .find(|m| m.local == installed.local)
                .unwrap() = installed.clone();
        }
        let prepared = plan.prepared;
        let next = plan.next;
        let mut moved: Vec<(PathBuf, bool)> = Vec::new();
        let changed = prepared.iter().any(|Prepared(_, m, unchanged, _)| {
            !unchanged
                || self
                    .mods
                    .iter()
                    .any(|old| old.identity == m.identity && old.binds != m.binds)
        });
        let transaction = (|| -> Result<(), HostError> {
            for Prepared(staging, installed, unchanged, _) in &prepared {
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
        for Prepared(_, installed, unchanged, component) in &prepared {
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
        Ok(prepared.into_iter().map(|Prepared(_, m, _, _)| m).collect())
    }
    pub fn relink(&mut self, home: &Path, preview: bool) -> Result<(), HostError> {
        let worlds = self
            .mods
            .iter()
            .filter_map(|m| {
                Mod::read(&home.join("mods").join(m.local.as_ref()))
                    .ok()
                    .map(|v| (m.local.clone(), v.world().clone()))
            })
            .collect();
        bind(
            &mut self.mods,
            &worlds,
            &BTreeSet::new(),
            false,
            preview,
            false,
        )?;
        provider_order(&self.mods)?;
        Ok(())
    }
    pub fn save(&mut self, home: &Path) -> Result<(), HostError> {
        home::write_json(
            &home.join("installed.json"),
            &DiskCatalog {
                watane_installed: 1,
                generation: self.generation + 1,
                mods: self.mods.iter().map(Record::from).collect(),
            },
        )?;
        self.generation += 1;
        Ok(())
    }
}

fn declares(m: &InstalledMod, provider: &Provider) -> bool {
    m.grants.iter().any(|g| {
        matches!(&g.grant,
        Grant::Call { provider: p, .. } | Grant::Subscribe { provider: p, .. } if p == provider)
    })
}

fn parse_identity(identity: &str) -> Option<Provider> {
    let (publisher, slug) = identity.split_once('/')?;
    Some(Provider {
        slug: slug.parse().ok()?,
        publisher: if publisher == "local" {
            None
        } else {
            Some(publisher.parse().ok()?)
        },
    })
}

fn fits(
    consumer: &InstalledMod,
    provider: &Provider,
    candidate: &InstalledMod,
    worlds: &BTreeMap<LocalName, watane_pkg::World>,
) -> bool {
    let Some(world) = worlds.get(&candidate.local) else {
        return false;
    };
    consumer.grants.iter().all(|g| match &g.grant {
        Grant::Call {
            interface,
            provider: p,
        } if p == provider => world.exports.iter().any(|i| interface.satisfied_by(i)),
        Grant::Subscribe {
            interface,
            provider: p,
        } if p == provider => world.events.iter().any(|i| interface.satisfied_by(i)),
        _ => true,
    })
}

fn bind(
    mods: &mut [InstalledMod],
    worlds: &BTreeMap<LocalName, watane_pkg::World>,
    supplied: &BTreeSet<String>,
    harness: bool,
    preview: bool,
    // A plan refuses two candidates; a relink after remove leaves the link unbound (4.5 rule 4).
    refuse_ambiguous: bool,
) -> Result<Vec<MissingProvider>, HostError> {
    let providers = mods.to_vec();
    let mut missing = Vec::new();
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
                let pick = m
                    .binds
                    .iter()
                    .find(|b| b.provider == *provider)
                    .filter(|b| providers.iter().any(|p| p.identity == b.to));
                if p.slug != provider.slug
                    || match pick {
                        Some(pick) => p.identity != pick.to || !fits(m, provider, p, worlds),
                        None => !preview && p.publisher != provider.publisher,
                    }
                {
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
                if refuse_ambiguous {
                    return Err(HostError::AmbiguousProvider(interface.to_string()));
                }
                candidates.clear();
            }
            let candidate = candidates.pop();
            if candidate.is_none()
                && supplied.contains(&m.identity)
                && !harness
                && !missing.iter().any(|m: &MissingProvider| {
                    m.provider == *provider && m.version == interface.version()
                })
            {
                missing.push(MissingProvider {
                    provider: provider.clone(),
                    version: interface.version().clone(),
                    release: provider.publisher.as_ref().map(|publisher| ReleaseRef {
                        publisher: publisher.clone(),
                        slug: provider.slug.clone(),
                        version: interface.version().clone(),
                    }),
                    needed_by: m.identity.clone(),
                });
            }
            m.links.push(LinkView {
                kind,
                interface: interface.clone(),
                provider: candidate.as_ref().map(|c| c.0.clone()),
                provider_interface: candidate.map(|c| c.1),
            });
        }
    }
    Ok(missing)
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

#[derive(Debug)]
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
