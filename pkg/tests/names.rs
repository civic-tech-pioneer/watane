use std::{fmt::Display, str::FromStr};
use watane_pkg::*;

fn grammar<T: FromStr + Display>(valid: &[&str], invalid: &[&str]) {
    for text in valid {
        let value = text
            .parse::<T>()
            .unwrap_or_else(|_| panic!("should accept {text}"));
        assert_eq!(value.to_string(), *text);
    }
    for text in invalid {
        assert!(text.parse::<T>().is_err(), "should refuse {text}");
    }
}

#[test]
fn ac1_names() {
    grammar::<Slug>(
        &[
            "a",
            "reading-list",
            "a0-b1",
            "abcdefghijklmnopqrstuvwxyzabcdefghijklmn",
        ],
        &[
            "",
            "A",
            "1a",
            "a-1",
            "a--b",
            "-a",
            "a-",
            "é",
            "a_b",
            "abcdefghijklmnopqrstuvwxyzabcdefghijklmno",
        ],
    );
    for word in "use type func u8 u16 u32 u64 s8 s16 s32 s64 f32 f64 char resource own borrow record flags variant enum bool string option result future stream error-context list map as from static interface tuple world import export package constructor include with async".split_whitespace() {
        assert!(word.parse::<Slug>().is_err(), "keyword {word}");
    }
    grammar::<Version>(
        &["0.0.0", "0.1.0", "18446744073709551615.2.3"],
        &[
            "",
            "1",
            "1.2",
            "01.2.3",
            "1.02.3",
            "1.2.03",
            "1.2.3-pre",
            "1.2.3+build",
            "1.2.3.4",
            "-1.2.3",
            "1.2.3\n",
            "18446744073709551616.0.0",
        ],
    );
    grammar::<InterfaceName>(
        &["mods:reading-list/books@0.1.0", "a0:b1/type@1.2.3"],
        &[
            "mods:a/b",
            "mods:a/b@01.0.0",
            "Mods:a/b@0.1.0",
            "mods:a/1b@0.1.0",
            "mods:a/b/c@0.1.0",
            "mods:a/b@0.1.0@1.0.0",
        ],
    );
    assert!(
        format!("mods:{}/b@0.1.0", "a".repeat(120))
            .parse::<InterfaceName>()
            .is_err()
    );
    grammar::<HttpHost>(
        &[
            "api.openlibrary.org",
            "127.0.0.1:8080",
            "a",
            "0",
            "a-b.example:65535",
        ],
        &[
            "",
            "A.example",
            "https://a.example",
            "a.example/",
            "a..b",
            "a.",
            ".a",
            "-a",
            "a-",
            "a:0",
            "a:01",
            "a:65536",
            "a:",
            "[::1]",
            "a_b",
            "a:80:90",
        ],
    );
    assert!("a".repeat(64).parse::<HttpHost>().is_err());
    assert!(
        format!(
            "{}.{}.{}.{}",
            "a".repeat(63),
            "b".repeat(63),
            "c".repeat(63),
            "d".repeat(63)
        )
        .parse::<HttpHost>()
        .is_err()
    );
    grammar::<PackageId>(
        &["0866e9df433265eaf0d959c589d82f1c2020d5b07faf7a530b012ec9f62ff87b"],
        &[
            "",
            "0866E9df433265eaf0d959c589d82f1c2020d5b07faf7a530b012ec9f62ff87b",
            "0866e9df",
        ],
    );
    grammar::<ReleaseRef>(
        &["yryonyebyryonyebyryonyebyryonyebyryonyebyryonyebyryo/reading-list@0.1.0"],
        &[
            "local/hello@0.1.0",
            "yryonyebyryonyebyryonyebyryonyebyryonyebyryonyebyryo/hello@01.0.0",
        ],
    );
    let name: InterfaceName = "mods:reading-list/books@0.1.0".parse().unwrap();
    assert_eq!(
        (name.namespace(), name.package(), name.interface()),
        ("mods", "reading-list", "books")
    );
    assert_eq!(
        name.version(),
        Version {
            major: 0,
            minor: 1,
            patch: 0
        }
    );
    assert!(name.satisfied_by(&"mods:reading-list/books@0.1.7".parse().unwrap()));
    assert!(!name.satisfied_by(&"mods:reading-list/events@0.1.7".parse().unwrap()));
    assert!("2.0.0".parse::<Version>().unwrap() < "10.0.0".parse().unwrap());
    assert_eq!(
        PackageId::of(b"").to_string(),
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
    );
    let error = "é".repeat(100).parse::<Slug>().unwrap_err();
    assert!(error.text.chars().count() <= 80);
}

#[test]
fn ac1_zbase32() {
    for (bytes, text) in [
        (
            [0; 32],
            "yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy",
        ),
        (
            [1; 32],
            "yryonyebyryonyebyryonyebyryonyebyryonyebyryonyebyryo",
        ),
        (
            [
                0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64,
                0x07, 0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68,
                0xf7, 0x07, 0x51, 0x1a,
            ],
            "47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy",
        ),
    ] {
        assert_eq!(PublisherKey::from_bytes(bytes).to_string(), text);
        assert_eq!(text.parse::<PublisherKey>().unwrap().as_bytes(), &bytes);
        assert_eq!(
            serde_json::to_string(&PublisherKey::from_bytes(bytes)).unwrap(),
            format!("\"{text}\"")
        );
    }
    grammar::<PublisherKey>(
        &[],
        &[
            "",
            "Yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy",
            "0yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy",
            "yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyb",
        ],
    );
}

#[test]
fn ac1_compatible() {
    for (wanted, offered, yes) in [
        ("1.2.0", "1.2.0", true),
        ("1.2.0", "1.3.5", true),
        ("1.2.0", "1.1.9", false),
        ("1.2.0", "2.0.0", false),
        ("0.1.0", "0.1.7", true),
        ("0.1.3", "0.1.2", false),
        ("0.1.0", "0.2.0", false),
        ("0.1.0", "1.0.0", false),
        ("0.0.1", "0.0.2", true),
    ] {
        assert_eq!(
            compatible(&wanted.parse().unwrap(), &offered.parse().unwrap()),
            yes,
            "{wanted}, {offered}"
        );
    }
}
