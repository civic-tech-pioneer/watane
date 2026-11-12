mod oracle;
use watane_log::{Error, Log};
#[test]
fn ac7_restore_requires_own_head_confirmation() {
    let d = oracle::directory();
    let first = Log::open(d.path()).unwrap();
    let entries: Vec<_> = (0..3).map(|_| first.append("a", b"").unwrap()).collect();
    let backup = first.export_identity("pass").unwrap();
    let dest = oracle::directory();
    let l = Log::open(dest.path()).unwrap();
    assert!(l.own_head_confirmed().unwrap());
    l.restore_identity(&backup, "pass").unwrap();
    drop(l);
    let l = Log::open(dest.path()).unwrap();
    assert!(!l.own_head_confirmed().unwrap());
    assert!(matches!(l.append("a", b""), Err(Error::OwnHeadUnconfirmed)));
    l.ingest(&l.identity(), &entries).unwrap();
    l.confirm_own_head().unwrap();
    assert!(l.own_head_confirmed().unwrap());
    let e = l.append("a", b"").unwrap();
    assert_eq!(e.seq(), 3);
    assert_eq!(e.prev(), Some(entries[2].id()));
}
