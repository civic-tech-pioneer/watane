mod oracle;
use watane_log::Log;
#[test]
fn ac4_concurrent_appends_form_one_chain() {
    let d = oracle::directory();
    let a = Log::open(d.path()).unwrap();
    let b = Log::open(d.path()).unwrap();
    let threads: Vec<_> = [a, b]
        .into_iter()
        .map(|l| {
            std::thread::spawn(move || {
                for _ in 0..50 {
                    l.append("a", b"").unwrap();
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    let l = Log::open(d.path()).unwrap();
    let es = l.entries(&l.identity().unwrap(), 0, 101).unwrap();
    assert_eq!(es.len(), 100);
    let mut prev = None;
    for (seq, e) in es.iter().enumerate() {
        assert_eq!(e.seq(), seq as u64);
        assert_eq!(e.prev().map(|p| *p.as_bytes()), prev);
        prev = Some(*blake3::hash(e.bytes()).as_bytes());
    }
}
