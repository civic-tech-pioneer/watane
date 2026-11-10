use std::{
    collections::BTreeMap,
    fs::{self, FileTimes},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};
use watane_pkg::{MAX_PACKAGE_BYTES, Mod, ModError, Package, PackageError, PackageId, Rule, pack};

const HELLO: [(&str, &[u8]); 3] = [
    ("mod.toml", b"name = \"Hello\"\nsummary = \"Says hello.\"\n"),
    ("ui/index.html", b"<p>hello</p>\n"),
    (
        "wit/world.wit",
        b"package mods:hello@0.1.0;\n\nworld hello {}\n",
    ),
];
const GOLDEN: &str = concat!(
    "574154414e45504b0100000003",
    "00086d6f642e746f6d6c00000027",
    "6e616d65203d202248656c6c6f220a73756d6d617279203d2022536179732068656c6c6f2e220a",
    "000d75692f696e6465782e68746d6c0000000d3c703e68656c6c6f3c2f703e0a",
    "000d7769742f776f726c642e7769740000002a",
    "7061636b616765206d6f64733a68656c6c6f40302e312e303b0a0a776f726c642068656c6c6f207b7d0a",
);

fn put(dir: &Path, name: &str, data: impl AsRef<[u8]>) {
    let file = dir.join(name);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, data).unwrap();
}
fn hello() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, bytes) in HELLO {
        put(dir.path(), path, bytes);
    }
    dir
}
// Deliberately no sorting, validation, or use of the package encoder.
fn raw(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut bytes = b"WATANEPK\x01".to_vec();
    bytes.extend_from_slice(&(files.len() as u32).to_be_bytes());
    for (path, data) in files {
        bytes.extend_from_slice(&(path.len() as u16).to_be_bytes());
        bytes.extend_from_slice(path.as_bytes());
        bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
        bytes.extend_from_slice(data);
    }
    bytes
}
fn literal_map() -> BTreeMap<String, Vec<u8>> {
    HELLO
        .into_iter()
        .map(|(p, b)| (p.into(), b.into()))
        .collect()
}
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root).unwrap().to_str().unwrap().into(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}
fn path_error(bytes: Vec<u8>, path: &str) {
    let error = std::panic::catch_unwind(|| Package::from_bytes(bytes))
        .expect("decoder must not panic")
        .unwrap_err();
    assert!(
        matches!(&error, PackageError::Path { path: p, .. } if p == path),
        "{error:?}"
    );
    assert!(error.to_string().contains(path));
}
fn format_error(bytes: Vec<u8>) {
    let error = std::panic::catch_unwind(|| Package::from_bytes(bytes))
        .expect("decoder must not panic")
        .unwrap_err();
    assert!(matches!(error, PackageError::Format(_)), "{error:?}");
}

#[test]
fn ac1_pack_golden() {
    let dir = hello();
    let p = pack(dir.path(), false).unwrap();
    let expected = data_encoding::HEXLOWER.decode(GOLDEN.as_bytes()).unwrap();
    assert_eq!(p.as_bytes(), expected);
    assert_eq!(p.len(), 159);
    assert_eq!(
        p.id().to_string(),
        "0866e9df433265eaf0d959c589d82f1c2020d5b07faf7a530b012ec9f62ff87b"
    );
    assert_eq!(p.clone().into_bytes(), expected);
}

#[test]
fn ac2_pack_determinism() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    for (n, dir) in [a.path(), b.path()].into_iter().enumerate() {
        let mut files = HELLO.to_vec();
        if n == 1 {
            files.reverse();
        }
        for (path, data) in files {
            put(dir, path, data);
            let file = fs::OpenOptions::new()
                .write(true)
                .open(dir.join(path))
                .unwrap();
            file.set_times(FileTimes::new().set_modified(
                SystemTime::UNIX_EPOCH + Duration::from_secs(1_000 + n as u64 * 50_000),
            ))
            .unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(
                    dir.join(path),
                    fs::Permissions::from_mode(if n == 0 { 0o600 } else { 0o755 }),
                )
                .unwrap();
            }
        }
    }
    let pa = pack(a.path(), false).unwrap();
    let pb = pack(b.path(), false).unwrap();
    assert_eq!(pa.as_bytes(), pb.as_bytes());
    assert_eq!(pa.id(), pb.id());
}

#[test]
fn ac3_pack_content() {
    let dir = hello();
    for path in [
        ".git/config",
        "logic/target/output",
        ".DS_Store",
        "notes.md",
        "logic/src/lib.rs",
        "logic/Cargo.lock",
        "tests/check.rs",
        "ui/.secret",
        "logic/src/.key",
    ] {
        put(dir.path(), path, b"source or secret");
    }
    assert_eq!(
        pack(dir.path(), false).unwrap().paths().collect::<Vec<_>>(),
        ["mod.toml", "ui/index.html", "wit/world.wit"]
    );
    let p = pack(dir.path(), true).unwrap();
    assert_eq!(
        p.paths().collect::<Vec<_>>(),
        [
            "logic/Cargo.lock",
            "logic/src/lib.rs",
            "mod.toml",
            "tests/check.rs",
            "ui/index.html",
            "wit/world.wit"
        ]
    );
    assert!(p.has_source());
    put(dir.path(), "ui/a b.js", b"bad path");
    assert!(
        matches!(pack(dir.path(), false), Err(PackageError::Path { path, .. }) if path == "ui/a b.js")
    );
}

#[cfg(unix)]
#[test]
fn ac3_pack_content_links_and_fifo() {
    use std::os::unix::fs::symlink;
    let dir = hello();
    let outside = tempfile::NamedTempFile::new().unwrap();
    symlink(outside.path(), dir.path().join("ui/key")).unwrap();
    assert!(
        matches!(pack(dir.path(), false), Err(PackageError::Symlink { path }) if path == "ui/key")
    );
    fs::remove_file(dir.path().join("ui/key")).unwrap();
    assert!(
        std::process::Command::new("mkfifo")
            .arg(dir.path().join("ui/pipe"))
            .status()
            .unwrap()
            .success()
    );
    assert!(
        matches!(pack(dir.path(), false), Err(PackageError::Path { path, .. }) if path == "ui/pipe")
    );
    fs::remove_file(dir.path().join("ui/pipe")).unwrap();
    // Source parents must be checked before descending through them as well.
    symlink(outside.path().parent().unwrap(), dir.path().join("logic")).unwrap();
    assert!(pack(dir.path(), false).is_ok());
    assert!(
        matches!(pack(dir.path(), true), Err(PackageError::Symlink { path }) if path == "logic")
    );
}

#[test]
fn ac3_pack_checks_component() {
    let dir = hello();
    put(
        dir.path(),
        "wit/world.wit",
        b"package mods:hello@0.1.0; interface api { ping: func(); } world hello { export api; }",
    );
    put(dir.path(), "logic/Cargo.toml", b"source");
    for built in [false, true] {
        if built {
            put(dir.path(), "logic.wasm", b"not a component");
        }
        assert!(
            matches!(pack(dir.path(), true), Err(PackageError::Mod(ModError::Invalid(v))) if v.iter().any(|v| v.rule == Rule::Component && v.file == "logic.wasm"))
        );
    }
}

#[test]
fn ac3_pack_content_built_mod_with_source() {
    use wit_component::{ComponentEncoder, StringEncoding, dummy_module, embed_component_metadata};
    use wit_parser::{ManglingAndAbi, Resolve};
    let world =
        "package mods:hello@0.1.0; interface api { ping: func(); } world hello { export api; }";
    let mut resolve = Resolve::default();
    let package = resolve.push_str("world.wit", world).unwrap();
    let world_id = resolve.select_world(&[package], None).unwrap();
    let mut module = dummy_module(&resolve, world_id, ManglingAndAbi::Standard32);
    embed_component_metadata(&mut module, &resolve, world_id, StringEncoding::UTF8).unwrap();
    let component = ComponentEncoder::default()
        .module(&module)
        .unwrap()
        .validate(true)
        .encode()
        .unwrap();
    let dir = hello();
    put(dir.path(), "wit/world.wit", world);
    put(dir.path(), "logic.wasm", &component);
    for path in [
        "logic/Cargo.toml",
        "logic/Cargo.lock",
        "logic/src/lib.rs",
        "tests/check.rs",
    ] {
        put(dir.path(), path, b"source");
    }
    let expected_paths = [
        "logic.wasm",
        "logic/Cargo.lock",
        "logic/Cargo.toml",
        "logic/src/lib.rs",
        "mod.toml",
        "tests/check.rs",
        "ui/index.html",
        "wit/world.wit",
    ];
    let p = pack(dir.path(), true).unwrap();
    assert_eq!(p.paths().collect::<Vec<_>>(), expected_paths);
    assert_eq!(p.file("logic.wasm"), Some(component.as_slice()));
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("built");
    let m = p.unpack(&dest).unwrap();
    assert!(m.logic().is_some());
    assert_eq!(tree(&dest), tree(dir.path()));
    let p = pack(dir.path(), false).unwrap();
    assert_eq!(
        p.paths().collect::<Vec<_>>(),
        ["logic.wasm", "mod.toml", "ui/index.html", "wit/world.wit"]
    );
    assert!(!p.has_source());
}

#[test]
fn ac4_decode_refusals() {
    for path in [
        "ui/../../x",
        "/x",
        "ui//x",
        "ui/.x",
        "notes.md",
        "ui/a b.js",
        "ui/é",
        "ui/a\\b",
        "ui/",
        "",
    ] {
        path_error(raw(&[(path, b"")]), path);
    }
    for files in [
        vec![("ui/A.js", b"".as_slice()), ("ui/a.js", b"")],
        vec![("ui/x", b""), ("ui/x/y", b"")],
        vec![("ui/X", b""), ("ui/x/y", b"")],
        vec![("ui/A/x", b""), ("ui/a", b"")],
        vec![("ui/x", b""), ("ui/x", b"")],
    ] {
        path_error(raw(&files), files[1].0);
    }
    let oversize = vec![0; MAX_PACKAGE_BYTES as usize + 1];
    assert!(matches!(
        Package::from_bytes(oversize),
        Err(PackageError::TooLarge { size: 67_108_865 })
    ));
    let mut past_end = raw(&[("ui/x", b"")]);
    let end = past_end.len();
    past_end[end - 4..].copy_from_slice(&u32::MAX.to_be_bytes());
    let error = Package::from_bytes(past_end).unwrap_err();
    assert!(matches!(error, PackageError::Format(_)));
    assert!(error.to_string().contains("ui/x"));
    let mut bad_magic = raw(&HELLO);
    bad_magic[0] = 0;
    let mut version = raw(&HELLO);
    version[8] = 2;
    // 4097 well-formed, sorted records: only the count bound can refuse them.
    let names: Vec<_> = (0..4097).map(|i| format!("ui/{i:04}")).collect();
    let count = raw(&names
        .iter()
        .map(|p| (p.as_str(), b"".as_slice()))
        .collect::<Vec<_>>());
    let mut trailing = raw(&HELLO);
    trailing.push(0);
    for bytes in [
        bad_magic,
        version,
        raw(&[]),
        count,
        raw(&[("ui/z", b""), ("ui/a", b"")]),
        trailing,
    ] {
        format_error(bytes);
    }
    // Every strict prefix exercises truncated headers, paths, sizes and payloads.
    let valid = raw(&HELLO);
    for end in 0..valid.len() {
        format_error(valid[..end].to_vec());
    }
    let long = format!("ui/{}", "a".repeat(129));
    path_error(raw(&[(&long, b"")]), &long);
    let deep = format!("ui/{}", vec!["a"; 16].join("/"));
    path_error(raw(&[(&deep, b"")]), &deep);
    let long = format!("ui/{}/{}", "a".repeat(128), "b".repeat(128));
    path_error(raw(&[(&long, b"")]), &long);
    let mut non_ascii = raw(&[("ui/x", b"")]);
    non_ascii[18] = 255;
    assert!(Package::from_bytes(non_ascii).is_err());
}

#[test]
fn ac5_unpack() {
    let tmp = tempfile::tempdir().unwrap();
    let p = Package::from_bytes(raw(&HELLO)).unwrap();
    let dest = tmp.path().join("new");
    let m = p.unpack(&dest).unwrap();
    assert_eq!(tree(&dest), literal_map());
    assert_eq!(m.dir(), dest);
    assert_eq!(m.name(), "Hello");
    assert_eq!(Mod::read(&dest).unwrap().slug().as_str(), "hello");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in [
            "",
            "ui",
            "wit",
            "mod.toml",
            "ui/index.html",
            "wit/world.wit",
        ] {
            let meta = fs::metadata(dest.join(path)).unwrap();
            assert_eq!(
                meta.permissions().mode() & 0o777,
                if meta.is_dir() { 0o755 } else { 0o644 }
            );
        }
    }
    put(&dest, "keep", b"unchanged");
    let before = tree(&dest);
    assert!(matches!(p.unpack(&dest), Err(PackageError::DestExists { path }) if path == dest));
    assert_eq!(tree(&dest), before);
    let file_dest = tmp.path().join("file");
    fs::write(&file_dest, b"keep").unwrap();
    assert!(matches!(
        p.unpack(&file_dest),
        Err(PackageError::DestExists { .. })
    ));
    assert_eq!(fs::read(file_dest).unwrap(), b"keep");
    let mut bad = HELLO;
    bad[0].1 = b"name = \"Hello\"\nsummary = \"Bad\"\nslug = \"hello\"\n";
    let invalid = Package::from_bytes(raw(&bad)).unwrap();
    let failed = tmp.path().join("invalid");
    assert!(
        matches!(invalid.unpack(&failed), Err(PackageError::Mod(ModError::Invalid(v))) if v.iter().any(|v| v.rule == Rule::ModToml && v.file == "mod.toml"))
    );
    assert!(!failed.exists());
    assert!(p.unpack(&tmp.path().join("missing/child")).is_err());
    assert!(!tmp.path().join("missing").exists());
}

#[test]
fn ac5_unpack_checks_component_and_cleans_up() {
    let files = [
        ("logic.wasm", b"not a component".as_slice()),
        HELLO[0],
        HELLO[1],
        ("wit/world.wit", b"package mods:hello@0.1.0; interface api { ping: func(); } world hello { export api; }"),
    ];
    let p = Package::from_bytes(raw(&files)).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let dest = tmp.path().join("bad-component");
    assert!(
        matches!(p.unpack(&dest), Err(PackageError::Mod(ModError::Invalid(v))) if v.iter().any(|v| v.rule == Rule::Component))
    );
    assert!(!dest.exists());
}

#[cfg(unix)]
#[test]
fn ac5_unpack_existing_symlink_is_unchanged() {
    use std::os::unix::fs::symlink;
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("missing");
    let dest = tmp.path().join("link");
    symlink(&target, &dest).unwrap();
    let p = Package::from_bytes(raw(&HELLO)).unwrap();
    assert!(matches!(p.unpack(&dest), Err(PackageError::DestExists { path }) if path == dest));
    assert_eq!(fs::read_link(dest).unwrap(), target);
    assert!(!target.exists());
}

#[test]
fn ac4_package_limits() {
    // A sparse input at the limit is refused, not packed.
    let dir = hello();
    fs::File::create(dir.path().join("ui/large"))
        .unwrap()
        .set_len(MAX_PACKAGE_BYTES)
        .unwrap();
    assert!(matches!(
        pack(dir.path(), false),
        Err(PackageError::TooLarge { .. })
    ));

    // Inclusive count bound, with no payload allocations in the decoder.
    let names: Vec<_> = (0..4096).map(|i| format!("ui/{i:04}")).collect();
    let files: Vec<_> = names.iter().map(|p| (p.as_str(), b"".as_slice())).collect();
    let p = Package::from_bytes(raw(&files)).unwrap();
    assert_eq!(p.paths().count(), 4096);
    assert_eq!(p.file("ui/4095"), Some(b"".as_slice()));

    // Inclusive byte bound. The wire overhead is 13 + 2 + 4 + 4 = 23 bytes.
    let payload = vec![0; MAX_PACKAGE_BYTES as usize - 23];
    let p = Package::from_bytes(raw(&[("ui/x", &payload)])).unwrap();
    assert_eq!(p.len(), MAX_PACKAGE_BYTES);
    assert_eq!(p.file("ui/x"), Some(payload.as_slice()));
}

#[test]
fn ac6_round_trip() {
    let mut seed = 0x7357_1234_5678_abcd_u64;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        seed
    };
    for case in 0..200 {
        let dir = hello();
        let mut expected = literal_map();
        for n in 0..next() % 16 {
            let path = format!("ui/group{}/file{n}.bin", next() % 4);
            let data: Vec<u8> = (0..next() % 512).map(|_| (next() >> 32) as u8).collect();
            put(dir.path(), &path, &data);
            expected.insert(path, data);
        }
        let p = pack(dir.path(), false).unwrap();
        let decoded = Package::from_bytes(p.as_bytes().to_vec()).unwrap();
        for (path, data) in &expected {
            assert_eq!(
                decoded.file(path),
                Some(data.as_slice()),
                "case {case}: {path}"
            );
        }
        assert_eq!(decoded.file("ui/missing"), None);
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("unpacked");
        decoded.unpack(&dest).unwrap();
        assert_eq!(tree(&dest), expected, "case {case}");
    }
}

#[test]
fn ac7_fixtures_and_source() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut valid_pages = Vec::new();
    for entry in fs::read_dir(fixtures).unwrap() {
        let path = entry.unwrap().path();
        if let Ok(m) = Mod::read(&path)
            && m.logic().is_none()
        {
            valid_pages.push(path);
        }
    }
    assert!(valid_pages.iter().any(|p| p.ends_with("hello")));
    for dir in valid_pages {
        for source in [false, true] {
            let a = pack(&dir, source).unwrap();
            let b = pack(&dir, source).unwrap();
            assert_eq!(a.as_bytes(), b.as_bytes());
            assert_eq!(a.id(), PackageId::of(a.as_bytes()));
            assert_eq!(a.len(), a.as_bytes().len() as u64);
            assert!(!a.has_source());
        }
    }
    for path in [
        "logic/Cargo.toml",
        "logic/Cargo.lock",
        "logic/src/lib.rs",
        "tests/test.rs",
    ] {
        let p = Package::from_bytes(raw(&[(path, b"")])).unwrap();
        assert!(p.has_source(), "{path}");
    }
    let dir = hello();
    put(dir.path(), "tests/test.rs", b"test source");
    assert!(!pack(dir.path(), false).unwrap().has_source());
    assert!(pack(dir.path(), true).unwrap().has_source());
}

#[test]
fn ac8_public_types_are_send_sync() {
    fn send_sync<T: Send + Sync + 'static>() {}
    send_sync::<Package>();
    send_sync::<PackageError>();
    assert_eq!(MAX_PACKAGE_BYTES, 67_108_864);
}
