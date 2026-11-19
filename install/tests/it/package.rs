//! `package.sh` over fake programs (TECHNICAL 4, 5, 12.1): the tree, the refusals, the end to end run.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use crate::support::*;

const V: &str = "0.1.0";

fn top(v: &str) -> String {
    format!("watane-{v}-{TARGET}")
}

fn extract(archive: &Path, to: &Path) {
    let st = Command::new(which("tar"))
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(to)
        .status()
        .expect("tar");
    assert!(st.success());
}

#[test]
fn ac1_the_archive_has_the_tree_of_4_2() {
    let co = Checkout::new(V, &["serde-1.0.0", "zzz-2.0.0"]);
    let out_dir = tempfile::tempdir().expect("out");
    let out = co.pack(V, out_dir.path(), None);
    assert_eq!(out.code, 0, "{out:?}");
    let archive = out_dir.path().join(archive_name(V));
    let t = top(V);
    let mut want: Vec<String> = [
        "",
        "/VERSION",
        "/bin",
        "/bin/watane",
        "/bin/watane-dev",
        "/bin/watane-node",
        "/share",
        "/share/watane-dev",
        "/share/watane-dev/vendor",
        "/share/watane-dev/vendor/serde-1.0.0",
        "/share/watane-dev/vendor/serde-1.0.0/Cargo.toml",
        "/share/watane-dev/vendor/zzz-2.0.0",
        "/share/watane-dev/vendor/zzz-2.0.0/Cargo.toml",
        "/share/watane",
        "/share/watane/install.sh",
        "/share/watane/VERSION",
        "/Applications",
        "/Applications/watane.app",
        "/Applications/watane.app/Contents",
        "/Applications/watane.app/Contents/Info.plist",
        "/Applications/watane.app/Contents/MacOS",
        "/Applications/watane.app/Contents/MacOS/watane",
    ]
    .iter()
    .map(|s| format!("{t}{s}"))
    .collect();
    if std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .any(|dir| dir.join("codesign").is_file())
    {
        for name in [
            "",
            "/CodeDirectory",
            "/CodeRequirements",
            "/CodeResources",
            "/CodeSignature",
        ] {
            want.push(format!(
                "{t}/Applications/watane.app/Contents/_CodeSignature{name}"
            ));
        }
    }
    want.sort();
    assert_eq!(listing(&archive), want);

    let x = tempfile::tempdir().expect("x");
    extract(&archive, x.path());
    let tree = x.path().join(&t);
    assert_eq!(
        fs::read_to_string(tree.join("VERSION")).expect("v"),
        "0.1.0\n"
    );
    assert_eq!(
        fs::read_to_string(tree.join("share/watane/VERSION")).expect("v"),
        "0.1.0\n"
    );
    let baked = fs::read_to_string(tree.join("share/watane/install.sh")).expect("script");
    assert_eq!(baked, baked_script(V));
    assert!(!baked.contains("@RELEASE@"));
    for n in [
        "watane",
        "watane-dev",
        "watane-node",
        "../share/watane/install.sh",
    ] {
        let mode = fs::metadata(tree.join("bin").join(n))
            .expect("md")
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "{n} is executable");
    }
    // the third asset is the same bytes, and the sum line is the bare name, two spaces
    assert_eq!(
        fs::read_to_string(out_dir.path().join("install.sh")).expect("asset"),
        baked
    );
    let sums = fs::read_to_string(out_dir.path().join("SHA256SUMS")).expect("sums");
    let bytes = fs::read(&archive).expect("archive");
    assert_eq!(sums, format!("{}  {}\n", sha256(&bytes), archive_name(V)));
}

#[test]
fn ac2_a_missing_part_is_refused_and_named() {
    type Spoil = fn(&Checkout);
    let parts: [(&str, &str, Spoil); 7] = [
        ("watane", "target/release/watane", |c| {
            fs::remove_file(c.path("target/release/watane")).expect("rm");
        }),
        ("watane-dev", "target/release/watane-dev", |c| {
            fs::remove_file(c.path("target/release/watane-dev")).expect("rm");
        }),
        ("watane-node", "target/release/watane-node", |c| {
            fs::remove_file(c.path("target/release/watane-node")).expect("rm");
        }),
        ("the app", "watane.app", |c| {
            fs::remove_dir_all(c.path("target/release/bundle")).expect("rm");
        }),
        ("the app's Info.plist", "Info.plist", |c| {
            fs::remove_file(c.path("target/release/bundle/macos/watane.app/Contents/Info.plist"))
                .expect("rm");
        }),
        ("the vendor script", "vendor.sh", |c| {
            fs::remove_file(c.path("dev/scripts/vendor.sh")).expect("rm");
        }),
        ("an empty vendor folder", "vendor folder", |c| {
            fs::write(
                c.path("dev/scripts/vendor.sh"),
                "#!/bin/sh\nmkdir -p \"$1\"\n",
            )
            .expect("w");
        }),
    ];
    for (what, named, spoil) in parts {
        let co = Checkout::new(V, &["serde-1.0.0"]);
        spoil(&co);
        let out_dir = tempfile::tempdir().expect("out");
        let out = co.pack(V, &out_dir.path().join("o"), None);
        assert_ne!(out.code, 0, "{what}: {out:?}");
        assert!(out.stderr.contains("missing part"), "{what}: {out:?}");
        assert!(out.stderr.contains(named), "{what}: {out:?}");
        assert!(
            !out_dir.path().join("o").exists(),
            "{what}: nothing was written"
        );
    }
}

#[test]
fn ac3_signing_runs_only_where_the_tools_are() {
    let co = Checkout::new(V, &["serde-1.0.0"]);
    let fakes = tempfile::tempdir().expect("fakes");
    let log = fakes.path().join("log");
    for t in ["codesign", "otool"] {
        let p = fakes.path().join(t);
        fs::write(
            &p,
            format!("#!/bin/sh\necho \"{t} $*\" >> {}\n", log.display()),
        )
        .expect("w");
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    let out_dir = tempfile::tempdir().expect("out");
    let out = co.pack(V, out_dir.path(), Some(fakes.path()));
    assert_eq!(out.code, 0, "{out:?}");
    let calls = fs::read_to_string(&log).expect("log");
    let lines: Vec<&str> = calls.lines().collect();
    for p in ["watane", "watane-dev", "watane-node"] {
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("codesign --force --sign - ")
                    && l.ends_with(&format!("/bin/{p}"))),
            "{calls}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("codesign --verify --deep --strict ")
                    && l.ends_with(&format!("/bin/{p}"))),
            "{calls}"
        );
    }
    assert!(
        lines.iter().any(
            |l| l.starts_with("codesign --force --deep --sign - ") && l.ends_with("watane.app")
        ),
        "{calls}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("codesign --verify --deep --strict ")
                && l.ends_with("watane.app")),
        "{calls}"
    );

    // a failing signature check stops the pack
    fs::write(fakes.path().join("codesign"), "#!/bin/sh\nexit 1\n").expect("w");
    let out_dir = tempfile::tempdir().expect("out");
    let out = co.pack(V, &out_dir.path().join("o"), Some(fakes.path()));
    assert_ne!(out.code, 0, "{out:?}");
    assert!(!out_dir.path().join("o").exists());
}

#[test]
#[cfg(not(target_os = "macos"))]
fn ac3_without_the_tools_the_pack_goes_on() {
    assert!(!Path::new("/usr/bin/codesign").exists());
    let co = Checkout::new(V, &["serde-1.0.0"]);
    let out_dir = tempfile::tempdir().expect("out");
    let out = co.pack(V, out_dir.path(), None);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out_dir.path().join(archive_name(V)).is_file());
}

#[test]
#[cfg(target_os = "macos")]
fn ac3_the_programs_carry_a_valid_ad_hoc_signature() {
    let co = Checkout::new(V, &["serde-1.0.0"]);
    for n in ["watane", "watane-dev", "watane-node"] {
        fs::copy("/bin/echo", co.path(&format!("target/release/{n}"))).expect("copy");
    }
    let out_dir = tempfile::tempdir().expect("out");
    let out = co.pack(V, out_dir.path(), None);
    assert_eq!(out.code, 0, "{out:?}");
    let x = tempfile::tempdir().expect("x");
    extract(&out_dir.path().join(archive_name(V)), x.path());
    for n in ["watane", "watane-dev", "watane-node"] {
        let st = Command::new("codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(x.path().join(top(V)).join("bin").join(n))
            .status()
            .expect("codesign");
        assert!(st.success(), "{n} is signed");
    }
}

/// Packs `version` and serves it from the fake release of `fx`; returns the baked script of the archive.
fn pack_and_serve(
    fx: &Fx,
    sums: &mut String,
    version: &str,
    vendor: &[&str],
) -> std::path::PathBuf {
    let co = Checkout::new(version, vendor);
    let out_dir = tempfile::tempdir().expect("out");
    let out = co.pack(version, out_dir.path(), None);
    assert_eq!(out.code, 0, "{out:?}");
    let name = archive_name(version);
    fx.rel.put(
        &name,
        fs::read(out_dir.path().join(&name)).expect("archive"),
    );
    // one server stands for both releases: its SHA256SUMS holds a line for each archive
    let line = fs::read_to_string(out_dir.path().join("SHA256SUMS")).expect("sums");
    sums.push_str(&line);
    fx.rel.put("SHA256SUMS", sums.clone().into_bytes());
    let x = tempfile::tempdir().expect("x");
    extract(&out_dir.path().join(&name), x.path());
    let s = x.path().join(top(version)).join("share/watane/install.sh");
    let keep = fx.root.path().join(format!("script-{version}.sh"));
    fs::copy(s, &keep).expect("copy");
    keep
}

#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1) for the uninstall"]
fn ac4_install_upgrade_uninstall_of_packed_archives() {
    let fx = Fx::new();
    let mut sums = String::new();
    let s1 = pack_and_serve(&fx, &mut sums, "0.1.0", &["old-crate-1.0.0"]);
    let s2 = pack_and_serve(&fx, &mut sums, "0.2.0", &["new-crate-1.0.0"]);
    assert_ne!(s1, s2);

    fs::copy(&s1, &fx.script).expect("script");
    let out = fx.run(&[]);
    assert_eq!(out.code, 0, "{out:?}");
    fx.assert_installed("0.1.0");
    assert!(fx.share("watane-dev/vendor/old-crate-1.0.0").is_dir());

    // the upgrade runs the installed copy, which says 0.1.0 and is told which release to take
    fs::copy(fx.share("watane/install.sh"), &fx.script).expect("script");
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.has_line("upgrading watane 0.1.0 to 0.2.0"));
    fx.assert_installed("0.2.0");
    assert!(!fx.share("watane-dev/vendor/old-crate-1.0.0").exists());
    assert!(fx.share("watane-dev/vendor/new-crate-1.0.0").is_dir());

    fs::copy(fx.share("watane/install.sh"), &fx.script).expect("script");
    write_file(&fx.home.join(".watane/marker"), "marker");
    let out = fx.run_pty(&["--uninstall"], &[], &[("keep it: ", "n")]);
    assert_eq!(out.code, 0, "{out:?}");
    for p in [
        fx.bin("watane"),
        fx.bin("watane-dev"),
        fx.bin("watane-node"),
        fx.share("watane"),
        fx.share("watane-dev"),
        fx.app(),
    ] {
        assert!(!p.exists(), "{} should be gone", p.display());
    }
}

/// AC1 (4.2): a link among the inputs is refused and no archive is made.
#[test]
fn ac1_a_link_among_the_inputs_is_refused() {
    let co = Checkout::new(V, &["serde-1.0.0"]);
    std::os::unix::fs::symlink(
        "Info.plist",
        co.path("target/release/bundle/macos/watane.app/Contents/link"),
    )
    .expect("link");
    let out_dir = tempfile::tempdir().expect("out");
    let out = co.pack(V, out_dir.path(), None);
    assert_eq!(out.code, 1, "{out:?}");
    assert!(out.stderr.contains("link"), "{out:?}");
    assert!(!out_dir.path().join(archive_name(V)).exists());
}
