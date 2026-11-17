mod link;
pub use link::Link;
pub use watane_log::PublicKey;
pub use watane_pkg::Slug;
mod error;
pub use error::DistError;
pub use watane_log::{DecodeError, Entry, EntryId, KeyTextError};
pub use watane_pkg::{
    Grant, InterfaceName, Package, PackageError, PackageId, Provider, PublisherKey, Release,
    ReleaseError, ReleaseRef, Version,
};
mod confirm;
pub use confirm::{Confirm, Decision, Listing, Profile, SignRequest, Statement, Withdraw};
mod body;
#[derive(Clone, Debug)]
pub struct ChannelView {
    pub channel: PublicKey,
    pub fingerprint: String,
    pub name: Option<String>, // the last valid profile
    pub base: Option<String>, // followed: the base of its link; own: the site URL
    pub head: Option<u64>,
    pub releases: Vec<ReleaseInfo>, // seq order; withdrawn ones included
    pub bad: Vec<BadStatement>,     // seq order
    pub status: Status,
    pub followed: bool,
    pub index: bool,
    pub kept: bool,
    pub listed_by: Vec<PublicKey>,
    pub listings: Vec<ListingInfo>,
    pub last_attempt: Option<u64>,
    pub last_success: Option<u64>, // ms since the Unix epoch; None for the own channel
}
#[derive(Clone, Debug)]
pub struct ListingInfo {
    pub entry: EntryId,
    pub seq: u64,
    pub include: bool,
    pub listing: Listing,
    pub unlisted: Option<u64>,
}
#[derive(Clone, Debug)]
pub struct ReleaseInfo {
    pub reference: ReleaseRef, // channel key, release.slug, release.version
    pub channel: PublicKey,
    pub entry: EntryId,
    pub seq: u64,
    pub release: Release,
    pub withdrawn: Option<Withdrawal>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Withdrawal {
    pub seq: u64,
    pub reason: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadStatement {
    pub seq: u64,
    pub kind: String,
    pub cause: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ok,
    Compromised { seq: u64 },
} // seq of the first contradicting position

mod view;

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
#[derive(Clone, Debug)]
pub struct Options {
    pub connect_timeout: Duration, // default 10 s
    pub entry_timeout: Duration,   // default 30 s: one whole GET of `head` or `entries/<n>`
    pub package_timeout: Duration, // default 300 s: one whole GET of `packages/<id>`
    pub max_entry_file: u64,       // default watane_log::MAX_ENTRY (65_728 bytes)
    pub max_package: u64,          // default watane_pkg::MAX_PACKAGE_BYTES (64 MiB)
    pub max_entries_per_read: u64, // default 10_000 entry files per channel per read
    pub max_listed: usize,
    pub max_listed_per_index: usize,
    pub max_listed_entries: u64,
    pub listed_local: bool,
    pub max_kept: usize,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            entry_timeout: Duration::from_secs(30),
            package_timeout: Duration::from_secs(300),
            max_entry_file: watane_log::MAX_ENTRY as u64,
            max_package: watane_pkg::MAX_PACKAGE_BYTES,
            max_entries_per_read: 10_000,
            max_listed: 512,
            max_listed_per_index: 128,
            max_listed_entries: 1_000,
            listed_local: false,
            max_kept: 4096,
        }
    }
}

pub struct Dist {
    home: PathBuf,
    _lock: File,
    log: watane_log::Log,
    confirm: Arc<dyn Confirm>,
    own_mutex: Mutex<()>,
    base: Mutex<Option<String>>,
    options: Options,
    follows: Mutex<std::collections::BTreeMap<PublicKey, Follow>>,
    read_mutex: Mutex<()>,
    agent: ureq::Agent,
}
// The confirm callback has no Debug, and the rest is lock and cache state.
impl std::fmt::Debug for Dist {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dist")
            .field("home", &self.home)
            .finish_non_exhaustive()
    }
}
impl Dist {
    pub fn open(
        home: &Path,
        confirm: Arc<dyn Confirm>,
        options: Options,
    ) -> Result<Self, DistError> {
        Self::open_inner(home, confirm, options, false)
    }
    pub fn open_keyless(home: &Path, options: Options) -> Result<Self, DistError> {
        Self::open_inner(home, Arc::new(KeylessConfirm), options, true)
    }
    fn open_inner(
        home: &Path,
        confirm: Arc<dyn Confirm>,
        options: Options,
        keyless: bool,
    ) -> Result<Self, DistError> {
        use std::fs::{self, OpenOptions};
        use std::os::unix::fs::OpenOptionsExt;
        let dir = home.join("dist");
        private_dir(&dir)?;
        let path = dir.join("lock");
        if let Ok(meta) = fs::symlink_metadata(&path)
            && !meta.is_file()
        {
            return Err(DistError::Corrupt {
                path,
                cause: "lock is not a regular file".into(),
            });
        }
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(&path)
            .map_err(|source| DistError::Io {
                path: path.clone(),
                source,
            })?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => return Err(DistError::Locked { path }),
            Err(std::fs::TryLockError::Error(source)) => {
                return Err(DistError::Io { path, source });
            }
        }
        private_dir(&dir.join("packages"))?;
        site::public_dir(&dir.join("site"))?;
        let tmp = dir.join("tmp");
        match fs::remove_dir_all(&tmp) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(DistError::Io { path: tmp, source }),
        }
        private_dir(&tmp)?;
        let log = if keyless {
            watane_log::Log::open_keyless(&home.join("log"))?
        } else {
            watane_log::Log::open(&home.join("log"))?
        };
        let mut follows = std::collections::BTreeMap::new();
        if let Some(file) = read_json::<Follows>(&dir.join("follows.json"))? {
            let path = dir.join("follows.json");
            if file.version != 1 {
                return Err(DistError::Corrupt {
                    path,
                    cause: "unsupported version".into(),
                });
            }
            for (key, mut record) in file.channels {
                let channel = key.parse().map_err(|e: KeyTextError| DistError::Corrupt {
                    path: path.clone(),
                    cause: e.to_string(),
                })?;
                if !record.base.is_empty() {
                    record.base = Link::new(channel, &record.base, None)
                        .map_err(|e| DistError::Corrupt {
                            path: path.clone(),
                            cause: e.to_string(),
                        })?
                        .base()
                        .into();
                }
                if record.base.is_empty()
                    && (!record.kept || record.followed || record.index || !record.bases.is_empty())
                {
                    return Err(DistError::Corrupt {
                        path: path.clone(),
                        cause: "empty base requires a kept-only record".into(),
                    });
                }
                if record.bases.len() > 4 {
                    return Err(DistError::Corrupt {
                        path: path.clone(),
                        cause: "more than four listed bases".into(),
                    });
                }
                for base in &mut record.bases {
                    *base = Link::new(channel, base, None)
                        .map_err(|e| DistError::Corrupt {
                            path: path.clone(),
                            cause: e.to_string(),
                        })?
                        .base()
                        .into();
                }
                if !record.followed && !record.index && !record.base.is_empty() {
                    if record.bases.is_empty() {
                        record.bases.push(record.base.clone());
                    }
                    record.base = record.bases[0].clone();
                }
                follows.insert(channel, record);
            }
        }
        let base = match read_json::<Site>(&dir.join("site.json"))? {
            None => None,
            Some(file) => {
                let path = dir.join("site.json");
                if file.version != 1 {
                    return Err(DistError::Corrupt {
                        path,
                        cause: "unsupported version".into(),
                    });
                }
                Some(
                    Link::new(
                        log.identity().ok_or(DistError::NoIdentity)?,
                        &file.url,
                        None,
                    )
                    .map_err(|e| DistError::Corrupt {
                        path,
                        cause: e.to_string(),
                    })?
                    .base()
                    .into(),
                )
            }
        };
        Ok(Self {
            home: home.to_owned(),
            _lock: lock,
            log,
            confirm,
            own_mutex: Mutex::new(()),
            base: Mutex::new(base),
            agent: ureq::Agent::config_builder()
                .http_status_as_error(false)
                .max_redirects(5)
                .timeout_connect(Some(options.connect_timeout))
                .user_agent(concat!("watane-dist/", env!("CARGO_PKG_VERSION")))
                .build()
                .into(),
            options,
            follows: Mutex::new(follows),
            read_mutex: Mutex::new(()),
        })
    }
    pub fn identity(&self) -> Option<PublicKey> {
        self.log.identity()
    }
    pub fn fingerprint(&self) -> Option<String> {
        self.identity().map(|key| key.fingerprint())
    }
    fn require_identity(&self) -> Result<PublicKey, DistError> {
        self.identity().ok_or(DistError::NoIdentity)
    }
    pub fn site_dir(&self) -> PathBuf {
        self.home.join("dist/site")
    }
}
mod own;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Follow {
    base: String,
    #[serde(default = "default_followed")]
    followed: bool,
    #[serde(default)]
    index: bool,
    #[serde(default)]
    kept: bool,
    #[serde(default)]
    bases: Vec<String>,
    #[serde(rename = "followed_at")]
    _followed_at: u64,
    #[serde(rename = "last_attempt")]
    _last_attempt: u64,
    last_success: Option<u64>,
}
fn default_followed() -> bool {
    true
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Follows {
    version: u64,
    channels: std::collections::BTreeMap<String, Follow>,
}
#[derive(serde::Deserialize)]
struct Site {
    version: u64,
    url: String,
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>, DistError> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| DistError::Corrupt {
                path: path.to_owned(),
                cause: e.to_string(),
            }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(DistError::Io {
            path: path.to_owned(),
            source,
        }),
    }
}
fn private_dir(path: &Path) -> Result<(), DistError> {
    use std::fs;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let io = |source| DistError::Io {
        path: path.to_owned(),
        source,
    };
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .map_err(io)?;
    if !fs::symlink_metadata(path).map_err(io)?.is_dir() {
        return Err(DistError::Corrupt {
            path: path.to_owned(),
            cause: "not a plain directory".into(),
        });
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(io)
}

mod fetch;
mod site;
#[derive(Debug)]
pub struct Published {
    pub entry: Entry,
    pub info: ReleaseInfo,
    pub link: Option<Link>,
    pub site: Exported,
}
#[derive(Debug)]
pub struct Exported {
    pub dir: PathBuf,
    pub head: Option<u64>,
    pub written: Vec<PathBuf>,
}

mod follow;
mod http;
#[derive(Debug)]
pub struct RefreshReport {
    pub channel: PublicKey,
    pub outcome: Outcome,
    pub changes: Vec<Change>,
}
#[derive(Debug)]
pub enum Outcome {
    Current { head: u64 },
    Behind { served: u64, stored: u64 },
    Compromised { seq: u64 },
    Failed { cause: DistError },
}
#[derive(Clone, Debug)]
pub enum Change {
    Name { seq: u64, name: String },
    Release(ReleaseInfo),
    Withdrawn { seq: u64, release: ReleaseInfo },
    Bad(BadStatement),
    Listed(ListingInfo),
    Unlisted { seq: u64, listing: ListingInfo },
}
#[derive(Debug)]
pub struct Fetched {
    pub link: Option<Link>,
    pub package: Package,
    pub entry: Entry,
    pub info: ReleaseInfo,
    pub publisher_name: Option<String>,
    pub checked_at: Option<u64>,
}

mod index;
pub use index::{IndexView, Skipped};

mod search;
pub use search::{Hit, InterfaceQuery, Query, Via};

struct KeylessConfirm;
impl Confirm for KeylessConfirm {
    fn confirm(&self, _: &SignRequest) -> Decision {
        unreachable!("keyless homes never ask for signatures")
    }
}

mod holder;
pub use holder::{HeadState, Ingested, Reason};
