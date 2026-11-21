#![allow(
    clippy::unwrap_used,
    clippy::disallowed_methods,
    reason = "tests may panic, catch panics, and share lock-only mutexes that guard no data"
)]

use watane_pkg::*;
fn hex(s: &str) -> Vec<u8> {
    data_encoding::HEXLOWER.decode(s.as_bytes()).unwrap()
}
const GOLDEN: &str = concat!(
    "8c016568656c6c6f65302e312e306548656c6c6f6b536179732068656c6c6f2e5820",
    "0866e9df433265eaf0d959c589d82f1c2020d5b07faf7a530b012ec9f62ff87b",
    "189ff4808080f6"
);

fn refusal(bytes: &[u8], cause: &str) -> ReleaseError {
    let e = std::panic::catch_unwind(|| Release::decode(bytes))
        .expect("decoder must not panic")
        .unwrap_err();
    assert!(e.to_string().contains(cause), "{e}: expected {cause}");
    e
}
#[test]
fn ac4_noncanonical_forms() {
    let g = hex(GOLDEN);
    let mut long = g.clone();
    long.splice(1..2, [0x18, 1]);
    let mut indefinite = g.clone();
    indefinite[0] = 0x9f;
    indefinite.push(0xff);
    let mut extra = g.clone();
    extra[0] = 0x8d;
    extra.push(0xf6);
    let mut trailing = g.clone();
    trailing.push(0);
    for bytes in [long, indefinite, trailing] {
        assert!(matches!(
            refusal(&bytes, "canonical"),
            ReleaseError::NotCanonical
        ));
    }
    assert!(matches!(refusal(&extra, "12"), ReleaseError::Malformed(_)));
    let mut format = g.clone();
    format[1] = 2;
    refusal(&format, "format");
    let mut slug = g.clone();
    slug[3] = b'H';
    refusal(&slug, "slug");
    let mut id = g.clone();
    id[33] = 31;
    id.remove(34);
    refusal(&id, "package");
    assert!(matches!(
        refusal(&vec![0xff; MAX_RELEASE_BYTES + 1], "32768"),
        ReleaseError::TooLarge { size: 32769 }
    ));
}
// Raw CBOR writer for hostile list entries: no release encoder or validation.
fn text(s: &str) -> Vec<u8> {
    let mut v = if s.len() < 24 {
        vec![0x60 + s.len() as u8]
    } else {
        vec![0x78, s.len() as u8]
    };
    v.extend_from_slice(s.as_bytes());
    v
}
fn list(field: usize, entries: Vec<Vec<u8>>) -> Vec<u8> {
    let mut g = hex(GOLDEN);
    let mut v = if entries.len() < 24 {
        vec![0x80 + entries.len() as u8]
    } else {
        vec![0x98, entries.len() as u8]
    };
    for e in entries {
        v.extend(e);
    }
    g.splice(field..field + 1, v);
    g
}
#[test]
fn ac4_semantic_refusals() {
    let a = text("mods:hello/a@0.1.0");
    let b = text("mods:hello/b@0.1.0");
    for field in [69, 70] {
        let label = if field == 69 { "offers" } else { "events" };
        refusal(&list(field, vec![b.clone(), a.clone()]), label);
        refusal(&list(field, vec![a.clone(), a.clone()]), label);
        refusal(&list(field, vec![text("mods:other/a@0.1.0")]), label);
        refusal(&list(field, vec![text("mods:hello/a@0.2.0")]), label);
        refusal(&list(field, vec![a.clone(); 33]), label);
    }
    let mut local = vec![0x83, 2];
    local.extend(text("mods:reading-list/books@0.1.0"));
    local.push(0xf6);
    assert!(
        matches!(refusal(&list(71,vec![local]),"reading-list"),ReleaseError::LocalProvider{slug} if slug.as_str()=="reading-list")
    );
    refusal(&list(71, vec![vec![0x81, 0]; 85]), "at most 84");
    refusal(&list(71, vec![vec![0x81, 0], vec![0x81, 0]]), "grants");
    for interface in ["wasi:other/api@0.1.0", "mods:hello/api@0.1.0"] {
        let mut grant = vec![0x83, 2];
        grant.extend(text(interface));
        grant.extend([0x58, 32]);
        grant.extend([1; 32]);
        refusal(&list(71, vec![grant]), "grant");
    }
    for (field, value) in [(14, ""), (14, " Hello"), (20, "S\n")] {
        let mut g = hex(GOLDEN);
        let end = if field == 14 { 20 } else { 32 };
        g.splice(field..end, text(value));
        refusal(&g, if field == 14 { "name" } else { "summary" });
    }
}
#[test]
fn ac4_truncation_types_and_hostile_lengths() {
    let g = hex(GOLDEN);
    for i in 0..g.len() {
        assert!(Release::decode(&g[..i]).is_err(), "prefix {i}");
    }
    for field in [2, 8, 14, 20, 32, 66, 68, 69, 70, 71, 72] {
        let mut bad = g.clone();
        bad[field] = 0xa0;
        assert!(Release::decode(&bad).is_err(), "field at {field}");
    }
    for field in [69, 70, 71] {
        let mut bad = g.clone();
        bad.splice(
            field..field + 1,
            [0x9b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        );
        Release::decode(&bad).unwrap_err();
    }
}

#[test]
fn ac4_non_text_slug_names_field() {
    let mut bytes = hex(GOLDEN);
    bytes.splice(2..8, [0x01]);
    assert!(matches!(
        refusal(&bytes, "slug"),
        ReleaseError::Malformed(_)
    ));
}
