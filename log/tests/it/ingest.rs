#![allow(clippy::unwrap_used, reason = "tests may panic")]

use crate::oracle;
use watane_log::{Entry, Error, Log, PublicKey, Status};
fn fixture() -> (oracle::Directory, Log, PublicKey, Vec<Entry>) {
    let d = oracle::directory();
    let l = Log::open(d.path()).unwrap();
    let a = PublicKey::from_bytes(&oracle::PUBLIC).unwrap();
    let es = oracle::chain(oracle::SEED, 5)
        .iter()
        .map(|r| Entry::decode(r).unwrap())
        .collect();
    (d, l, a, es)
}
#[test]
fn ac2_shuffled_gaps_and_author_mismatch() {
    let (_d, l, a, e) = fixture();
    let result = l
        .ingest(
            &a,
            &[
                e[4].clone(),
                e[1].clone(),
                e[0].clone(),
                e[3].clone(),
                e[2].clone(),
            ],
        )
        .unwrap();
    assert_eq!(result.stored, e.iter().map(Entry::id).collect::<Vec<_>>());
    assert_eq!(l.head(&a).unwrap().unwrap().seq, 4);
    assert!(l.ingest(&a, &e).unwrap().stored.is_empty());
    let (_d, l, a, e) = fixture();
    l.ingest(&a, &e[..2]).unwrap();
    let r = l.ingest(&a, &e[3..]).unwrap();
    assert!(r.stored.is_empty());
    assert_eq!(r.gap, Some(3));
    let own = l.append("a", b"").unwrap();
    let before = l.entries(&a, 0, 100).unwrap();
    assert!(matches!(
        l.ingest(&a, &[e[2].clone(), own]),
        Err(Error::AuthorMismatch { index: 1, .. })
    ));
    assert_eq!(l.entries(&a, 0, 100).unwrap(), before);
}
#[test]
fn ac2_forks_persist_and_freeze() {
    for wrong_prev in [false, true] {
        let (d, l, a, e) = fixture();
        l.ingest(&a, &e[..2]).unwrap();
        let seq = if wrong_prev { 2 } else { 1 };
        let other = Entry::decode(&oracle::entry(seq, Some(&[9; 32]), 99, "a", b"fork")).unwrap();
        let r = l.ingest(&a, std::slice::from_ref(&other)).unwrap();
        let Status::Compromised(proof) = r.status.clone() else {
            panic!("missing proof")
        };
        assert_eq!(proof.chain, e[1]);
        assert_eq!(proof.other, other);
        assert_eq!(l.entries(&a, 1, 1).unwrap(), vec![e[1].clone()]);
        assert!(l.ingest(&a, &e[2..]).unwrap().stored.is_empty());
        drop(l);
        assert_eq!(Log::open(d.path()).unwrap().status(&a).unwrap(), r.status);
    }
}
#[test]
fn ac2_compromised_own_refuses_append() {
    let d = oracle::directory();
    let path = d.path().join("identity.key");
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    std::fs::write(&path, oracle::SEED).unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    #[cfg(windows)]
    use std::io::Write;
    #[cfg(windows)]
    let mut file = watane_log_windows_fs::create_file(&path).unwrap();
    #[cfg(windows)]
    file.write_all(&oracle::SEED).unwrap();
    #[cfg(windows)]
    drop(file);
    let l = Log::open(d.path()).unwrap();
    let e = l.append("a", b"").unwrap();
    let other = Entry::decode(&oracle::entry(0, None, 0, "a", b"fork")).unwrap();
    l.ingest(&e.author(), &[other]).unwrap();
    assert!(matches!(l.append("a",b""),Err(Error::Compromised(a)) if a==e.author()));
}
