#![allow(clippy::unwrap_used, reason = "tests may panic")]

use crate::support;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use support::*;
use watane_net::{Net, NetError, Options};

fn open(dir: &Path) -> Result<Net, NetError> {
    Net::open(dir, MemHoldings::empty(), Options::default())
}
#[cfg(unix)]
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
        #[cfg(unix)] // Unix needs mode 0600 on the fixture key.
        set_mode(&dir.join("node.key"), 0o600);
        assert!(matches!(open(&dir), Err(NetError::Corrupt { .. })), "{len}");
        assert_eq!(fs::read(dir.join("node.key")).unwrap(), bytes);
    }
}

#[cfg(unix)] // Exposes the key through POSIX mode bits.
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

#[cfg(unix)] // Exposes the directory mode and creates a POSIX symlink.
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

#[cfg(windows)] // Uses a Windows owner-only DACL and the built-in ACL editor.
#[test]
fn windows_key_reopens_and_refuses_an_exposed_acl() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join("net");
    let key = open(&dir).unwrap().node_key();
    let path = dir.join("node.key");
    let before = fs::read(&path).unwrap();
    assert_eq!(open(&dir).unwrap().node_key(), key);
    let status = std::process::Command::new("icacls.exe")
        .arg(&path)
        .args(["/grant", "*S-1-1-0:(R)"])
        .output()
        .unwrap();
    assert!(status.status.success(), "{status:?}");
    let Err(NetError::Corrupt { cause, .. }) = open(&dir) else {
        panic!("exposed key must be refused");
    };
    assert!(cause.contains("owner-only Windows ACL"), "{cause}");
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[cfg(windows)] // Checks the Windows owner-only DACL of the blob store.
#[test]
fn windows_blob_store_is_private_and_an_exposed_one_is_refused() {
    use watane_log_windows_fs as winfs;

    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join("net");
    let options = Options {
        on: true,
        bind: Some(lo()),
        ..Options::default()
    };
    let net = open(&dir).unwrap();
    net.set_options(options.clone()).unwrap();
    drop(net);
    let blobs = dir.join("blobs");
    winfs::check_owner_only(&winfs::open_path(&blobs, false).unwrap()).unwrap();
    let output = std::process::Command::new("icacls.exe")
        .arg(&blobs)
        .args(["/grant", "*S-1-1-0:(R)"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let error = Net::open(&dir, MemHoldings::empty(), options).unwrap_err();
    assert!(
        error.to_string().contains("owner-only Windows ACL"),
        "{error:?}"
    );
}

#[cfg(windows)] // A Windows hard link shares the key's DACL.
#[test]
fn windows_a_linked_key_is_corrupt() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join("net");
    open(&dir).unwrap();
    fs::hard_link(dir.join("node.key"), home.path().join("copy")).unwrap();
    let Err(NetError::Corrupt { cause, .. }) = open(&dir) else {
        panic!("hard-linked key must be refused");
    };
    assert!(cause.contains("regular file with one link"), "{cause}");
}
