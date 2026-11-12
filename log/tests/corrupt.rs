mod oracle;
use watane_log::{Error, Log};
#[test]
fn ac3_corrupt_row_fails_every_read() {
    let d = oracle::directory();
    let l = Log::open(d.path()).unwrap();
    let e = l.append("a", b"").unwrap();
    let db = rusqlite::Connection::open(d.path().join("channels.db")).unwrap();
    db.execute_batch("DROP TRIGGER entries_no_update").unwrap();
    let mut raw = e.bytes().to_vec();
    let last = raw.len() - 1;
    raw[last] ^= 1;
    db.execute("UPDATE entries SET raw=?", [raw]).unwrap();
    for err in [
        l.head(&e.author()).unwrap_err(),
        l.entries(&e.author(), 0, 10).unwrap_err(),
        l.status(&e.author()).unwrap_err(),
    ] {
        assert!(matches!(err,Error::CorruptStore{author,seq:0,..} if author==e.author()));
    }
}
#[test]
fn ac8_unsupported_schema_leaves_file_unchanged() {
    let d = oracle::directory();
    let l = Log::open(d.path()).unwrap();
    drop(l);
    let path = d.path().join("channels.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("PRAGMA user_version=27;").unwrap();
    drop(db);
    let before = std::fs::read(&path).unwrap();
    assert!(
        matches!(Log::open(d.path()), Err(Error::Storage(s)) if s=="unsupported schema version 27")
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
}
