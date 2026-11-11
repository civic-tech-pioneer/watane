use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};
use watane_host::{Host, Limits};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mods")
        .join(name)
}

fn page() -> tempfile::TempDir {
    let folder = tempfile::tempdir().unwrap();
    for file in ["mod.toml", "wit/world.wit", "ui/index.html", "ui/app.js"] {
        let dest = folder.path().join(file);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::copy(fixture("hello").join(file), dest).unwrap();
    }
    folder
}

fn catalog(home: &Path) -> Value {
    serde_json::from_slice(&fs::read(home.join("installed.json")).unwrap()).unwrap()
}

#[test]
fn ac1_installed_and_persisted_ids_match_pack_for_both_parts() {
    for name in ["counter", "hello"] {
        let folder = fixture(name);
        let expected = watane_pkg::pack(&folder, false).unwrap().id();
        let home = tempfile::tempdir().unwrap();
        let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
        let installed = host.install_folders(&[folder], 0).unwrap();
        assert_eq!(installed[0].package, expected);
        assert_eq!(
            catalog(home.path())["mods"][0]["package"],
            expected.to_string()
        );
        drop(host);
        let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
        let served = host.install_folders(&[fixture(name)], 0).unwrap();
        assert_eq!(served[0].package, expected);
        assert_eq!(catalog(home.path())["generation"], 1);
    }
}

#[test]
fn ac2_pack_skipped_files_are_absent_from_install() {
    let folder = page();
    for file in [
        ".git/x",
        ".hidden",
        "ui/.hidden",
        "logic/target/x",
        "logic/src/lib.rs",
    ] {
        let path = folder.path().join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "excluded").unwrap();
    }
    let expected = watane_pkg::pack(folder.path(), false).unwrap().id();
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
    let installed = host.install_folders(&[folder.path().into()], 0).unwrap();
    for file in [".git", ".hidden", "ui/.hidden", "logic"] {
        assert!(
            !home.path().join("mods/hello").join(file).exists(),
            "{file}"
        );
    }
    assert_eq!(installed[0].package, expected);
}

#[test]
fn ac3_unchanged_keeps_generation_one_byte_updates_package() {
    let folder = page();
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
    let folders = [folder.path().into()];
    let first = host.install_folders(&folders, 0).unwrap();
    let before = fs::read(home.path().join("installed.json")).unwrap();
    let same = host.install_folders(&folders, 0).unwrap();
    assert_eq!(same[0].package, first[0].package);
    assert_eq!(
        fs::read(home.path().join("installed.json")).unwrap(),
        before
    );
    let path = folder.path().join("ui/index.html");
    let mut bytes = fs::read(&path).unwrap();
    bytes[0] ^= 1;
    fs::write(path, bytes).unwrap();
    let updated = host.install_folders(&folders, 0).unwrap();
    assert_ne!(updated[0].package, first[0].package);
    assert_eq!(
        updated[0].package,
        watane_pkg::pack(folder.path(), false).unwrap().id()
    );
    assert_eq!(catalog(home.path())["generation"], 2);
}

#[test]
fn ac4_package_errors_name_input_and_leave_no_staging() {
    for symlink in [true, false] {
        let folder = page();
        if symlink {
            std::os::unix::fs::symlink("index.html", folder.path().join("ui/link")).unwrap();
        } else {
            fs::File::create(folder.path().join("ui/large"))
                .unwrap()
                .set_len(watane_pkg::MAX_PACKAGE_BYTES + 1)
                .unwrap();
        }
        let expected = watane_pkg::pack(folder.path(), false)
            .unwrap_err()
            .to_string();
        let home = tempfile::tempdir().unwrap();
        let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
        let error = host
            .install_folders(&[fixture("hello"), folder.path().into()], 0)
            .unwrap_err();
        assert_eq!(error.code(), "invalid_mod");
        match error {
            watane_host::HostError::InvalidMod { input, reason } => {
                assert_eq!(input, folder.path().display().to_string());
                assert_eq!(reason, expected);
            }
            other => panic!("{other}"),
        }
        assert_eq!(
            fs::read_dir(home.path().join("staging")).unwrap().count(),
            0
        );
        assert_eq!(fs::read_dir(home.path().join("mods")).unwrap().count(), 0);
    }
}
