//! Adversarial tests of S50c: the release origin check and the receipt folder rule (TECHNICAL 7.2, 8.5).
mod support;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::Command;
use support::*;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("/usr/bin/git")
        .current_dir(dir)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .expect("git");
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// 7.1: "a tag whose commit is not on `main` is refused". The checkout of a tag run with
/// `fetch-depth: 0` fetches every tag to `refs/tags/*` and every branch to
/// `refs/remotes/origin/*`. git resolves the short name `origin/main` through
/// `refs/tags/` before `refs/remotes/`, so a tag named `origin/main` on the unreviewed
/// commit makes the ancestor check pass for a release tag on that commit.
#[test]
fn adv_s50c_origin_main_tag_passes_ancestor_check() {
    let yml = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../.github/workflows/release.yml"),
    )
    .unwrap();
    let check = yml
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("git merge-base --is-ancestor"))
        .expect("the ancestor check of 7.2 step 2");

    let root = tempfile::tempdir().unwrap();
    let repo = root.path();
    git(repo, &["init", "-q", "-b", "main"]);
    fs::write(repo.join("a"), "reviewed").unwrap();
    git(repo, &["add", "a"]);
    git(repo, &["commit", "-qm", "reviewed"]);
    let main = git(repo, &["rev-parse", "HEAD"]);
    git(repo, &["checkout", "-qb", "feature"]);
    fs::write(repo.join("a"), "unreviewed").unwrap();
    git(repo, &["commit", "-qam", "unreviewed"]);
    let feature = git(repo, &["rev-parse", "HEAD"]);
    // The layout actions/checkout leaves on a tag run with fetch-depth 0.
    git(repo, &["update-ref", "refs/remotes/origin/main", &main]);
    git(
        repo,
        &["update-ref", "refs/remotes/origin/feature", &feature],
    );
    git(repo, &["update-ref", "refs/tags/v0.1.0", &feature]);
    // The attacker's extra tag.
    git(repo, &["update-ref", "refs/tags/origin/main", &feature]);
    git(repo, &["checkout", "-q", "--detach", &feature]);

    let out = Command::new("/bin/bash")
        .current_dir(repo)
        .args(["-c", check])
        .env("GITHUB_SHA", &feature)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "a tag on a commit that is not on main passed `{check}`: {out:?}"
    );
}

/// 8.5 rule 4 / IN-42 / 3.4.2: "an upgrade never swaps out what a link made after the
/// install now points at". The folders are resolved once, at step 4; a link made between
/// that check and the swap (here while the run stages, held by the fake `chmod`) is
/// followed by the swap, and the commit then deletes `<path>.old.<pid>` under rule 3 only.
#[test]
fn adv_s50c_link_after_check_lets_upgrade_delete_linked_folder() {
    let fx = Fx::new();
    assert_eq!(fx.run(&["--no-toolchain"]).code, 0);
    fx.rel.set_version("0.2.0", &["serde-1.0.1"]);
    fx.bake_script("0.2.0");
    write_file(&fx.home.join("Documents/vendor/marker"), "mine");

    fx.state_write("chmod_wait", "");
    let child = spawn(&mut fx.cmd(&["--no-toolchain"]));
    fx.wait_state("chmod_waiting");
    // Another program (or the person) moves the folder away and links it to Documents.
    fs::rename(fx.share("watane-dev"), fx.home.join("old-dev")).unwrap();
    symlink(fx.home.join("Documents"), fx.share("watane-dev")).unwrap();
    fs::remove_file(fx.state.join("chmod_wait")).unwrap();
    let out = finish(child);

    assert!(
        fx.home.join("Documents/vendor/marker").exists(),
        "the upgrade replaced and deleted ~/Documents/vendor: {out:?}"
    );
}
