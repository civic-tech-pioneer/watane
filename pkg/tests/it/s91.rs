//! S91 acceptance regressions: `add_provider` names a `.wit` file swapped for a directory
//! between its listing and its read as a `path` cause (TECHNICAL 4.8, 6.4).
#![cfg(any(target_os = "linux", target_os = "macos"))]
#![allow(clippy::unwrap_used, reason = "tests may panic")]

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use watane_pkg::{Mod, ModError, Rule, add_provider};

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let p = e.unwrap().path();
        let dest = to.join(p.file_name().unwrap());
        if p.is_dir() {
            copy(&p, &dest);
        } else {
            fs::copy(p, dest).unwrap();
        }
    }
}
fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// The consumer's `wit/deps/watane-host/host.wit` is atomically exchanged with a directory
/// (rename exchange: the name always exists, as a file or as a directory) while
/// `add_provider` compares it with the provider's. When the listing saw a file and the read
/// finds a directory, the cause must be rule `path` (CHANGED, or "expected a regular file,
/// found a directory"), never rule `limit` "expected a regular file" and never `Io`.
/// Seen through `watane-dev add` as `[limit] expected a regular file` (S91 acceptance).
#[test]
#[ignore = "slow: a real rename-exchange race checks for windows the add.rs hook tests do not stage"]
fn regress_s91_add_provider_wit_file_swapped_for_directory() {
    let p = tempfile::tempdir().unwrap();
    copy(&fixture("reading-list"), p.path());
    let provider = Mod::read(p.path()).unwrap();
    let c = tempfile::tempdir().unwrap();
    copy(&fixture("hello"), c.path());
    copy(
        &p.path().join("wit/deps/watane-host"),
        &c.path().join("wit/deps/watane-host"),
    );
    let file = c.path().join("wit/deps/watane-host/host.wit");
    let other = c.path().join("swap");
    fs::create_dir(&other).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let swapper = {
        let (stop, file, other) = (Arc::clone(&stop), file.clone(), other.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                rustix::fs::renameat_with(
                    rustix::fs::CWD,
                    &file,
                    rustix::fs::CWD,
                    &other,
                    rustix::fs::RenameFlags::EXCHANGE,
                )
                .unwrap();
            }
        })
    };
    let start = Instant::now();
    let mut bad = None;
    while bad.is_none() && start.elapsed() < Duration::from_secs(5) {
        match add_provider(c.path(), &provider, None) {
            Ok(()) => {}
            Err(ModError::Invalid(v)) => {
                if let Some(v) = v.into_iter().find(|v| v.rule != Rule::Path) {
                    bad = Some(format!("{:?} {} {}", v.rule, v.file, v.message));
                }
            }
            Err(e) => bad = Some(format!("not a named cause: {e:?}")),
        }
    }
    stop.store(true, Ordering::Relaxed);
    swapper.join().unwrap();
    assert_eq!(bad, None);
}
