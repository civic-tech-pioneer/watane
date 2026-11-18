use std::fs;
use std::path::{Path, PathBuf};
use watane_pkg::*;
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}
use crate::support;
#[cfg(unix)]
#[test]
fn ac1_s22_hidden_links() {
    for (path, target) in [
        ("ui/.git/x", "../index.html"),
        ("ui/.a/.b/y", "/etc/passwd"),
        ("wit/.git/x", "../world.wit"),
        ("ui/.d", "."),
    ] {
        let dir = support::built_fixture("hello");
        let link = dir.path().join(path);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(target, link).unwrap();
        assert!(
            matches!(pack(dir.path(), false), Err(PackageError::Symlink { path: p }) if p == path),
            "{path}"
        );
    }
}
#[test]
fn ac1_s22_hidden_regular_files_omitted() {
    let dir = support::hello();
    let before = pack(dir.path(), false).unwrap();
    fs::create_dir_all(dir.path().join("ui/.a/.b")).unwrap();
    fs::write(dir.path().join("ui/.a/.b/file"), "secret").unwrap();
    let after = pack(dir.path(), false).unwrap();
    assert_eq!(before.as_bytes(), after.as_bytes());
    assert_eq!(before.id(), after.id());
}
#[test]
fn ac4_s22_committed_fixtures_pack() {
    for name in ["reading-list", "recommendations"] {
        let dir = fixture(name);
        let package = pack(&dir, false).unwrap();
        if name == "recommendations" {
            assert!(
                matches!(Release::new(&Mod::read(&dir).unwrap(), &package), Err(ReleaseError::LocalProvider { slug }) if slug.as_str() == "reading-list")
            );
        }
    }
}
const GOLDEN: &str = concat!(
    "8c016568656c6c6f65302e312e306548656c6c6f6b536179732068656c6c6f2e5820",
    "0866e9df433265eaf0d959c589d82f1c2020d5b07faf7a530b012ec9f62ff87b",
    "189ff4808080f6"
);
#[test]
fn ac5_s22_disk_hello_golden() {
    let dir = fixture("hello");
    let package = pack(&dir, false).unwrap();
    assert_eq!(
        package.id().to_string(),
        "0866e9df433265eaf0d959c589d82f1c2020d5b07faf7a530b012ec9f62ff87b"
    );
    let release = Release::new(&Mod::read(&dir).unwrap(), &package).unwrap();
    assert_eq!(
        release.encode(),
        data_encoding::HEXLOWER.decode(GOLDEN.as_bytes()).unwrap()
    );
    assert_eq!(release.encode().len(), 73);
}
#[test]
fn ac6_s22_release_cause_once() {
    let golden = data_encoding::HEXLOWER.decode(GOLDEN.as_bytes()).unwrap();
    for bytes in [&[][..], &golden[..10]] {
        let message = Release::decode(bytes).unwrap_err().to_string();
        assert_eq!(message.matches("release").count(), 1, "{message}");
        assert!(message.contains("end of input"), "{message}");
    }
}

#[test]
fn ac6_s22_other_repeated_contexts() {
    let golden = data_encoding::HEXLOWER.decode(GOLDEN.as_bytes()).unwrap();
    for (offset, byte, word) in [
        (0, 0x8d, "release"),
        (3, b'H', "slug"),
        (9, b'x', "version"),
    ] {
        let mut bytes = golden.clone();
        bytes[offset] = byte;
        let message = Release::decode(&bytes).unwrap_err().to_string();
        assert_eq!(message.matches(word).count(), 1, "{message}");
    }
}

#[test]
fn s22_single_file_deps_are_separate_packages() {
    // 6.1 allows wit/deps/<s>.wit; each such file is its own package, as push_dir read it.
    let dir = support::hello();
    for (file, text) in [
        ("wit/deps/a.wit", "package wasi:a@0.1.0;\ninterface x {}\n"),
        ("wit/deps/b.wit", "package wasi:b@0.1.0;\ninterface y {}\n"),
    ] {
        fs::create_dir_all(dir.path().join("wit/deps")).unwrap();
        fs::write(dir.path().join(file), text).unwrap();
    }
    Mod::read(dir.path()).unwrap();
}
