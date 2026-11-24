//! Windows identity storage tests for TECHNICAL 4.7.

use std::fs;
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
    // Use Windows' ACL reporting independently of the production Win32 wrapper.
    let result = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            r"
            $ErrorActionPreference = 'Stop'
            $sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
            $acl = Get-Acl -LiteralPath $env:WATANE_TEST_KEY_PATH
            $sidType = [System.Security.Principal.SecurityIdentifier]
            $rules = $acl.GetAccessRules($true, $true, $sidType)
            if ($acl.GetOwner($sidType).Value -ne $sid -or
                -not $acl.AreAccessRulesProtected -or $rules.Count -ne 1 -or
                $rules[0].IdentityReference.Value -ne $sid -or
                $rules[0].AccessControlType -ne 'Allow' -or $rules[0].IsInherited) {
                throw 'identity.key must have a protected owner-only ACL'
            }
        ",
        ])
        .env("WATANE_TEST_KEY_PATH", &path)
        .output()
        .expect("inspect the key ACL with PowerShell");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
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
