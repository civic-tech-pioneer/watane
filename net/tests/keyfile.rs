mod support;
use std::{fs, os::unix::fs::PermissionsExt, path::Path};
use support::*;
use watane_net::{Net, NetError, Options};

fn open(dir: &Path) -> Result<Net, NetError> {
    Net::open(dir, MemHoldings::empty(), Options::default())
}
fn set_mode(p: &Path, mode: u32) {
    fs::set_permissions(p, fs::Permissions::from_mode(mode)).unwrap();
}

#[test]
fn a_key_file_of_the_wrong_length_is_corrupt_and_not_changed() {
    for len in [0usize, 31, 33, 64] {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join("net");
        open(&dir).unwrap();
        let bytes = vec![7u8; len];
        fs::write(dir.join("node.key"), &bytes).unwrap();
        set_mode(&dir.join("node.key"), 0o600);
        assert!(matches!(open(&dir), Err(NetError::Corrupt { .. })), "{len}");
        assert_eq!(fs::read(dir.join("node.key")).unwrap(), bytes);
    }
}

#[test]
fn a_key_file_others_can_read_is_corrupt_and_not_changed() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join("net");
    open(&dir).unwrap();
    let before = fs::read(dir.join("node.key")).unwrap();
    set_mode(&dir.join("node.key"), 0o644);
    assert!(matches!(open(&dir), Err(NetError::Corrupt { .. })));
    assert_eq!(fs::read(dir.join("node.key")).unwrap(), before);
    assert_eq!(
        fs::metadata(dir.join("node.key"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o644
    );
}

#[test]
fn a_directory_others_can_enter_or_a_linked_key_is_corrupt() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join("net");
    open(&dir).unwrap();
    set_mode(&dir, 0o755);
    assert!(matches!(open(&dir), Err(NetError::Corrupt { .. })));
    set_mode(&dir, 0o700);
    let real = home.path().join("elsewhere");
    fs::rename(dir.join("node.key"), &real).unwrap();
    std::os::unix::fs::symlink(&real, dir.join("node.key")).unwrap();
    assert!(matches!(open(&dir), Err(NetError::Corrupt { .. })));
}
