#![allow(clippy::unwrap_used, reason = "tests may panic")]

use watane_pkg::*;

const GOLDEN: &str = concat!(
    "8c016568656c6c6f65302e312e306548656c6c6f6b536179732068656c6c6f2e5820",
    "0866e9df433265eaf0d959c589d82f1c2020d5b07faf7a530b012ec9f62ff87b",
    "189ff4808080f6"
);

fn hex(s: &str) -> Vec<u8> {
    data_encoding::HEXLOWER.decode(s.as_bytes()).unwrap()
}
fn with_grants(body: &[u8]) -> Vec<u8> {
    let mut b = hex(GOLDEN);
    b.splice(71..72, body.iter().copied());
    b
}

#[test]
fn ac3_agent_grants_encode_and_decode() {
    let mut r = Release::decode(&hex(GOLDEN)).unwrap();
    for (grants, body) in [
        (
            vec![Grant::Storage, Grant::ClaudeCode, Grant::ClaudeCodeShell],
            "83810081058106",
        ),
        (vec![Grant::Manage, Grant::ClaudeCode], "8281048105"),
    ] {
        r.grants = grants.clone();
        let expected = with_grants(&hex(&body.replace(' ', "")));
        assert_eq!(r.encode(), expected);
        assert_eq!(Release::decode(&expected).unwrap().grants, grants);
    }
}

#[test]
fn ac3_agent_grants_must_ascend_once() {
    for body in ["8281068105", "8281058105"] {
        let e = Release::decode(&with_grants(&hex(&body.replace(' ', "")))).unwrap_err();
        assert!(e.to_string().contains("ascending"), "{e}");
    }
}

#[test]
fn ac3_limit_is_84_grants() {
    let list = |n: usize| {
        let mut body = vec![0x98, n as u8];
        for i in 0..n {
            let host = format!("h{i:02}.example");
            body.extend([0x82, 0x01, 0x60 + host.len() as u8]);
            body.extend(host.as_bytes());
        }
        with_grants(&body)
    };
    let r = Release::decode(&list(84)).unwrap();
    assert_eq!(r.grants.len(), 84);
    let e = Release::decode(&list(85)).unwrap_err();
    assert!(
        e.to_string()
            .contains("grants: expected an array of at most 84"),
        "{e}"
    );
}
