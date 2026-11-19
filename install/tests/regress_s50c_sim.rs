//! Regressions found by acting out S50c as Alice on the real release (TECHNICAL 3.4.2, 12.4).
mod support;

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::Command;
use support::*;

/// A rerun after Alice re-linked `~/.local/share/watane-dev` to `~/Documents`
/// is refused (rule 4), but the refusal only says "files that watane did not
/// install" and tells her to rename `~/.local/share/watane-dev/vendor`, which
/// is her own `~/Documents/vendor`. It must name the cause, the link, the way
/// the uninstall's `skipped` line does.
#[test]
fn regress_s50c_relinked_folder_refusal_names_the_link() {
    let fx = Fx::new();
    assert_eq!(fx.run(&["--no-toolchain"]).code, 0);
    fs::rename(fx.share("watane-dev"), fx.home.join("old-dev")).unwrap();
    write_file(&fx.home.join("Documents/vendor/thesis.txt"), "mine");
    symlink(fx.home.join("Documents"), fx.share("watane-dev")).unwrap();

    let out = fx.run(&["--no-toolchain"]);

    assert_eq!(out.code, 8, "{out:?}");
    assert_eq!(
        fx.read(&fx.home.join("Documents/vendor/thesis.txt")),
        "mine"
    );
    assert!(
        out.stderr.contains("~/.local/share/watane-dev resolves to"),
        "the refusal does not say the folder was re-linked since the install: {out:?}"
    );
}

/// The VM run failed twice in its home comparison on files that the guest's
/// own macOS session writes outside `~/Library` during the run:
/// `~/Pictures/Photos Library.photoslibrary/resources/partialvideo` (part clean)
/// and `~/Music/Music/Music Library.musiclibrary/com.apple.MusicKit` (part clt).
/// Neither is watane's, so `home_changes` must not fail on them, while a new
/// shell file still fails.
#[test]
fn regress_s50c_home_changes_ignores_macos_media_library_writes() {
    let source =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/clean_mac.sh"))
            .expect("source");
    let function = source
        .split("home_changes() {")
        .nth(1)
        .expect("home comparison")
        .split("native_dependencies() {")
        .next()
        .expect("end");
    let temp = tempfile::tempdir().expect("temp");
    let home = temp.path().join("home");
    fs::create_dir_all(home.join("Pictures/Photos Library.photoslibrary/resources")).unwrap();
    fs::create_dir_all(home.join("Music/Music/Music Library.musiclibrary")).unwrap();
    let work = temp.path().join("work");
    fs::create_dir(&work).unwrap();
    let before = Command::new("find")
        .arg(&home)
        .arg("-print")
        .output()
        .unwrap();
    assert!(before.status.success());
    fs::write(work.join("home-before"), before.stdout).unwrap();
    fs::create_dir_all(home.join(".local/bin")).unwrap();
    fs::create_dir_all(home.join("Pictures/Photos Library.photoslibrary/resources/partialvideo"))
        .unwrap();
    fs::write(
        home.join("Music/Music/Music Library.musiclibrary/com.apple.MusicKit"),
        "",
    )
    .unwrap();
    let helper = temp.path().join("compare.sh");
    let run = |mode: &str| {
        fs::write(
            &helper,
            format!(
                "H=$1; work=$2; mode={mode}\nfail() {{ echo \"$*\" >&2; exit 1; }}\ncheck() {{ \"$@\" || fail \"$*\"; }}\nhome_changes() {{{function}\nhome_changes\n"
            ),
        )
        .unwrap();
        Command::new("/bin/sh")
            .arg(&helper)
            .arg(&home)
            .arg(&work)
            .output()
            .unwrap()
    };
    for mode in ["clean", "clt"] {
        let out = run(mode);
        assert!(
            out.status.success(),
            "{mode}: macOS's own write failed the run: {out:?}"
        );
    }
    fs::write(home.join(".zshrc"), "unexpected change").unwrap();
    let out = run("clean");
    assert_eq!(out.status.code(), Some(1), "{out:?}");
}

#[test]
fn ac1_guest_home_changes_checks_top_level_and_watane_but_fresh_stays_strict() {
    let source = include_str!("clean_mac.sh");
    let function = source
        .split("home_changes() {")
        .nth(1)
        .unwrap()
        .split("native_dependencies() {")
        .next()
        .unwrap();
    for (mode, new_path, succeeds) in [
        ("clean", "Pictures/media/cache", true),
        ("clt", ".cargo/bin/rustc", true),
        ("clean", "new-folder/file", false),
        ("clean", "Pictures/watane/cache", false),
        ("clean", ".watane/data", true),
        ("fresh", "Pictures/media/cache", false),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let work = temp.path().join("work");
        fs::create_dir_all(home.join("Pictures")).unwrap();
        fs::create_dir(&work).unwrap();
        fs::write(
            work.join("home-before"),
            format!("{}\n{}\n", home.display(), home.join("Pictures").display()),
        )
        .unwrap();
        write_file(&home.join(new_path), "new");
        let helper = temp.path().join("compare.sh");
        fs::write(&helper, format!("H=$1; work=$2; mode={mode}\nfail() {{ exit 1; }}\ncheck() {{ \"$@\" || fail; }}\nhome_changes() {{{function}\nhome_changes\n")).unwrap();
        let out = Command::new("/bin/sh")
            .arg(&helper)
            .arg(&home)
            .arg(&work)
            .output()
            .unwrap();
        assert_eq!(
            out.status.success(),
            succeeds,
            "{mode}: {new_path}: {out:?}"
        );
    }
}
