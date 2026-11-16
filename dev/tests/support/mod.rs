//! One cache at its original path: copying changes Cargo's rustflags fingerprint.
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

pub fn cache() -> &'static Path {
    // A `static` is never dropped, so the cache cannot live in `$TMPDIR`: it sits
    // under the target folder, is rebuilt at first use in each run and removed by `cargo clean`.
    static WARM: OnceLock<PathBuf> = OnceLock::new();
    WARM.get_or_init(|| {
        let temp = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(concat!("watane-dev-warm-", env!("CARGO_CRATE_NAME")));
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();
        let dir = temp.join("counter");
        let binary = env!("CARGO_BIN_EXE_watane-dev");
        let new = Command::new(binary)
            .args(["new", "counter", "--json", "--dir"])
            .arg(&dir)
            .output()
            .unwrap();
        assert!(
            new.status.success(),
            "{}",
            String::from_utf8_lossy(&new.stdout)
        );
        let built = Command::new(binary)
            .current_dir(&dir)
            .env("WATANE_DEV_CACHE", &temp)
            .env_remove("WATANE_DEV_UNIT_SECONDS")
            .args(["test", "--json"])
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&built.stdout),
            String::from_utf8_lossy(&built.stderr)
        );
        temp
    })
}
