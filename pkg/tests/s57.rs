#![cfg(unix)]
use std::{fs, path::Path};
use watane_pkg::{Mod, ModError, PackageError, Rule, pack};

const LINK_ENTRY: &str = "a symbolic link; replace it with a regular file or directory";
const LINK_FILE: &str = "a symbolic link; replace it with a regular file";
const SPECIAL: &str =
    "not a regular file or a directory (a FIFO, socket or device); replace or remove it";

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
fn hello() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hello"),
        dir.path(),
    );
    dir
}
fn violations(dir: &Path) -> Vec<(Rule, String, String)> {
    match Mod::read(dir) {
        Err(ModError::Invalid(v)) => v.into_iter().map(|v| (v.rule, v.file, v.message)).collect(),
        other => panic!("expected invalid, got {other:?}"),
    }
}
fn mkfifo(path: &Path) {
    assert!(
        std::process::Command::new("mkfifo")
            .arg(path)
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn ac6_s57_socket_at_mod_toml() {
    let dir = hello();
    fs::remove_file(dir.path().join("mod.toml")).unwrap();
    let _socket = std::os::unix::net::UnixListener::bind(dir.path().join("mod.toml")).unwrap();
    assert!(violations(dir.path()).contains(&(
        Rule::ModToml,
        "mod.toml".into(),
        "expected a regular file".into()
    )));
}
#[test]
fn ac7_s57_link_and_fifo_texts_agree() {
    let dir = hello();
    fs::create_dir_all(dir.path().join("ui")).unwrap();
    std::os::unix::fs::symlink("index.html", dir.path().join("ui/link")).unwrap();
    mkfifo(&dir.path().join("ui/fifo"));
    let v = violations(dir.path());
    assert!(
        v.contains(&(Rule::Path, "ui/link".into(), LINK_ENTRY.into())),
        "{v:?}"
    );
    assert!(
        v.contains(&(Rule::Path, "ui/fifo".into(), SPECIAL.into())),
        "{v:?}"
    );
    // pack stops at the first entry in name order: the FIFO, then the link.
    fs::remove_file(dir.path().join("ui/fifo")).unwrap();
    match pack(dir.path(), false) {
        Err(e @ PackageError::Symlink { .. }) => {
            assert_eq!(e.to_string(), format!("`ui/link`: {LINK_ENTRY}"));
        }
        other => panic!("{other:?}"),
    }
    fs::remove_file(dir.path().join("ui/link")).unwrap();
    mkfifo(&dir.path().join("ui/fifo"));
    match pack(dir.path(), false) {
        Err(e @ PackageError::Path { .. }) => assert!(e.to_string().ends_with(SPECIAL), "{e}"),
        other => panic!("{other:?}"),
    }
}
#[test]
fn ac7_s57_mod_toml_link() {
    let dir = hello();
    fs::remove_file(dir.path().join("mod.toml")).unwrap();
    std::os::unix::fs::symlink("missing", dir.path().join("mod.toml")).unwrap();
    let v = violations(dir.path());
    assert!(
        v.contains(&(Rule::Path, "mod.toml".into(), LINK_FILE.into())),
        "{v:?}"
    );
    match pack(dir.path(), false) {
        Err(e @ PackageError::Symlink { .. }) => {
            assert_eq!(e.to_string(), format!("`mod.toml`: {LINK_FILE}"));
        }
        other => panic!("{other:?}"),
    }
}
// An entry listed by readdir and gone before its no-follow stat is a mid-read change too:
// today `open::entries` returns the stat's NotFound as `ModError::Io` naming the parent
// ("<mod>/ui: No such file or directory"), though `ui` is there.
#[test]
fn regress_s57_entry_gone_between_list_and_stat() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let dir = hello();
    let ui = dir.path().join("ui");
    fs::create_dir_all(&ui).unwrap();
    for i in 0..200 {
        fs::write(ui.join(format!("f{i:03}.txt")), "x").unwrap();
    }
    let stop = Arc::new(AtomicBool::new(false));
    let churn = {
        let stop = stop.clone();
        let ui = ui.clone();
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                for i in 0..200 {
                    let f = ui.join(format!("f{i:03}.txt"));
                    let _ = fs::remove_file(&f);
                    let _ = fs::write(&f, "x");
                }
            }
        })
    };
    let mut io = None;
    for _ in 0..2000 {
        match Mod::read(dir.path()) {
            Err(e @ ModError::Io { .. }) => {
                io = Some(e.to_string());
                break;
            }
            Err(ModError::Invalid(v)) => {
                for v in v {
                    assert_eq!(v.rule, Rule::Path, "{v:?}");
                    assert_eq!(v.message, "changed while the mod was read; read it again");
                }
            }
            Ok(_) => {}
        }
    }
    stop.store(true, Ordering::Relaxed);
    churn.join().unwrap();
    assert_eq!(io, None, "a mid-read change reads as plain I/O");
}
