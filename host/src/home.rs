//! The home (TECHNICAL 3): its layout and lock, `home.json`, `run.json`, the
//! local name grammar and `default_home`.

use crate::{HostError, Mode};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{fmt, fs};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct LocalName(String);

impl LocalName {
    pub fn parse(s: &str) -> Result<Self, HostError> {
        if (1..=24).contains(&s.len())
            && s.as_bytes()[0].is_ascii_lowercase()
            && s.split('-').all(|part| {
                !part.is_empty()
                    && part
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            })
        {
            Ok(Self(s.into()))
        } else {
            Err(HostError::BadName {
                value: s.into(),
                rule: "use 1 to 24 lowercase letters, digits and single internal hyphens; start with a letter",
            })
        }
    }
}
impl TryFrom<String> for LocalName {
    type Error = HostError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}
impl AsRef<str> for LocalName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for LocalName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunInfo {
    pub watane: u32,
    pub pid: u32,
    pub port: u16,
    pub mode: String,
}
impl RunInfo {
    pub fn read(home: &Path) -> Result<Option<Self>, HostError> {
        match fs::read(home.join("run.json")) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

pub fn default_home() -> Result<PathBuf, HostError> {
    if let Some(path) = std::env::var_os("WATANE_HOME").filter(|s| !s.is_empty()) {
        return Ok(path.into());
    }
    std::env::var_os("HOME")
        .filter(|s| !s.is_empty())
        .map(|p| PathBuf::from(p).join(".watane"))
        .ok_or_else(|| HostError::Internal("neither WATANE_HOME nor HOME is set".into()))
}

#[derive(Debug)]
pub(crate) struct Home {
    pub path: PathBuf,
    pub mode: Mode,
    _lock: fs::File,
}

impl Home {
    pub fn open(path: &Path, mode: Mode) -> Result<Self, HostError> {
        fs::create_dir_all(path).map_err(|e| HostError::BadHome {
            path: path.into(),
            reason: e.to_string(),
        })?;
        let path = path.canonicalize()?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.join("lock"))?;
        match lock.try_lock() {
            Ok(()) => (),
            Err(std::fs::TryLockError::WouldBlock) => return Err(HostError::Busy(path)),
            Err(e) => return Err(HostError::Internal(format!("lock {}: {e}", path.display()))),
        }
        let marker = path.join("home.json");
        if !marker.exists() {
            let empty =
                fs::read_dir(&path)?.all(|entry| entry.is_ok_and(|e| e.file_name() == "lock"));
            if !empty {
                return Err(HostError::BadHome {
                    path,
                    reason: "home.json is missing in a nonempty directory".into(),
                });
            }
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(marker)?;
            file.write_all(
                format!(r#"{{"watane_home":1,"mode":"{}"}}"#, mode.as_str()).as_bytes(),
            )?;
            file.sync_all()?;
        } else {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Marker {
                watane_home: u32,
                mode: String,
            }
            let read = || -> Result<Marker, HostError> {
                Ok(serde_json::from_slice(&fs::read(&marker)?)?)
            };
            let marker = read().map_err(|e| HostError::BadHome {
                path: path.clone(),
                reason: e.to_string(),
            })?;
            if marker.watane_home != 1 || !matches!(marker.mode.as_str(), "home" | "preview") {
                return Err(HostError::BadHome {
                    path,
                    reason: "unknown home version or mode".into(),
                });
            }
            if marker.mode != mode.as_str() {
                return Err(HostError::ModeMismatch {
                    path,
                    found: if marker.mode == "home" {
                        "home"
                    } else {
                        "preview"
                    },
                });
            }
        }
        for dir in ["mods", "staging"] {
            fs::create_dir_all(path.join(dir))?;
        }
        for entry in fs::read_dir(path.join("staging"))? {
            remove_entry(&entry?.path())?;
        }
        // A stale run file is not proof of a running host: the lock is.
        match fs::remove_file(path.join("run.json")) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
        Ok(Self {
            path,
            mode,
            _lock: lock,
        })
    }
}

pub(crate) fn remove_entry(path: &Path) -> Result<(), HostError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub(crate) fn write_json(path: &Path, value: &impl Serialize) -> Result<(), HostError> {
    let temp = path.with_extension("json.tmp");
    #[cfg(unix)]
    let mut file = fs::File::create(&temp)?;
    #[cfg(windows)]
    let mut file = watane_log_windows_fs::open_or_create_private_file(&temp)?;
    #[cfg(windows)]
    file.set_len(0)?;
    file.write_all(&serde_json::to_vec(value)?)?;
    file.sync_all()?;
    drop(file);
    #[cfg(unix)]
    {
        fs::rename(temp, path)?;
        fs::File::open(path.parent().expect("home file has parent"))?.sync_all()?
    };
    #[cfg(windows)]
    watane_log_windows_fs::publish(&temp, path, true)?;
    Ok(())
}
