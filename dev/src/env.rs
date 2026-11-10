use crate::outcome::Problem;
use std::{fs, path::PathBuf};
pub struct Env {
    pub cache: PathBuf,
}
impl Env {
    pub fn from_process() -> Result<Self, Problem> {
        let cache = if let Some(p) = std::env::var_os("WATANE_DEV_CACHE") {
            PathBuf::from(p)
        } else {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .ok_or_else(|| Problem::internal("HOME is unset; set WATANE_DEV_CACHE"))?;
            if cfg!(target_os = "macos") {
                home.join("Library/Caches/watane-dev")
            } else {
                std::env::var_os("XDG_CACHE_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join(".cache"))
                    .join("watane-dev")
            }
        };
        fs::create_dir_all(&cache).map_err(Problem::internal)?;
        Ok(Self {
            cache: cache.canonicalize().map_err(Problem::internal)?,
        })
    }
}
