//! The environment dev runs in: the cache folder and the unit test time limit
//! (TECHNICAL 3).

use crate::outcome::Problem;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::PathBuf;

thread_local! {
    /// The variables of an in-process run (`cli::main_with`): `None` unsets one.
    static VARS: RefCell<HashMap<String, Option<OsString>>> = RefCell::new(HashMap::new());
}
/// An environment variable as dev reads it: the in-process run's value
/// (2.5) first, then the process's own.
pub fn var_os(key: &str) -> Option<OsString> {
    VARS.with_borrow(|vars| vars.get(key).cloned())
        .unwrap_or_else(|| std::env::var_os(key))
}
/// Runs `f` with `vars` set (`Some`) or unset (`None`) for `var_os` on this
/// thread, then restores the previous values.
pub(crate) fn with_vars<T>(vars: &[(&str, Option<&OsStr>)], f: impl FnOnce() -> T) -> T {
    let saved = VARS.with_borrow_mut(|map| {
        let saved = map.clone();
        for (key, value) in vars {
            map.insert((*key).into(), value.map(OsStr::to_os_string));
        }
        saved
    });
    struct Restore(Option<HashMap<String, Option<OsString>>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            if let Some(saved) = self.0.take() {
                VARS.with_borrow_mut(|map| *map = saved);
            }
        }
    }
    let _restore = Restore(Some(saved));
    f()
}
#[derive(Debug)]
pub struct Env {
    pub cache: PathBuf,
    pub unit_seconds: u64,
}
impl Env {
    pub fn from_process() -> Result<Self, Problem> {
        let cache = if let Some(p) = var_os("WATANE_DEV_CACHE") {
            PathBuf::from(p)
        } else {
            let home = var_os("HOME")
                .map(PathBuf::from)
                .ok_or_else(|| Problem::internal("HOME is unset; set WATANE_DEV_CACHE"))?;
            if cfg!(target_os = "macos") {
                home.join("Library/Caches/watane-dev")
            } else {
                var_os("XDG_CACHE_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join(".cache"))
                    .join("watane-dev")
            }
        };
        fs::create_dir_all(&cache).map_err(Problem::internal)?;
        for folder in ["cargo-home", "tmp", "build", "target"] {
            fs::create_dir_all(cache.join(folder)).map_err(Problem::internal)?;
        }
        Ok(Self {
            cache: cache.canonicalize().map_err(Problem::internal)?,
            unit_seconds: var_os("WATANE_DEV_UNIT_SECONDS")
                .and_then(|s| s.to_str()?.parse().ok())
                .unwrap_or(60),
        })
    }
}
