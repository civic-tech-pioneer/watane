//! Windows identity storage tests for TECHNICAL 4.7.

use std::fs;
use std::path::Path;
use std::process::Command;
use watane_log::{Error, Log};

#[test]
fn s110_key_is_private_and_reopens() {
    let temp = tempfile::tempdir().expect("create temporary parent");
    let dir = temp.path().join("fresh/nested");
    let log = Log::open(&dir).expect("create private identity");
    let path = dir.join("identity.key");
    assert_eq!(fs::metadata(&path).expect("stat key").len(), 32);
    assert_eq!(
        Log::open(&dir).expect("reopen private identity").identity(),
        log.identity()
    );
    assert_private_acl(&path, &temp.path().join("key.acl"), false);
}

fn assert_private_acl(path: &Path, acl_path: &Path, directory: bool) {
    let user = Command::new("whoami.exe")
        .args(["/user", "/fo", "csv", "/nh"])
        .output()
        .expect("query the current user's SID");
    assert!(user.status.success(), "{user:?}");
    // The SID is ASCII and is the last CSV field, even for a localized account name.
    let stdout = String::from_utf8_lossy(&user.stdout);
    let (_, sid) = stdout.trim().rsplit_once(',').expect("whoami CSV fields");
    let sid = sid.trim_matches('"');
    assert!(sid.starts_with("S-1-"), "invalid account SID");

    let result = Command::new("icacls.exe")
        .arg(path)
        .arg("/save")
        .arg(acl_path)
        .output()
        .expect("save the DACL using icacls");
    assert!(result.status.success(), "{path:?}: {result:?}");

    // icacls saves only the DACL. Decode its SDDL with .NET and check the owner
    // separately. No PowerShell modules are needed.
    let result = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            r"
            $ErrorActionPreference = 'Stop'
            $sid = $env:WATANE_TEST_USER_SID
            $lines = [System.IO.File]::ReadAllLines($env:WATANE_TEST_ACL_PATH, [System.Text.Encoding]::Unicode)
            if ($lines.Count -ne 2 -or -not $lines[1].StartsWith('D:')) {
                throw 'icacls must save exactly one DACL'
            }
            $descriptor = [System.Security.AccessControl.RawSecurityDescriptor]::new($lines[1])
            $dacl = $descriptor.DiscretionaryAcl
            $protected = [System.Security.AccessControl.ControlFlags]::DiscretionaryAclProtected
            $flags = [System.Security.AccessControl.AceFlags]::None
            if ($env:WATANE_TEST_IS_DIRECTORY -eq 'true') {
                $flags = [System.Security.AccessControl.AceFlags]::ObjectInherit -bor
                    [System.Security.AccessControl.AceFlags]::ContainerInherit
                $acl = [System.IO.Directory]::GetAccessControl($env:WATANE_TEST_PRIVATE_PATH)
            } else {
                $acl = [System.IO.File]::GetAccessControl($env:WATANE_TEST_PRIVATE_PATH)
            }
            if (-not ($descriptor.ControlFlags -band $protected) -or
                $dacl.Count -ne 1 -or
                $dacl[0].AceType -ne 'AccessAllowed' -or
                $dacl[0].SecurityIdentifier.Value -ne $sid -or
                $dacl[0].AceFlags -ne $flags) {
                throw 'The path must have a protected owner-only DACL with the expected ACE flags'
            }
            if ($acl.GetOwner([System.Security.Principal.SecurityIdentifier]).Value -ne $sid) {
                throw 'The path must be owned by the current user'
            }
        ",
        ])
        .env("WATANE_TEST_USER_SID", sid)
        .env("WATANE_TEST_ACL_PATH", acl_path)
        .env("WATANE_TEST_PRIVATE_PATH", path)
        .env("WATANE_TEST_IS_DIRECTORY", directory.to_string())
        .output()
        .expect("check the saved DACL and live owner");
    assert!(
        result.status.success(),
        "{path:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn s116_private_dir_creates_and_pins_a_checked_directory() {
    use watane_log_windows_fs::{PrivateDirError, private_dir};

    let temp = tempfile::tempdir().expect("create temporary parent");
    let parent = temp.path().join("fresh");
    let dir = parent.join("nested");
    let directory = private_dir(&dir).expect("create and open a private directory");
    drop(private_dir(&parent).expect("check the intermediate directory"));
    assert!(directory.metadata().expect("inspect directory").is_dir());
    let moved = dir.with_file_name("moved");
    fs::rename(&dir, &moved).expect_err("the open directory must be pinned");
    drop(directory);
    assert_private_acl(&parent, &temp.path().join("parent.acl"), true);
    assert_private_acl(&dir, &temp.path().join("nested.acl"), true);
    fs::rename(&dir, &moved).expect("move directory after releasing its handle");

    let file = moved.join("regular-file");
    fs::write(&file, b"regular file").expect("create a regular file");
    assert!(matches!(
        private_dir(&file),
        Err(PrivateDirError::Invalid(_))
    ));

    let result = Command::new("icacls.exe")
        .arg(&moved)
        .args(["/grant", "*S-1-1-0:(R)"])
        .output()
        .expect("expose directory to Everyone");
    assert!(result.status.success(), "{result:?}");
    assert!(matches!(
        private_dir(&moved),
        Err(PrivateDirError::Invalid(_))
    ));
}

#[test]
fn s110_multiple_links_to_the_key_are_refused() {
    let temp = crate::oracle::directory();
    let dir = temp.path();
    drop(Log::open(dir).expect("create private identity"));
    let path = dir.join("identity.key");
    let alias = dir.join("other.key");
    fs::hard_link(&path, &alias).expect("give identity.key two names");
    let error = Log::open(dir).expect_err("refuse a multiply linked identity");
    assert!(matches!(error, Error::KeyLinks { links: 2, .. }));
}

#[test]
fn s110_simultaneous_first_opens_share_the_published_identity() {
    let temp = crate::oracle::directory();
    let dir = temp.path();
    let start = std::sync::Barrier::new(8);
    std::thread::scope(|scope| {
        let openers: Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    start.wait();
                    Log::open(dir).expect("open identity under lock").identity()
                })
            })
            .collect();
        let mut identities = openers.into_iter().map(|open| open.join().expect("worker"));
        let first = identities.next().expect("at least one opener");
        assert!(identities.all(|identity| identity == first));
    });
}

#[test]
fn s110_key_reparse_point_is_refused() {
    let temp = crate::oracle::directory();
    let dir = temp.path();
    fs::create_dir(dir.join("target")).expect("create junction target");
    fs::write(dir.join("target/marker"), b"untouched").expect("write target marker");
    let result = Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J", "identity.key", "target"])
        .current_dir(dir)
        .output()
        .expect("create NTFS junction");
    assert!(result.status.success(), "{result:?}");
    let error = Log::open(dir).expect_err("refuse reparse point");
    assert!(error.to_string().contains("reparse point"), "{error}");
    assert_eq!(
        fs::read(dir.join("target/marker")).expect("read marker"),
        b"untouched"
    );
}

#[test]
fn s110_exposed_acl_is_refused_without_touching_key() {
    let temp = crate::oracle::directory();
    let dir = temp.path();
    drop(Log::open(dir).expect("create private identity"));
    let path = dir.join("identity.key");
    let before = fs::read(&path).expect("read seed");
    let result = Command::new("icacls.exe")
        .arg(&path)
        .args(["/grant", "*S-1-1-0:(R)"])
        .output()
        .expect("expose key to Everyone");
    assert!(result.status.success(), "{result:?}");
    let error = Log::open(dir).expect_err("refuse exposed identity");
    assert!(matches!(error, Error::Io { .. }));
    assert!(
        error.to_string().contains("owner-only Windows ACL"),
        "{error}"
    );
    assert_eq!(fs::read(&path).expect("read unchanged seed"), before);
}

#[test]
fn s110_exposed_directory_refuses_key_creation() {
    let temp = crate::oracle::directory();
    let dir = temp.path();
    let result = Command::new("icacls.exe")
        .arg(dir)
        .args(["/grant", "*S-1-1-0:(R)"])
        .output()
        .expect("expose directory to Everyone");
    assert!(result.status.success(), "{result:?}");
    let error = Log::open(dir).expect_err("refuse exposed directory");
    assert!(
        error.to_string().contains("owner-only Windows ACL"),
        "{error}"
    );
    assert!(!dir.join("identity.key").exists());
}
