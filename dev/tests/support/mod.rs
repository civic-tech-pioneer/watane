//! One cache at its original path: copying changes Cargo's rustflags fingerprint.
use std::{path::Path, process::Command, sync::OnceLock};

pub fn cache() -> &'static Path {
    static WARM: OnceLock<tempfile::TempDir> = OnceLock::new();
    WARM.get_or_init(|| {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("counter");
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
            .env("WATANE_DEV_CACHE", temp.path())
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
    .path()
}
