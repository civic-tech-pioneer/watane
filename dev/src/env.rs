use crate::outcome::Problem;
use std::fs;
use std::path::PathBuf;
#[derive(Debug)]
pub struct Env {
    pub cache: PathBuf,
    pub unit_seconds: u64,
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
        for folder in ["cargo-home", "tmp", "build", "target"] {
            fs::create_dir_all(cache.join(folder)).map_err(Problem::internal)?;
        }
        Ok(Self {
            cache: cache.canonicalize().map_err(Problem::internal)?,
            unit_seconds: std::env::var("WATANE_DEV_UNIT_SECONDS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(60),
        })
    }
}
