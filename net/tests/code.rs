mod support;
use support::public_of;
use watane_net::{Hint, NetError, NodeKey, PairCode};

fn zbase32(bytes: &[u8]) -> String {
    let alphabet = b"ybndrfg8ejkmcpqxot1uwisza345h769";
    (0..(bytes.len() * 8).div_ceil(5))
        .map(|i| {
            let mut n = 0;
            for j in 0..5 {
                let bit = i * 5 + j;
                n <<= 1;
                if bit < bytes.len() * 8 {
                    n |= (bytes[bit / 8] >> (7 - bit % 8)) & 1;
                }
            }
            alphabet[n as usize] as char
        })
        .collect()
}

const SEED: [u8; 32] = [1; 32];
const SECRET: [u8; 16] = *b"0123456789abcdef";

fn key() -> NodeKey {
    NodeKey::from_bytes(public_of(SEED))
}
fn key_text() -> String {
    zbase32(&public_of(SEED))
}
fn secret_text() -> String {
    zbase32(&SECRET)
}
fn base() -> String {
    format!("watane-pair:{}:{}", key_text(), secret_text())
}
fn bad(text: &str) {
    assert!(
        matches!(text.parse::<PairCode>(), Err(NetError::BadCode)),
        "{text}"
    );
}

#[test]
fn ac3_round_trip_with_0_1_and_8_hints() {
    let addr: std::net::SocketAddr = "192.168.1.20:4433".parse().unwrap();
    let relay = url::Url::parse("https://relay.example.org/").unwrap();
    let hints: Vec<Hint> = (0..8)
        .map(|i| {
            if i % 2 == 0 {
                Hint::Addr(format!("10.0.0.{i}:{}", 4000 + i).parse().unwrap())
            } else {
                Hint::Relay(url::Url::parse(&format!("https://r{i}.example.org/")).unwrap())
            }
        })
        .collect();
    for hints in [vec![], vec![Hint::Addr(addr)], hints.clone()] {
        let code = PairCode {
            anchor: key(),
            secret: SECRET,
            hints,
        };
        let mut expected = base();
        for h in &code.hints {
            match h {
                Hint::Addr(a) => expected.push_str(&format!(";addr={a}")),
                Hint::Relay(u) => expected.push_str(&format!(";relay={u}")),
            }
        }
        assert_eq!(code.to_string(), expected);
        assert_eq!(expected.parse::<PairCode>().unwrap(), code);
    }
    let one = PairCode {
        anchor: key(),
        secret: SECRET,
        hints: vec![Hint::Relay(relay)],
    };
    assert_eq!(
        one.to_string(),
        format!("{};relay=https://relay.example.org/", base())
    );
}

#[test]
fn ac3_an_ipv6_address_hint_round_trips() {
    let text = format!("{};addr=[2001:db8::1]:4433", base());
    let code: PairCode = text.parse().unwrap();
    assert_eq!(code.to_string(), text);
}

#[test]
fn ac3_refused() {
    let (k, s) = (key_text(), secret_text());
    assert_eq!(s.len(), 26);
    bad(&format!("watane-node:{k}:{s}")); // wrong prefix
    bad(&format!("watane-pair:{k}")); // no secret
    bad(&format!("watane-pair:{k}:{}", &s[..25])); // 25 chars
    bad(&format!("watane-pair:{k}:{s}y")); // 27 chars
    // the last char carries 3 bits of 5; set one of the two unused bits
    let mut last = s.clone();
    let c = last.pop().unwrap();
    let alphabet = "ybndrfg8ejkmcpqxot1uwisza345h769";
    let index = alphabet.find(c).unwrap();
    assert_eq!(index & 3, 0, "the unused bits of a real secret are zero");
    last.push(alphabet.as_bytes()[index | 1] as char);
    bad(&format!("watane-pair:{k}:{last}"));
    // a char outside z-base-32 ("l", "v", "0", "2" are not in it)
    for c in ['l', 'v', '0', '2', 'A'] {
        let mut t = s.clone();
        t.replace_range(3..4, &c.to_string());
        bad(&format!("watane-pair:{k}:{t}"));
    }
    // 9 hints
    let nine: String = (0..9).map(|i| format!(";addr=10.0.0.{i}:1")).collect();
    bad(&format!("{}{nine}", base()));
    format!(
        "{}{}",
        base(),
        &nine[..nine.len() - ";addr=10.0.0.8:1".len()]
    )
    .parse::<PairCode>()
    .unwrap();
    // over 1024 bytes
    let long = format!("{};relay=https://{}.example.org/", base(), "a".repeat(1100));
    assert!(long.len() > 1024);
    bad(&long);
    // http relay, other schemes, unknown hint name, empty hint
    bad(&format!("{};relay=http://relay.example.org/", base()));
    bad(&format!("{};relay=wss://relay.example.org/", base()));
    bad(&format!("{};relay=relay.example.org", base()));
    bad(&format!("{};peer=10.0.0.1:1", base()));
    bad(&format!("{};", base()));
    bad(&format!("{};;addr=10.0.0.1:1", base()));
    bad(&format!("{};addr=", base()));
    bad(&format!("{};addr=not-an-address", base()));
}

#[test]
fn ac2_surrounding_whitespace_is_trimmed_inside_is_not() {
    let want: PairCode = base().parse().unwrap();
    for text in [
        format!("{}\n", base()),
        format!(" \t{}\r\n", base()),
        format!("\n{}", base()),
    ] {
        assert_eq!(text.parse::<PairCode>().unwrap(), want, "{text:?}");
    }
    bad(&format!("watane-pair:{}: {}", key_text(), secret_text()));
    bad(&format!("{}\n;addr=10.0.0.1:1", base()));
    bad(&format!("{} ;addr=10.0.0.1:1", base()));
}
