//! One cache at its original path: copying changes Cargo's rustflags fingerprint.
#![allow(clippy::unwrap_used, reason = "tests may panic")]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub fn cache() -> &'static Path {
    // A `static` is never dropped, so the cache cannot live in `$TMPDIR`: it sits under the
    // target folder and is removed by `cargo clean`. Every dev test binary shares it and keeps
    // it between runs; a rebuilt watane-dev refreshes it, keeping `target/`. Each process holds a shared lock while it
    // lives; a rebuild takes the exclusive one, so it never removes a cache in use.
    static WARM: OnceLock<(PathBuf, std::fs::File)> = OnceLock::new();
    &WARM.get_or_init(warm).0
}

fn warm() -> (PathBuf, std::fs::File) {
    let temp = Path::new(env!("CARGO_TARGET_TMPDIR")).join("watane-dev-warm");
    let lock = std::fs::File::create(temp.with_extension("lock")).unwrap();
    lock.lock_shared().unwrap();
    let binary = env!("CARGO_BIN_EXE_watane-dev");
    let meta = std::fs::metadata(binary).unwrap();
    let stamp = format!("{:?} {}", meta.modified().unwrap(), meta.len());
    let ready = temp.join("ready");
    if std::fs::read_to_string(&ready).is_ok_and(|r| r == stamp) {
        return (temp, lock);
    }
    lock.unlock().unwrap();
    lock.lock().unwrap();
    if std::fs::read_to_string(&ready).is_ok_and(|r| r == stamp) {
        lock.unlock().unwrap();
        lock.lock_shared().unwrap();
        return (temp, lock);
    }
    // Keep the compiled crates in `target/`: Cargo's fingerprints decide what a new watane-dev
    // must rebuild there, so a relink of watane-dev is not a cold template build (~180 s).
    for e in std::fs::read_dir(&temp).into_iter().flatten().flatten() {
        if e.file_name() != "target" {
            let p = e.path();
            let _ = std::fs::remove_dir_all(&p).or_else(|_| std::fs::remove_file(&p));
        }
    }
    std::fs::create_dir_all(&temp).unwrap();
    let dir = temp.join("counter");
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
    std::fs::write(&ready, stamp).unwrap();
    lock.unlock().unwrap();
    lock.lock_shared().unwrap();
    (temp, lock)
}

/// The path token expected in a shell recovery command.
pub fn shell_path(path: impl AsRef<Path>) -> String {
    let path = path.as_ref().to_string_lossy();
    if path.contains(['\\', ' ', '\'']) {
        format!("'{}'", path.replace('\'', "'\"'\"'"))
    } else {
        path.into_owned()
    }
}
