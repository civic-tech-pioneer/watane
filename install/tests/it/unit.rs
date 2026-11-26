//! The shell unit level (TECHNICAL 12.1): the rules of `install.sh` that a function decides,
//! each run on many cases in one `sh` that sources the functions, with no install in between.
//! The rules are INST-S-26 to 31 and 35 (TECHNICAL 3.4 to 3.6, 8.4, 8.5).

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use crate::support::unit::*;
use crate::support::*;

/// Runs `(name, body)` cases built at run time.
fn table(fx: &Fx, prelude: &str, cases: &[(String, String)]) -> Vec<Unit> {
    let refs: Vec<(&str, &str)> = cases
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    fx.units(prelude, &[], &refs)
}

/// A folder of the world, outside its home, with its physical path.
fn outside(fx: &Fx, name: &str) -> PathBuf {
    let p = fx.root.path().join(name);
    fs::create_dir_all(&p).expect("outside");
    fs::canonicalize(&p).expect("canonical")
}

/// A home `<root>/<name>` of the world, empty.
fn home_named(fx: &Fx, name: &str) -> PathBuf {
    let p = fx.root.path().join(name);
    fs::create_dir(&p).expect("home");
    p
}

/// INST-S-26, TECHNICAL 3.2: a version is `n.n.n` with an optional `-suffix` of letters, digits
/// and dots; anything else is refused with exit 1 and its own text (`check_version`).
#[test]
fn unit_check_version_accepts_versions_and_refuses_the_rest() {
    let fx = Fx::new();
    let good = [
        "0.1.0",
        "01.002.3",
        "1.2.3-rc1.A9.0",
        "10.20.30",
        "0.2.0-rc1",
    ];
    let bad = [
        "",
        "1.0",
        "1.2",
        "1.2.3.4",
        "1.2.3-",
        "1.2.3-a..b",
        "1.2.3-a-1",
        "1.2.3-a/b",
        "1.2.3-/",
        "1.2.3-rc1/../x",
        "1.2.3-.a",
        "1.2.3-a.",
        "1.2.3/x",
        "0.1.0/x",
        "a.2.3",
        "1.a.3",
        "1.2.",
        ".1.2",
        "1..2",
        "@RELEASE@",
        "1/../../../../../evil/watane/releases/download/v1",
        "1.2.3-\u{e9}",
        "1.2.3-\u{df}",
        "1.2.3 ",
        "1.2.3\n",
    ];
    let mut cases: Vec<(String, String)> = Vec::new();
    for (i, v) in good.iter().enumerate() {
        cases.push((format!("good{i}"), format!("check_version '{v}'")));
    }
    for (i, v) in bad.iter().enumerate() {
        cases.push((
            format!("bad{i}"),
            format!("LC_ALL=en_US.UTF-8; export LC_ALL; PATH=/nonexistent; check_version '{v}'"),
        ));
    }
    let refs: Vec<(&str, &str)> = cases
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let units = fx.units("", &[], &refs);
    for (i, v) in good.iter().enumerate() {
        let u = case(&units, &format!("good{i}"));
        assert_eq!((u.code, u.err.as_str()), (0, ""), "{v}: {u:?}");
    }
    for (i, v) in bad.iter().enumerate() {
        let u = case(&units, &format!("bad{i}"));
        assert_eq!(
            u.err,
            format!("watane install: not a version: {v}\na version looks like 0.1.0\n"),
            "{v}"
        );
        assert_eq!(u.code, 1, "{v}");
    }
}

/// INST-S-27 and TECHNICAL 8.5 (`allowed`): the only paths a removal may touch are the ones
/// the install writes, below the home, in a folder that resolves inside the home.
#[test]
fn unit_allowed_is_the_install_set_and_nothing_else() {
    let fx = Fx::new();
    fs::create_dir_all(fx.home.join(".local/share/watane")).expect("share");
    symlink(&fx.home, fx.home.join(".local/share/watane/x")).expect("link in");
    let other = outside(&fx, "other");
    let hout = home_named(&fx, "hout");
    symlink(&other, hout.join(".local")).expect("link out");
    let yes = [
        "$HOME/.local/bin/watane",
        "$HOME/.local/bin/watane-dev",
        "$HOME/.local/bin/watane-node",
        "$HOME/.local/bin/watane.old.99",
        "$HOME/Applications/watane.app",
        "$HOME/.local/share/watane/receipt",
        "$HOME/.local/share/watane/journal.5",
        "$HOME/.local/share/watane-dev/vendor",
    ];
    let no = [
        "$HOME/.local/bin/watane.mine",
        "$HOME/.local/bin/watane/x",
        "$HOME/.local/bin/mine",
        "$HOME/.local/share/watane",
        "$HOME/.local/share/watane-dev",
        "$HOME/.local/share/fish",
        "$HOME/Applications/other.app",
        "$HOME/Applications/watane.app/x",
        "$HOME/Applications/watane.app.new.7",
        "$HOME/.local/../Documents",
        "$HOME/.local/bin/../x",
        "",
        "relative/x",
        "$HOME/.local/share/watane//x",
        "$HOME/.local/share/watane/x/",
        "$ROOT/other/x",
        "$HOME/.local/share/watane/x/Documents",
        "/",
    ];
    let mut cases = Vec::new();
    for (i, p) in yes.iter().enumerate() {
        cases.push((format!("yes{i}"), format!("allowed \"{p}\"")));
    }
    for (i, p) in no.iter().enumerate() {
        cases.push((format!("no{i}"), format!("allowed \"{p}\"")));
    }
    // INST-S-31: an install folder that resolves outside the home is not used
    cases.push((
        "out".into(),
        "HOME=$ROOT/hout; check_home; set_entries; allowed \"$HOME/.local/bin/watane\"".into(),
    ));
    let units = table(&fx, "check_home; set_entries", &cases);
    for (i, p) in yes.iter().enumerate() {
        let u = case(&units, &format!("yes{i}"));
        assert_eq!((u.code, u.err.as_str()), (0, ""), "{p} is allowed: {u:?}");
    }
    for (i, p) in no.iter().enumerate() {
        let u = case(&units, &format!("no{i}"));
        assert_eq!(u.code, 1, "{p} is not allowed: {u:?}");
    }
    assert_eq!(
        case(&units, "out").code,
        1,
        "a folder linked out of the home"
    );
}

/// INST-S-27 and TECHNICAL 8.5 (`receipt_allowed`): a receipt line is honoured only for an
/// allowed path in a folder that resolves where it did at install; the skip is told on stderr.
#[test]
fn unit_receipt_allowed_skips_what_the_install_never_wrote() {
    let fx = Fx::new();
    outside(&fx, "other");
    // Not canonical: install.sh names the path as given (macOS: /var, not /private/var).
    let other = fx.root.path().join("other");
    let named = [
        "$HOME/.local/../Documents",
        "$HOME/.local/share/fish",
        "$HOME/.local/bin/mine",
        "$HOME/.local/share/watane",
        "$ROOT/other/x",
        "",
        "$HOME/.local/bin/watane.mine",
    ];
    let mut cases = Vec::new();
    for (i, p) in named.iter().enumerate() {
        cases.push((format!("skip{i}"), format!("receipt_allowed \"{p}\"")));
    }
    let ok = "R_RESOLVED=\"resolved .local/bin $HOME_R/.local/bin\"; receipt_allowed \"$HOME/.local/bin/watane\"";
    cases.push(("ok".into(), ok.into()));
    cases.push((
        "moved".into(),
        "R_RESOLVED='resolved .local/bin /elsewhere'; receipt_allowed \"$HOME/.local/bin/watane\""
            .into(),
    ));
    cases.push((
        "unrecorded".into(),
        "R_RESOLVED=''; receipt_allowed \"$HOME/.local/bin/watane\"".into(),
    ));
    let units = table(&fx, "check_home; set_entries", &cases);
    let h = fx.home.display().to_string();
    for (i, p) in named.iter().enumerate() {
        let u = case(&units, &format!("skip{i}"));
        let shown = p
            .replace("$HOME", "~")
            .replace("$ROOT/other", &other.display().to_string());
        assert_eq!(u.code, 1, "{p}: {u:?}");
        assert_eq!(
            u.err,
            format!("watane install: skipped {shown}: not a path that watane installs\n"),
            "{p} ({h})"
        );
    }
    let u = case(&units, "ok");
    assert_eq!((u.code, u.err.as_str()), (0, ""), "{u:?}");
    let u = case(&units, "moved");
    assert_eq!(u.code, 1);
    assert_eq!(
        u.err,
        "watane install: skipped ~/.local/bin/watane: ~/.local/bin resolves to ~/.local/bin, not to /elsewhere as at install\n"
    );
    let u = case(&units, "unrecorded");
    assert_eq!(u.code, 1);
    assert!(
        u.err.contains("not to an unrecorded path as at install"),
        "{u:?}"
    );
}

/// INST-S-35, TECHNICAL 3.6.1 (`read_journal`): a journal names paths, and only the ones of
/// the install set count; a line cut short by a kill, an unknown line or a linked file do not.
#[test]
fn unit_read_journal_honours_only_what_the_install_moves() {
    let fx = Fx::new();
    fs::write(fx.root.path().join("jreal"), "old /x\n").expect("real");
    symlink(fx.root.path().join("jreal"), fx.root.path().join("jlink")).expect("link");
    let units = fx.units_in_home(&[
        (
            "valid",
            r#"printf 'old %s\nnew %s\nmoved %s\n' "$HOME/.local/bin/watane" "$HOME/Applications/watane.app" "$HOME/.local/share/watane/receipt" >"$ROOT/j"
read_journal "$ROOT/j"
printf 'OLD<%s>\nNEW<%s>\nMOVED<%s>\n' "$J_OLD" "$J_NEW" "$J_MOVED""#,
        ),
        (
            "names_more",
            r#"printf 'old %s\nold %s\nmoved %s\nnew %s\nold %s\nnew %s\nmoved %s\n' "$HOME/Documents/d" "$HOME/.local/bin/watane.mine" "$HOME/Documents/e" "$HOME/.local/bin/watane-x" "$HOME/.local/share/watane/journal.1" "$HOME/.local/share/watane/x/receipt" "$HOME/.local/bin/../bin/watane" >"$ROOT/j"
read_journal "$ROOT/j"
printf 'OLD<%s>\nNEW<%s>\nMOVED<%s>\n' "$J_OLD" "$J_NEW" "$J_MOVED""#,
        ),
        (
            "cut_line",
            r#"printf 'old %s\nmoved %s' "$HOME/.local/bin/watane" "$HOME/.local/bin/watane-dev" >"$ROOT/j"
read_journal "$ROOT/j"
printf 'OLD<%s>\nNEW<%s>\nMOVED<%s>\n' "$J_OLD" "$J_NEW" "$J_MOVED""#,
        ),
        (
            "unknown_lines",
            r#"printf 'weird %s\n\nold\nolds %s\n  old %s\n' "$HOME/.local/bin/watane" "$HOME/.local/bin/watane" "$HOME/.local/bin/watane" >"$ROOT/j"
read_journal "$ROOT/j"
printf 'OLD<%s>\nNEW<%s>\nMOVED<%s>\n' "$J_OLD" "$J_NEW" "$J_MOVED""#,
        ),
        (
            "linked_file",
            r#"printf 'old %s\n' "$HOME/.local/bin/watane" >"$ROOT/jreal"
read_journal "$ROOT/jlink"
printf 'OLD<%s>\nNEW<%s>\nMOVED<%s>\n' "$J_OLD" "$J_NEW" "$J_MOVED""#,
        ),
        (
            "missing_file",
            r#"read_journal "$ROOT/nothing"
printf 'OLD<%s>\nNEW<%s>\nMOVED<%s>\n' "$J_OLD" "$J_NEW" "$J_MOVED""#,
        ),
    ]);
    let h = fx.home.display();
    let u = case(&units, "valid");
    assert_eq!(
        u.out,
        format!(
            "OLD<{h}/.local/bin/watane>\nNEW<{h}/Applications/watane.app>\nMOVED<{h}/.local/share/watane/receipt>\n"
        )
    );
    let empty = "OLD<>\nNEW<>\nMOVED<>\n";
    assert_eq!(
        case(&units, "names_more").out,
        empty,
        "a journal naming more than it moved"
    );
    assert_eq!(
        case(&units, "cut_line").out,
        format!("OLD<{h}/.local/bin/watane>\nNEW<>\nMOVED<>\n"),
        "a line cut short is ignored"
    );
    assert_eq!(case(&units, "unknown_lines").out, empty);
    assert_eq!(case(&units, "linked_file").out, empty);
    assert_eq!(case(&units, "missing_file").out, empty);
}

/// INST-S-31, TECHNICAL 3.4.1 (`check_folders`, `resolve_dir`, `below_home`): an install folder
/// that resolves outside the home, or to the home itself, ends the run with exit 2 and names
/// where it resolves; a link to a folder inside the home is fine.
#[test]
fn unit_check_folders_refuses_an_install_folder_outside_the_home() {
    let fx = Fx::new();
    let tail = "watane writes only under your home folder. Make it a real folder there, then run this again.\n";
    // (home, the link to make, target, the folder named, where it resolves)
    struct Out {
        folder: &'static str,
        shown: String,
        name: String,
    }
    let mut outs: Vec<Out> = Vec::new();
    let mut add = |fx: &Fx,
                   name: &str,
                   link: &str,
                   pre: &[&str],
                   to: Option<&str>,
                   folder: &'static str,
                   suffix: &str| {
        let home = home_named(fx, name);
        let other = outside(fx, &format!("{name}_o"));
        for d in pre {
            fs::create_dir_all(home.join(d)).expect("pre");
        }
        let target = match to {
            Some(t) => other.join(t),
            None => other.clone(),
        };
        symlink(&target, home.join(link)).expect("link");
        let shown = format!("{}{suffix}", other.display());
        outs.push(Out {
            folder,
            shown,
            name: name.to_string(),
        });
    };
    add(&fx, "o1", ".local", &[], None, "~/.local/bin", "/bin");
    add(
        &fx,
        "o2",
        "Applications",
        &[".local"],
        None,
        "~/Applications",
        "",
    );
    add(
        &fx,
        "o3",
        ".local/share",
        &[".local/bin"],
        None,
        "~/.local/share/watane",
        "/watane",
    );
    add(
        &fx,
        "o4",
        ".local/share/watane-dev",
        &[".local/share"],
        None,
        "~/.local/share/watane-dev",
        "",
    );
    add(
        &fx,
        "o5",
        ".local/bin",
        &[".local"],
        Some("bin"),
        "~/.local/bin",
        "/bin",
    );
    let mut cases: Vec<(String, String)> = Vec::new();
    for o in &outs {
        cases.push((
            o.name.clone(),
            format!(
                "HOME=$ROOT/{}; check_home; set_entries; check_folders",
                o.name
            ),
        ));
    }
    // a link to the home itself
    let rels = [
        (".local/bin", "~/.local/bin"),
        ("Applications", "~/Applications"),
        (".local/share/watane", "~/.local/share/watane"),
        (".local/share/watane-dev", "~/.local/share/watane-dev"),
    ];
    for (i, (rel, _)) in rels.iter().enumerate() {
        let home = home_named(&fx, &format!("self{i}"));
        fs::create_dir_all(home.join(".local/share")).expect("share");
        symlink(&home, home.join(rel)).expect("link");
        cases.push((
            format!("self{i}"),
            format!("HOME=$ROOT/self{i}; check_home; set_entries; check_folders"),
        ));
    }
    // a link to a folder inside the home is fine, and is recorded as resolved
    let ok = home_named(&fx, "inside");
    fs::create_dir(ok.join("real_local")).expect("real");
    symlink(ok.join("real_local"), ok.join(".local")).expect("link");
    cases.push((
        "inside".into(),
        "HOME=$ROOT/inside; check_home; set_entries; check_folders; printf '%s\\n' \"$FOLDERS_RESOLVED\"".into(),
    ));
    // resolve_dir, in_home, below_home on their own
    let rd = home_named(&fx, "rd");
    fs::create_dir_all(rd.join("a")).expect("a");
    symlink(rd.join("a"), rd.join("ln")).expect("ln");
    cases.push((
        "resolve".into(),
        "HOME=$ROOT/rd; check_home; resolve_dir \"$HOME/ln/new/sub\"; echo \"$RES\"; resolve_dir \"$HOME/missing\"; echo \"$RES\"; in_home \"$HOME_R\" && echo in_home_self; below_home \"$HOME_R\" || echo not_below_self; below_home \"$HOME_R/x\" && echo below_x; in_home \"$HOME_R-x\" || echo not_in_sibling; below_home / || echo not_below_root".into(),
    ));
    let units = table(&fx, "", &cases);
    for o in &outs {
        let u = case(&units, &o.name);
        assert_eq!(u.code, 2, "{}: {u:?}", o.name);
        assert_eq!(
            u.err,
            format!(
                "watane install: {} resolves to {}, outside your home folder\n{tail}",
                o.folder, o.shown
            ),
            "{}",
            o.name
        );
    }
    for (i, (_, shown)) in rels.iter().enumerate() {
        let u = case(&units, &format!("self{i}"));
        assert_eq!(u.code, 2, "{u:?}");
        assert_eq!(
            u.err,
            format!(
                "watane install: {shown} resolves to your home folder itself\nUse a real subfolder of your home folder, then run this again.\n"
            )
        );
    }
    let u = case(&units, "inside");
    let ok_r = fs::canonicalize(&ok).expect("canonical");
    assert_eq!((u.code, u.err.as_str()), (0, ""), "{u:?}");
    assert!(
        u.has_out_line(&format!(
            "resolved .local/bin {}/real_local/bin",
            ok_r.display()
        )),
        "{u:?}"
    );
    let rd_r = fs::canonicalize(&rd).expect("canonical");
    let u = case(&units, "resolve");
    assert_eq!(
        u.out,
        format!(
            "{0}/a/new/sub\n{0}/missing\nin_home_self\nnot_below_self\nbelow_x\nnot_in_sibling\nnot_below_root\n",
            rd_r.display()
        )
    );
}

/// INST-S-28, TECHNICAL 8.4 step 5 (`home_deletable`): only a plain folder that resolves inside
/// the home, and is neither the home nor above it, may be deleted; one that resolves outside
/// the home is the person's to delete (`HOME_OUT`).
#[test]
fn unit_home_deletable_refuses_what_is_not_a_watane_home() {
    let fx = Fx::new();
    let other = outside(&fx, "other");
    fs::write(other.join("marker"), "m").expect("marker");
    fs::create_dir(fx.home.join("x")).expect("x");
    fs::create_dir(fx.home.join("elsewhere")).expect("elsewhere");
    symlink(&fx.home, fx.home.join("link")).expect("link");
    symlink(&other, fx.home.join("olink")).expect("olink");
    symlink(fx.home.join("elsewhere"), fx.home.join("ilink")).expect("ilink");
    // a link to a folder whose name holds a line break: its resolved path is not plain
    fs::create_dir(fx.home.join("a\nb")).expect("a\\nb");
    symlink(fx.home.join("a\nb"), fx.home.join("nllink")).expect("nllink");
    let values = [
        ("home", "$HOME"),
        ("root", "/"),
        ("dotdot", "$HOME/x/.."),
        ("dotdot_elsewhere", "$HOME/x/../elsewhere"),
        ("dot_elsewhere", "$HOME/./elsewhere"),
        ("slash_elsewhere", "$HOME/elsewhere/"),
        ("double_slash", "$HOME//elsewhere"),
        ("link", "$HOME/link"),
        ("linkslash", "$HOME/link/"),
        ("above", "$ROOT"),
        ("missing", "$HOME/nothing"),
        ("relative", "elsewhere"),
        ("empty", ""),
        ("plain", "$HOME/elsewhere"),
        ("ilink", "$HOME/ilink"),
        ("olink", "$HOME/olink"),
        ("nllink", "$HOME/nllink"),
    ];
    let cases: Vec<(String, String)> = values
        .iter()
        .map(|(n, v)| {
            (
                (*n).to_string(),
                format!(
                    "HOME_OUT=0; HOME_DEL=; if home_deletable \"{v}\"; then rc=0; else rc=$?; fi; echo \"rc=$rc out=$HOME_OUT del=$HOME_DEL\"; exit $rc"
                ),
            )
        })
        .collect();
    let units = table(&fx, "check_home; set_entries", &cases);
    let real = |p: &Path| {
        fs::canonicalize(p)
            .expect("canonical")
            .display()
            .to_string()
    };
    let el = real(&fx.home.join("elsewhere"));
    let want = [
        ("home", 1, 0, String::new()),
        ("root", 1, 0, String::new()),
        ("dotdot", 1, 0, String::new()),
        ("dotdot_elsewhere", 1, 0, String::new()),
        ("dot_elsewhere", 1, 0, String::new()),
        ("slash_elsewhere", 1, 0, String::new()),
        ("double_slash", 1, 0, String::new()),
        ("link", 1, 0, String::new()),
        ("linkslash", 1, 0, String::new()),
        ("above", 1, 0, String::new()),
        ("missing", 1, 0, String::new()),
        ("relative", 1, 0, String::new()),
        ("empty", 1, 0, String::new()),
        ("plain", 0, 0, el.clone()),
        ("ilink", 0, 0, el),
        ("olink", 1, 1, real(&other)),
        ("nllink", 1, 0, String::new()),
    ];
    for (n, rc, out, del) in want {
        let u = case(&units, n);
        assert_eq!(u.code, rc, "{n}: {u:?}");
        let got = u.out.trim();
        assert!(
            got.starts_with(&format!("rc={rc} out={out} del=")),
            "{n}: {got}"
        );
        if rc == 0 || n == "olink" {
            assert!(got.ends_with(&format!("del={del}")), "{n}: {got}");
        }
    }
}

/// TECHNICAL 8.4, 3.9 (`tilde`, `tilde_r`, `plain_path`, `shell_quote`): the home is shown as `~`,
/// a path is plain or refused, and a text is quoted for a command the person runs.
#[test]
fn unit_path_and_text_helpers() {
    let fx = Fx::new();
    let units = fx.units_in_home(&[
        (
            "tilde",
            r#"tilde "$HOME"; echo; tilde "$HOME/a b"; echo; tilde "$HOME-x/y"; echo; tilde /etc; echo; tilde_r "$HOME_R"; echo; tilde_r "$HOME_R/z"; echo"#,
        ),
        (
            "plain",
            r#"for p in /a /a/b '' a /a/ /a//b /a/./b /a/../b /a/. /a/.. '/a
b' /a/.b /a/..b; do if plain_path "$p"; then echo "plain:$p"; else echo "refused:$p"; fi; done"#,
        ),
        (
            "quote",
            r#"shell_quote "plain"; echo; shell_quote "it's"; echo; shell_quote '~/x'; echo; shell_quote '=v'; echo; shell_quote 'a b$c'; echo; shell_quote ''; echo"#,
        ),
        (
            "exists_list",
            r#"L=$(add_item "" a); L=$(add_item "$L" b); in_list "$L" a && echo a; in_list "$L" b && echo b; in_list "$L" ab || echo not_ab; in_list "$L" '' || echo not_empty; printf '%s\n' "$L""#,
        ),
    ]);
    let h = fx.home.display();
    assert_eq!(
        case(&units, "tilde").out,
        format!("~\n~/a b\n{h}-x/y\n/etc\n~\n~/z\n")
    );
    let plain = case(&units, "plain").out.clone();
    let want = [
        "plain:/a",
        "plain:/a/b",
        "refused:",
        "refused:a",
        "refused:/a/",
        "refused:/a//b",
        "refused:/a/./b",
        "refused:/a/../b",
        "refused:/a/.",
        "refused:/a/..",
        "refused:/a",
        "b",
        "plain:/a/.b",
        "plain:/a/..b",
    ];
    assert_eq!(plain.lines().collect::<Vec<_>>(), want, "{plain}");
    assert_eq!(
        case(&units, "quote").out,
        "'plain'\n'it'\\''s'\n'~/x'\n'=v'\n'a b$c'\n''\n"
    );
    assert_eq!(
        case(&units, "exists_list").out,
        "a\nb\nnot_ab\nnot_empty\na\nb\n"
    );
}

/// A folder `<root>/<name>` with the archive `a.tar.gz` of these bytes and a `SHA256SUMS` of this text.
fn release_dir(fx: &Fx, name: &str, archive: &[u8], sums: &str) {
    let d = fx.root.path().join(name);
    fs::create_dir(&d).expect("release dir");
    fs::write(d.join("a.tar.gz"), archive).expect("archive");
    fs::write(d.join("SHA256SUMS"), sums).expect("sums");
}

/// INST-S-8, 9 and TECHNICAL 3.4 step 8 (`verify_archive`): the archive is used only when
/// `SHA256SUMS` has exactly one line for it and the sum matches (exit 4 otherwise, and nothing
/// was written).
#[test]
fn unit_verify_archive_needs_one_matching_line() {
    let fx = Fx::new();
    let bytes = b"not really an archive".to_vec();
    let sum = sha256(&bytes);
    let zeros = "0".repeat(64);
    let sums: [(&str, String); 10] = [
        ("ok", format!("{sum}  a.tar.gz\n")),
        (
            "ok_fields",
            format!("{zeros} other.tar.gz\n{sum}\ta.tar.gz\textra fields\n"),
        ),
        ("ok_no_newline", format!("{sum}  a.tar.gz")),
        ("mismatch", format!("{zeros}  a.tar.gz\n")),
        ("no_line", format!("{zeros}  other.tar.gz\n")),
        ("empty", String::new()),
        ("binary_mark", format!("{sum} *a.tar.gz\n")),
        (
            "two_lines",
            format!("{zeros}  a.tar.gz\n{zeros}  a.tar.gz\n"),
        ),
        (
            "two_matching",
            format!("{sum}  a.tar.gz\n{sum}  a.tar.gz\n"),
        ),
        (
            "mismatch_after_other",
            format!("{sum}  other.tar.gz\n{zeros}  a.tar.gz\n"),
        ),
    ];
    let mut cases: Vec<(String, String)> = Vec::new();
    for (name, text) in &sums {
        release_dir(&fx, name, &bytes, text);
        cases.push((
            (*name).to_string(),
            format!("TMP=$ROOT/{name}; ARCHIVE=a.tar.gz; RELEASE=0.1.0; verify_archive"),
        ));
    }
    // a receipt means a version was installed: the last line says so
    release_dir(&fx, "changed", &bytes, &format!("{zeros}  a.tar.gz\n"));
    cases.push((
        "changed".into(),
        "TMP=$ROOT/changed; ARCHIVE=a.tar.gz; RELEASE=0.1.0; mkdir -p \"$SHAREW\"; : >\"$RECEIPT\"; verify_archive".into(),
    ));
    let units = table(&fx, "check_home; set_entries", &cases);
    for name in ["ok", "ok_fields", "ok_no_newline"] {
        let u = case(&units, name);
        assert_eq!(u.code, 0, "{name}: {u:?}");
        assert_eq!(
            u.out,
            "checked: sha256 matches SHA256SUMS of release v0.1.0\n"
        );
        assert_eq!(u.err, "");
    }
    for name in ["mismatch", "mismatch_after_other"] {
        let u = case(&units, name);
        assert_eq!(u.code, 4, "{name}: {u:?}");
        let l: Vec<&str> = u.err.lines().collect();
        assert_eq!(
            l[0], "watane install: checksum mismatch for a.tar.gz",
            "{name}"
        );
        assert_eq!(l[1], format!("expected {zeros}"));
        assert_eq!(l[2], format!("got {sum}"));
        assert_eq!(l[3], "Nothing was installed.");
        assert_eq!(u.out, "");
    }
    for name in ["no_line", "empty", "binary_mark"] {
        let u = case(&units, name);
        assert_eq!(u.code, 4, "{name}: {u:?}");
        assert_eq!(
            u.err, "watane install: SHA256SUMS has no line for a.tar.gz\nNothing was installed.\n",
            "{name}"
        );
    }
    for name in ["two_lines", "two_matching"] {
        let u = case(&units, name);
        assert_eq!(u.code, 4, "{name}: {u:?}");
        assert_eq!(
            u.err,
            "watane install: SHA256SUMS has more than one line for a.tar.gz\nNothing was installed.\n",
            "{name}"
        );
    }
    let u = case(&units, "changed");
    assert_eq!(u.code, 4);
    assert!(u.err.ends_with("Nothing was changed.\n"), "{u:?}");
}

/// INST-S-9 and TECHNICAL 3.4 step 9 (`extract_archive`): an archive is unpacked only when it
/// is a watane archive of this version that holds every entry and no name that leaves its folder
/// and no link (exit 5 otherwise, and nothing was unpacked).
#[test]
fn unit_extract_archive_refuses_what_leaves_its_folder() {
    let fx = Fx::new();
    let top = "watane-0.1.0-aarch64-apple-darwin";
    let outside = fx.root.path().join("outside-file");
    write_file(&outside, "outside\n");
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o600)).expect("chmod");
    let o = outside.clone();
    let abs = fx.root.path().join("evil-abs").display().to_string();
    let link_kinds = ["symlink", "hardlink", "dirlink"];
    let mut archives: Vec<(String, Vec<u8>)> = vec![
        ("good".into(), build_archive("0.1.0", &["serde-1.0.0"])),
        ("abs".into(), archive_with_member(&abs)),
        (
            "rel".into(),
            archive_with_member(&format!("{top}/../../evil-rel")),
        ),
        ("up".into(), archive_with_member("../evil-up")),
        ("garbage".into(), b"garbage".to_vec()),
        (
            "other_version".into(),
            build_archive_with("0.1.0", &[], &|t| {
                fs::write(t.join("VERSION"), "0.0.9\n").expect("version");
            }),
        ),
        (
            "missing_entry".into(),
            build_archive_with("0.1.0", &[], &|t| {
                fs::remove_file(t.join("bin/watane-node")).expect("rm");
            }),
        ),
    ];
    for kind in link_kinds {
        let o = o.clone();
        archives.push((
            kind.to_string(),
            build_archive_with("0.1.0", &["serde-1.0.0"], &move |t| match kind {
                "symlink" => {
                    fs::remove_file(t.join("bin/watane")).expect("rm");
                    symlink(&o, t.join("bin/watane")).expect("link");
                }
                "hardlink" => {
                    fs::remove_file(t.join("bin/watane")).expect("rm");
                    fs::hard_link(t.join("bin/watane-dev"), t.join("bin/watane")).expect("hard");
                }
                _ => {
                    symlink(&o, t.join("share/watane-dev/vendor/serde-1.0.0/elsewhere"))
                        .expect("link");
                }
            }),
        ));
    }
    let mut cases = Vec::new();
    for (name, bytes) in &archives {
        let d = fx.root.path().join(name);
        fs::create_dir(&d).expect("dir");
        fs::write(d.join("a.tar.gz"), bytes).expect("archive");
        cases.push((
            name.clone(),
            format!(
                "TMP=$ROOT/{name}; ARCHIVE=a.tar.gz; RELEASE=0.1.0; extract_archive; echo \"TOP=$TOP\""
            ),
        ));
    }
    let units = table(&fx, "check_home; set_entries", &cases);
    let u = case(&units, "good");
    assert_eq!((u.code, u.err.as_str()), (0, ""), "{u:?}");
    assert_eq!(
        u.out,
        format!("TOP={}/good/x/{top}\n", fx.root.path().display())
    );
    assert!(
        fx.root
            .path()
            .join("good/x")
            .join(top)
            .join("bin/watane")
            .is_file()
    );
    let refused = [
        ("abs", "holds a file name that leaves its folder"),
        ("rel", "holds a file name that leaves its folder"),
        ("up", "holds a file name that leaves its folder"),
        ("garbage", "is not a watane archive"),
        ("other_version", "is not a watane archive of version 0.1.0"),
        (
            "missing_entry",
            "is not a watane archive: bin/watane-node is missing",
        ),
        (
            "symlink",
            "holds a symbolic or hard link, which a watane archive never does",
        ),
        (
            "hardlink",
            "holds a symbolic or hard link, which a watane archive never does",
        ),
        (
            "dirlink",
            "holds a symbolic or hard link, which a watane archive never does",
        ),
    ];
    for (name, why) in refused {
        let u = case(&units, name);
        assert_eq!(u.code, 5, "{name}: {u:?}");
        assert_eq!(
            u.err,
            format!("watane install: a.tar.gz {why}\nNothing was installed.\n"),
            "{name}"
        );
    }
    for name in [
        "abs", "rel", "up", "garbage", "symlink", "hardlink", "dirlink",
    ] {
        assert!(
            !fx.root.path().join(name).join("x").exists(),
            "{name}: nothing was unpacked"
        );
    }
    let evil: Vec<String> = fx
        .tree(fx.root.path())
        .into_iter()
        .filter(|n| n.contains("evil"))
        .collect();
    assert!(evil.is_empty(), "{evil:?}");
    assert_eq!(fx.read(&outside), "outside\n");
    let mode = fs::metadata(&outside).expect("meta").permissions().mode();
    assert_eq!(mode & 0o777, 0o600, "the outside file keeps its mode");
}

fn foreign_lines(u: &Unit, shown: &[&str]) {
    let l: Vec<&str> = u.err.lines().collect();
    assert_eq!(
        l[0],
        "watane install: found files that watane did not install:"
    );
    let n = shown.len();
    for (i, p) in shown.iter().enumerate() {
        assert_eq!(l[1 + i], format!("  {p}"));
        assert_eq!(l[2 + n + i], format!("  mv {p} {p}.mine"));
    }
    assert_eq!(
        l[1 + n],
        "Move them aside, then run this again, for example:"
    );
    assert_eq!(l.len(), 2 + 2 * n, "{u:?}");
}

/// INST-S-30, 35 and TECHNICAL 3.4.2 (`check_foreign`): a file at a name the install writes that
/// watane did not install ends the run with exit 8 and the `mv` that moves it aside; a file the
/// receipt or a dead run's journal says it installed is not foreign. Nothing is written.
#[test]
fn unit_check_foreign_refuses_a_file_watane_did_not_install() {
    let fx = Fx::new();
    let dead = dead_pid();
    // a path of the install set at each name, none of them installed by watane
    let foreign = home_named(&fx, "foreign");
    write_file(&foreign.join(".local/bin/watane"), "mine");
    fs::create_dir_all(foreign.join("Applications/watane.app")).expect("app");
    symlink("/nonexistent/target", foreign.join(".local/bin/watane-dev")).expect("link");
    let before = snapshot(&foreign);
    // the receipt lacks a path that exists
    let vendor = fx.installed_home("vendor");
    let receipt = vendor.join(".local/share/watane/receipt");
    let text = fs::read_to_string(&receipt).expect("receipt");
    let kept: Vec<&str> = text
        .lines()
        .filter(|l| !l.ends_with("watane-dev/vendor"))
        .collect();
    fs::write(&receipt, format!("{}\n", kept.join("\n"))).expect("write");
    // a receipt that is a folder
    let rdir = home_named(&fx, "rdir");
    fs::create_dir_all(rdir.join(".local/share/watane/receipt")).expect("receipt dir");
    // an install that was killed at its receipt: its journal says what it moved in
    let killed = home_named(&fx, "killed");
    write_file(
        &killed.join(".local/bin/watane"),
        "moved in by the dead run",
    );
    write_file(
        &killed.join(format!(".local/share/watane/journal.{dead}")),
        &format!(
            "new {0}/.local/bin/watane\nmoved {0}/.local/bin/watane\n",
            killed.display()
        ),
    );
    // a `new` line with no `moved` line does not make a foreign app watane's
    let unmoved = home_named(&fx, "unmoved");
    write_file(&unmoved.join("Applications/watane.app/marker"), "m");
    write_file(
        &unmoved.join(format!(".local/share/watane/journal.{dead}")),
        &format!("new {}/Applications/watane.app\n", unmoved.display()),
    );
    // an installed home, and a journal that names more than it moved (INST-S-35)
    let installed = fx.installed_home("installed");
    let sw = installed.join(".local/share/watane");
    write_file(
        &sw.join(format!("journal.{dead}")),
        &format!(
            "old {0}/.local/bin/watane\nnew {0}/Applications/watane.app\nold {0}/Documents/d\nmoved {0}/Documents/e\nnew {0}/.local/bin/watane-x",
            installed.display()
        ),
    );
    // the receipt's folder identity: a folder that now resolves elsewhere
    let relinked = fx.installed_home("relinked");
    fs::rename(
        relinked.join(".local/share/watane-dev"),
        relinked.join("old-dev"),
    )
    .expect("move");
    write_file(&relinked.join("Documents/vendor/marker"), "mine");
    symlink(
        relinked.join("Documents"),
        relinked.join(".local/share/watane-dev"),
    )
    .expect("link");
    let run = |name: &str, release: &str| {
        (
            name.to_string(),
            format!(
                "HOME=$ROOT/{name}; check_home; set_entries; RELEASE={release}; check_foreign; rc=$?; echo \"UPGRADED=$UPGRADED\"; exit $rc"
            ),
        )
    };
    let again = fx.installed_home("again");
    assert!(again.is_dir());
    let at = |name: &str, release: &str| {
        (
            name.to_string(),
            format!(
                "HOME=$ROOT/again; check_home; set_entries; RELEASE={release}; check_foreign; echo \"UPGRADED=$UPGRADED\""
            ),
        )
    };
    let cases = vec![
        run("foreign", "0.1.0"),
        run("vendor", "0.1.0"),
        run("rdir", "0.1.0"),
        run("killed", "0.1.0"),
        run("unmoved", "0.1.0"),
        run("installed", "0.1.0"),
        run("relinked", "0.1.0"),
        (
            "fresh".to_string(),
            "HOME=$ROOT/fresh; mkdir -p \"$HOME\"; check_home; set_entries; RELEASE=0.1.0; check_foreign; echo \"UPGRADED=$UPGRADED\"".to_string(),
        ),
        at("same", "0.1.0"),
        at("upgrade", "0.2.0"),
    ];
    let units = table(&fx, "UPGRADED=0", &cases);
    let u = case(&units, "foreign");
    assert_eq!(u.code, 8, "{u:?}");
    foreign_lines(
        u,
        &[
            "~/.local/bin/watane",
            "~/.local/bin/watane-dev",
            "~/Applications/watane.app",
        ],
    );
    assert_eq!(snapshot(&foreign), before, "nothing was written");
    let u = case(&units, "vendor");
    assert_eq!(u.code, 8, "{u:?}");
    foreign_lines(u, &["~/.local/share/watane-dev/vendor"]);
    let u = case(&units, "rdir");
    assert_eq!(u.code, 8, "{u:?}");
    foreign_lines(u, &["~/.local/share/watane/receipt"]);
    let u = case(&units, "killed");
    assert_eq!((u.code, u.err.as_str()), (0, ""), "{u:?}");
    let u = case(&units, "unmoved");
    assert_eq!(u.code, 8, "{u:?}");
    foreign_lines(u, &["~/Applications/watane.app"]);
    let u = case(&units, "installed");
    assert_eq!((u.code, u.err.as_str()), (0, ""), "{u:?}");
    assert!(u.has_out_line("reinstalling watane 0.1.0"), "{u:?}");
    let u = case(&units, "relinked");
    assert_eq!(u.code, 8, "{u:?}");
    assert!(
        u.has_err_line("watane install: ~/.local/share/watane-dev resolves to ~/Documents, not to ~/.local/share/watane-dev as at install"),
        "{u:?}"
    );
    let u = case(&units, "fresh");
    assert_eq!(
        (u.code, u.err.as_str(), u.out.as_str()),
        (0, "", "UPGRADED=0\n")
    );
    let u = case(&units, "same");
    assert_eq!(u.out, "reinstalling watane 0.1.0\nUPGRADED=0\n");
    let u = case(&units, "upgrade");
    assert_eq!(u.out, "upgrading watane 0.1.0 to 0.2.0\nUPGRADED=1\n");
}

/// INST-S-35 and TECHNICAL 3.6.2 (`take_lock`, `check_no_live_install`): a second run is stopped
/// by the lock, and by the journal of a live run, with exit 5 and the command that frees the lock;
/// a lock that is found is never broken.
#[test]
fn unit_take_lock_and_the_live_journal_stop_a_second_run() {
    let fx = Fx::new();
    let mut live = std::process::Command::new(fx.real.join("sleep"))
        .arg("600")
        .spawn()
        .expect("sleep");
    let live_pid = live.id();
    let dead = dead_pid();
    let mut cases: Vec<(String, String)> = Vec::new();
    // the lock: the first run takes it, the second stops, the lock stays the first run's
    let twice = home_named(&fx, "twice");
    cases.push((
        "twice".into(),
        "HOME=$ROOT/twice; check_home; set_entries; take_lock; echo \"PID=$$\"; ( take_lock )"
            .into(),
    ));
    // planted locks: with the pid of a run, a dead pid, no pid, a pid that is not one line, text
    let plants: [(&str, Option<&str>); 5] = [
        ("p_dead", Some("99999999\n")),
        ("p_none", None),
        ("p_two", Some("12\n34\n")),
        ("p_text", Some("abc\n")),
        ("p_live", Some("")),
    ];
    for (name, pid) in plants {
        let home = home_named(&fx, name);
        fs::create_dir(home.join(".watane-install.lock")).expect("lock");
        match (name, pid) {
            ("p_live", _) => {
                fs::write(
                    home.join(".watane-install.lock/pid"),
                    format!("{live_pid}\n"),
                )
                .expect("pid");
            }
            (_, Some(p)) => fs::write(home.join(".watane-install.lock/pid"), p).expect("pid"),
            _ => {}
        }
        cases.push((
            name.to_string(),
            format!("HOME=$ROOT/{name}; check_home; set_entries; take_lock"),
        ));
    }
    // a lock whose pid file cannot be written stops the run and is removed again
    let nowrite = home_named(&fx, "nowrite");
    cases.push((
        "nowrite".into(),
        "HOME=$ROOT/nowrite; check_home; set_entries; trap cleanup EXIT; mkdir() { command mkdir \"$@\" && : >\"$1/pid\"; }; take_lock".into(),
    ));
    // the journal of a live run, of a dead one, of this one, and names that are not journals
    let j = home_named(&fx, "j_live");
    write_file(
        &j.join(format!(".local/share/watane/journal.{live_pid}")),
        "old x\n",
    );
    let jd = home_named(&fx, "j_dead");
    write_file(
        &jd.join(format!(".local/share/watane/journal.{dead}")),
        "old x\n",
    );
    let jo = home_named(&fx, "j_other");
    for n in ["journal.abc", "journal.", "journal.1x", "receipt.new.1"] {
        write_file(&jo.join(".local/share/watane").join(n), "x");
    }
    fs::create_dir_all(jo.join(format!(".local/share/watane/journal.{live_pid}"))).expect("dir");
    for n in ["j_live", "j_dead", "j_other"] {
        cases.push((
            n.to_string(),
            format!("HOME=$ROOT/{n}; check_home; set_entries; check_no_live_install; echo \"JOURNALS=$JOURNALS\""),
        ));
    }
    cases.push((
        "j_own".into(),
        "HOME=$ROOT/j_dead; check_home; set_entries; printf 'old x\\n' >\"$SHAREW/journal.$$\"; check_no_live_install; rm -f \"$SHAREW/journal.$$\"; echo own_pid_is_gone".into(),
    ));
    let units = table(&fx, "", &cases);
    live.kill().expect("kill");
    live.wait().expect("wait");
    let u = case(&units, "twice");
    let pid: String = u
        .out
        .trim()
        .strip_prefix("PID=")
        .expect("pid line")
        .to_string();
    assert_eq!(u.code, 5, "{u:?}");
    assert_eq!(
        u.err,
        format!(
            "watane install: another install may be running (pid {pid})\nIf no install is running, remove its lock and run this again:\n  rm -rf ~/.watane-install.lock\n"
        )
    );
    assert!(
        twice.join(".watane-install.lock/pid").is_file(),
        "the lock stays"
    );
    for (name, suffix) in [
        ("p_dead", " (pid 99999999)".to_string()),
        ("p_none", String::new()),
        ("p_two", String::new()),
        ("p_text", String::new()),
        ("p_live", format!(" (pid {live_pid})")),
    ] {
        let u = case(&units, name);
        assert_eq!(u.code, 5, "{name}: {u:?}");
        assert_eq!(
            u.err,
            format!(
                "watane install: another install may be running{suffix}\nIf no install is running, remove its lock and run this again:\n  rm -rf ~/.watane-install.lock\n"
            ),
            "{name}"
        );
        assert!(
            fx.root
                .path()
                .join(name)
                .join(".watane-install.lock")
                .is_dir(),
            "{name}: a lock that is found is never broken"
        );
    }
    let u = case(&units, "nowrite");
    assert_eq!(u.code, 5, "{u:?}");
    assert!(
        u.has_err_line("watane install: could not write install lock"),
        "{u:?}"
    );
    assert!(
        !nowrite.join(".watane-install.lock").exists(),
        "the lock is removed again"
    );
    let u = case(&units, "j_live");
    assert_eq!(u.code, 5, "{u:?}");
    assert_eq!(
        u.first_err(),
        format!("watane install: another install is running (pid {live_pid})")
    );
    assert!(u.err.contains(&format!("journal.{live_pid}")), "{u:?}");
    assert_eq!(case(&units, "j_dead").out, format!("JOURNALS={dead}\n"));
    assert_eq!(case(&units, "j_dead").code, 0);
    assert_eq!(case(&units, "j_other").out, "JOURNALS=\n");
    assert_eq!(case(&units, "j_other").code, 0);
    assert_eq!(case(&units, "j_own").out, "own_pid_is_gone\n");
}

/// INST-S-20, 21 and TECHNICAL 3.4 steps 3 and 4 (`check_platform`, `check_tools`): only macOS 14
/// or later on Apple silicon (also a shell under Rosetta) is served, with exit 2 and the platform
/// named; a tool that is missing is named.
#[test]
fn unit_check_platform_and_tools() {
    let fx = Fx::new();
    let set = |s: &str, m: &str, v: &str, r: &str| {
        format!(
            "printf '%s' '{s}' >\"$FAKE_STATE/uname_s\"; printf '%s' '{m}' >\"$FAKE_STATE/uname_m\"; printf '%s' '{v}' >\"$FAKE_STATE/sw_vers\"; printf '%s' '{r}' >\"$FAKE_STATE/rosetta\"; check_platform"
        )
    };
    let units = fx.units(
        "",
        &[],
        &[
            ("ok", &set("Darwin", "arm64", "14.5", "0")),
            ("ok_14", &set("Darwin", "arm64", "14", "0")),
            ("ok_new", &set("Darwin", "arm64", "26.1", "0")),
            ("rosetta", &set("Darwin", "x86_64", "14.5", "1")),
            ("linux", &set("Linux", "x86_64", "14.5", "0")),
            ("intel", &set("Darwin", "x86_64", "14.5", "0")),
            ("old", &set("Darwin", "arm64", "12.7", "0")),
            ("odd_version", &set("Darwin", "arm64", "abc", "0")),
            ("odd_machine", &set("Darwin", "riscv", "14.5", "0")),
            ("tools", "check_tools 'sh nonexistent-tool-xyz'"),
            ("tools_ok", "check_tools 'sh cat'"),
        ],
    );
    for n in ["ok", "ok_14", "ok_new", "rosetta", "tools_ok"] {
        let u = case(&units, n);
        assert_eq!((u.code, u.err.as_str()), (0, ""), "{n}: {u:?}");
    }
    for (n, shown) in [
        ("linux", "Linux x86_64"),
        ("intel", "Darwin x86_64"),
        ("old", "macOS 12.7 (needs 14.0 or later)"),
        ("odd_version", "macOS abc (needs 14.0 or later)"),
        ("odd_machine", "Darwin riscv"),
    ] {
        let u = case(&units, n);
        assert_eq!(u.code, 2, "{n}: {u:?}");
        assert_eq!(
            u.err,
            format!(
                "watane install: unsupported platform: {shown}\nwatane installs on macOS 14.0 or later, on Apple silicon.\n"
            )
        );
    }
    let u = case(&units, "tools");
    assert_eq!(u.code, 2);
    assert_eq!(
        u.err,
        "watane install: the tool nonexistent-tool-xyz is missing\nwatane install uses only the tools that stock macOS ships; nonexistent-tool-xyz was not found on your PATH.\n"
    );
}

/// INST-S-18 and TECHNICAL 3.10 (`do_status`, `usage`): `--status` prints the version, target and
/// paths of the receipt, with `~` for the home, or exits 7 when there is no receipt.
#[test]
fn unit_status_and_usage() {
    let fx = Fx::new();
    let installed = fx.installed_home("installed");
    let bad = home_named(&fx, "bad");
    write_file(
        &bad.join(".local/share/watane/receipt"),
        "watane-receipt 2\nversion 9\n",
    );
    let _ = installed;
    let units = fx.units(
        "",
        &[],
        &[
            (
                "none",
                "HOME=$ROOT/none; mkdir -p \"$HOME\"; check_home; set_entries; do_status",
            ),
            ("bad", "HOME=$ROOT/bad; check_home; set_entries; do_status"),
            (
                "bad_uninstall",
                "HOME=$ROOT/bad; check_home; set_entries; do_uninstall",
            ),
            (
                "installed",
                "HOME=$ROOT/installed; check_home; set_entries; do_status",
            ),
            ("usage", "usage"),
        ],
    );
    for (name, want) in [
        ("none", "not installed: no receipt"),
        ("bad", "not installed: unrecognised receipt"),
        (
            "bad_uninstall",
            "nothing to uninstall: unrecognised receipt",
        ),
    ] {
        let u = case(&units, name);
        assert_eq!(u.code, 7, "{name}: {u:?}");
        assert_eq!(
            u.err,
            format!("watane install: {want} at ~/.local/share/watane/receipt\n"),
            "{name}"
        );
    }
    assert_eq!(
        fs::read_to_string(bad.join(".local/share/watane/receipt")).expect("unrecognised receipt"),
        "watane-receipt 2\nversion 9\n",
    );
    let u = case(&units, "installed");
    assert_eq!(u.code, 0, "{u:?}");
    assert_eq!(
        u.out,
        "watane 0.1.0 (aarch64-apple-darwin)\n~/.local/bin/watane\n~/.local/bin/watane-dev\n~/.local/bin/watane-node\n~/.local/share/watane-dev/vendor\n~/.local/share/watane/install.sh\n~/.local/share/watane/VERSION\n~/Applications/watane.app\n"
    );
    assert!(case(&units, "usage").out.starts_with("usage: install.sh"));
}

/// INST-S-4, 5, 19, 22, 34 and TECHNICAL 3.7, 3.9, 3.10 (`final_output`, `print_commands`): what a
/// finished install tells the person, for each state: the PATH block for the shell, the
/// quarantine step, the command line tools, the toolchain commands for each state, and the
/// installer URL quoted for a shell.
#[test]
fn unit_what_the_end_of_an_install_says() {
    let fx = Fx::new();
    let end = |shell: &str, path: &str, extra: &str| {
        format!("RELEASE=0.1.0; UPGRADED=0; TC_OK=1; {extra} SHELL='{shell}'; {path}; final_output")
    };
    let on_path = "PATH=\"$PATH:$PREFIX/bin\"";
    let off_path = "PATH=\"$PATH\"";
    let toolchain = |state: &str, url: &str| {
        format!("TC_STATE={state}; WATANE_RUSTUP_URL='{url}'; print_commands")
    };
    let units = fx.units_in_home(&[
        ("zsh", &end("/bin/zsh", off_path, "")),
        ("bash", &end("/bin/bash", off_path, "")),
        ("other", &end("/bin/fish", off_path, "")),
        ("on_path", &end("/bin/zsh", on_path, "")),
        ("upgraded", &end("/bin/zsh", off_path, "UPGRADED=1;")),
        ("no_toolchain", &end("/bin/zsh", off_path, "TC_OK=0;")),
        (
            "no_clt",
            &end(
                "/bin/zsh",
                off_path,
                "printf x >\"$FAKE_STATE/xcode_missing\";",
            ),
        ),
        ("a_default", &toolchain("none_a", "")),
        (
            "a_url",
            &toolchain("none_a", "https://example.invalid/rustup-init.sh"),
        ),
        (
            "a_file",
            &toolchain("none_a", "file:///tmp/x/rustup-init.sh"),
        ),
        ("a_odd", &toolchain("none_a", "http://x/a b;c")),
        ("a_tilde", &toolchain("none_a", "~root/x")),
        ("a_equals", &toolchain("none_a", "=x")),
        ("target", &toolchain("target", "")),
        ("none_b", &toolchain("none_b", "")),
    ]);
    let l = |u: &Unit, s: &str| assert!(u.has_out_line(s), "{s:?} not in {u:?}");
    let u = case(&units, "zsh");
    l(
        u,
        "~/.local/bin is not on your PATH, so a new terminal will not find watane-dev yet.",
    );
    l(
        u,
        r#"  echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc"#,
    );
    l(
        u,
        "    xattr -dr com.apple.quarantine ~/Applications/watane.app",
    );
    l(
        u,
        "  ~/.local/bin/watane-dev doctor              checks the maker tools",
    );
    l(u, "watane 0.1.0 is installed.");
    assert!(!u.out.contains("xcode-select"), "{u:?}");
    assert!(!u.out.contains("install the toolchain first"), "{u:?}");
    assert!(!u.out.contains("quit it and open it again"), "{u:?}");
    l(
        case(&units, "bash"),
        r#"  echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bash_profile"#,
    );
    let u = case(&units, "other");
    l(
        u,
        "Add it to your shell's startup file, then open a new terminal:",
    );
    l(u, r#"  export PATH="$HOME/.local/bin:$PATH""#);
    let u = case(&units, "on_path");
    assert!(!u.out.contains("is not on your PATH"), "{u:?}");
    l(
        u,
        "  watane-dev doctor                     checks the maker tools",
    );
    l(
        case(&units, "upgraded"),
        "If watane is open, quit it and open it again.",
    );
    l(
        case(&units, "no_toolchain"),
        "  (install the toolchain first: see above)",
    );
    let u = case(&units, "no_clt");
    let lines: Vec<&str> = u.out.lines().collect();
    assert_eq!(
        lines[lines.len() - 2..],
        [
            "watane-dev needs the macOS command line tools (a C linker and git). Run this and follow macOS's own dialog:",
            "  xcode-select --install"
        ]
    );
    let curl = |proto: &str, url: &str| {
        format!(
            "  curl {proto}--tlsv1.2 -sSf {url} | sh -s -- -y --profile minimal --no-modify-path -t wasm32-wasip2"
        )
    };
    let u = case(&units, "a_default");
    assert_eq!(u.out.lines().next(), Some("To install it later, run:"));
    l(u, &curl("--proto '=https' ", "https://sh.rustup.rs"));
    l(u, "Then `~/.local/bin/watane-dev doctor` checks it again.");
    l(
        case(&units, "a_url"),
        &curl("", "https://example.invalid/rustup-init.sh"),
    );
    l(
        case(&units, "a_file"),
        &curl("", "'file:///tmp/x/rustup-init.sh'"),
    );
    l(case(&units, "a_odd"), &curl("", "'http://x/a b;c'"));
    l(case(&units, "a_tilde"), &curl("", "'~root/x'"));
    l(case(&units, "a_equals"), &curl("", "'=x'"));
    l(case(&units, "target"), "  rustup target add wasm32-wasip2");
    let u = case(&units, "none_b");
    l(
        u,
        "  rustup toolchain install stable --profile minimal -t wasm32-wasip2",
    );
    l(u, "  rustup default stable");
}

/// INST-S-6, 7 and TECHNICAL 3.7 (`detect_toolchain`): the toolchain is found through `rustup`
/// on the PATH or in `~/.cargo/bin`, run from the home with auto-install off, and is complete
/// only when its sysroot has the `wasm32-wasip2` target.
#[test]
fn unit_detect_toolchain_states() {
    let fx = Fx::new();
    fx.add_rustup(true);
    let with = outside(&fx, "sysroot_with");
    fs::create_dir_all(with.join("lib/rustlib/wasm32-wasip2/lib")).expect("target");
    let without = outside(&fx, "sysroot_without");
    fs::create_dir_all(without.join("lib")).expect("lib");
    let sys = |p: &Path| {
        format!(
            "printf '%s\\n' '{}' >\"$FAKE_STATE/rustup_sysroot\"",
            p.display()
        )
    };
    let show = "echo \"state=$TC_STATE rustup=$RUSTUP\"";
    let units = fx.units_in_home(&[
        ("ok", &format!("{}; detect_toolchain; {show}", sys(&with))),
        ("target", &format!("{}; detect_toolchain; {show}", sys(&without))),
        (
            "none_b",
            &format!("{}; : >\"$FAKE_STATE/rustup_no_toolchain\"; detect_toolchain; {show}", sys(&with)),
        ),
        ("none_a", &format!("PATH=$(printf %s \"$PATH\" | tr ':' '\\n' | grep -v fakes | tr '\\n' ':'); detect_toolchain; {show}")),
    ]);
    let u = case(&units, "ok");
    assert!(u.out.starts_with("state=ok rustup="), "{u:?}");
    assert!(case(&units, "target").out.starts_with("state=target "));
    assert!(case(&units, "none_b").out.starts_with("state=none_b "));
    assert!(
        case(&units, "none_a").out.starts_with("state=none_a "),
        "{:?}",
        case(&units, "none_a")
    );
    // rustup ran from the home with auto-install off (3.7)
    let home = fs::canonicalize(&fx.home).expect("home");
    for l in fx.state_read("rustup.pwd").lines() {
        assert_eq!(fs::canonicalize(l).expect("pwd"), home);
    }
    assert!(fx.state_read("rustup.auto").lines().all(|l| l == "0"));
}

/// INST-S-4, 5, 6 and TECHNICAL 3.7 (`toolchain_step` without a terminal): a complete toolchain
/// is reported, `--no-toolchain` skips the offer, and with no terminal to ask on nothing is
/// installed whatever a variable or a flag says; each prints the commands to run later.
#[test]
fn unit_toolchain_step_never_installs_without_a_typed_yes() {
    let fx = Fx::new();
    fx.add_rustup(true);
    let with = outside(&fx, "sysroot_with");
    fs::create_dir_all(with.join("lib/rustlib/wasm32-wasip2/lib")).expect("target");
    let without = outside(&fx, "sysroot_without");
    fs::create_dir_all(without.join("lib")).expect("lib");
    let tty = fx.root.path().join("tty");
    fs::write(&tty, "y\n").expect("tty file");
    let sys = |p: &Path| {
        format!(
            "printf '%s\\n' '{}' >\"$FAKE_STATE/rustup_sysroot\"",
            p.display()
        )
    };
    let step = |sysroot: &Path, no_tc: u8, env: &str| {
        format!(
            "{}; {env} NO_TC={no_tc}; toolchain_step; echo \"TC_OK=$TC_OK\"",
            sys(sysroot)
        )
    };
    let units = fx.units_in_home(&[
        ("complete", &step(&with, 0, "")),
        ("skipped", &step(&without, 1, "")),
        ("no_terminal", &step(&without, 0, "")),
        (
            "no_variable_says_yes",
            &step(
                &without,
                0,
                &format!("WATANE_INSTALL_TTY='{}'; export WATANE_INSTALL_TTY; WATANE_RUSTUP_URL=file:///x/y.sh;", tty.display()),
            ),
        ),
    ]);
    let u = case(&units, "complete");
    assert_eq!(
        u.out, "toolchain: ok (rustup, wasm32-wasip2)\nTC_OK=1\n",
        "{u:?}"
    );
    let u = case(&units, "skipped");
    assert!(
        u.has_out_line("toolchain: skipped (--no-toolchain)"),
        "{u:?}"
    );
    assert!(u.has_out_line("  rustup target add wasm32-wasip2"), "{u:?}");
    assert!(u.out.ends_with("TC_OK=0\n"));
    for n in ["no_terminal", "no_variable_says_yes"] {
        let u = case(&units, n);
        assert!(
            u.has_out_line("no terminal to ask on: the toolchain was not installed"),
            "{n}: {u:?}"
        );
        assert!(
            u.has_out_line("  rustup target add wasm32-wasip2"),
            "{n}: {u:?}"
        );
        assert!(!u.out.contains("[y/N]"), "{n}");
    }
    let log = fx.state_read("rustup.log");
    assert!(
        !log.contains("target add") && !log.contains("toolchain install"),
        "{log}"
    );
    assert_eq!(fx.state_read("installer.log"), "");
}

/// TECHNICAL 3.6 step 0, 3.8 (`recover`): recovery cannot continue after a real filesystem
/// failure: it ends the run with exit 5 and its own text, never the next step. The failure is a
/// shell function that stands in for `mkdir`, `mv` or `rm` and fails on one name.
#[test]
fn unit_recover_stops_on_a_filesystem_failure() {
    let fx = Fx::new();
    let failures = [
        "marker",
        "move",
        "restore",
        "staging",
        "journal",
        "committed",
        "commit_journal",
        "remove",
        "receipt_old",
        "stale",
    ];
    let mut cases = Vec::new();
    for failure in failures {
        let home = home_named(&fx, &format!("rec_{failure}"));
        let bin = home.join(".local/bin/watane");
        let share = home.join(".local/share/watane");
        fs::create_dir_all(home.join(".local/bin")).expect("bin");
        if failure != "stale" {
            write_file(
                &share.join("journal.99999999"),
                &format!(
                    "{} {}\nmoved {}\n",
                    if matches!(failure, "restore" | "remove") {
                        "old"
                    } else {
                        "new"
                    },
                    bin.display(),
                    bin.display(),
                ),
            );
        }
        if !matches!(failure, "committed" | "commit_journal") {
            write_file(&share.join("receipt.new.99999999"), "pending");
        } else {
            let real = fs::canonicalize(&home).expect("physical home");
            let resolved = [
                ".local/bin",
                ".local/share/watane",
                ".local/share/watane-dev",
                "Applications",
            ]
            .map(|folder| format!("resolved {folder} {}\n", real.join(folder).display()))
            .join("");
            write_file(
                &share.join("receipt"),
                &format!(
                    "watane-receipt 1\nversion 0.1.0\ntarget {TARGET}\npath {}\n{resolved}",
                    bin.display()
                ),
            );
        }
        let fail = |tool: &str, pat: &str| {
            format!(
                "{tool}() {{ for a; do case $a in {pat}) return 1 ;; esac; done; command {tool} \"$@\"; }}"
            )
        };
        let fault = match failure {
            "marker" => {
                "mkdir() { case $1 in *.new.99999999) return 1 ;; esac; command mkdir \"$@\"; }"
                    .to_string()
            }
            "move" => {
                write_file(&bin, "new");
                fail("mv", "*/bin/watane.new.99999999")
            }
            "restore" => {
                write_file(&bin, "new");
                write_file(&home.join(".local/bin/watane.old.99999999"), "old");
                fail("mv", "*/bin/watane.old.99999999")
            }
            "staging" | "stale" => {
                write_file(&home.join(".local/bin/watane.new.99999999"), "staged");
                fail("rm", "*/bin/watane.new.99999999")
            }
            "journal" | "commit_journal" => fail("rm", "*/journal.99999999"),
            "remove" => {
                write_file(&bin, "new");
                write_file(&home.join(".local/bin/watane.old.99999999"), "old");
                fail("rm", "*/bin/watane")
            }
            "receipt_old" => {
                write_file(&share.join("receipt.old.99999999"), "old");
                fail("rm", "*/receipt.old.99999999")
            }
            "committed" => {
                write_file(&home.join(".local/bin/watane.old.99999999"), "old");
                fail("rm", "*/bin/watane.old.99999999")
            }
            _ => unreachable!(),
        };
        cases.push((
            failure.to_string(),
            format!(
                "HOME=$ROOT/rec_{failure}; check_home; set_entries; {fault}; recover; printf 'continued after recovery\\n'"
            ),
        ));
    }
    let units = table(&fx, "", &cases);
    for failure in failures {
        let u = case(&units, failure);
        assert_eq!(u.code, 5, "{failure}: {u:?}");
        let message = if failure == "stale" {
            "watane install: could not recover stale install files"
        } else {
            "watane install: could not recover interrupted install (pid 99999999)"
        };
        assert!(u.has_err_line(message), "{failure}: {u:?}");
        assert!(
            u.has_err_line("Recovery stopped; fix the filesystem error and run this again."),
            "{failure}: {u:?}"
        );
        assert!(
            !u.out.contains("continued after recovery"),
            "{failure}: {u:?}"
        );
    }
}

/// TECHNICAL 3.7, 8.4 (`read_answer`): an answer is one line of at most six bytes, and a NUL byte
/// anywhere (which a shell would silently drop) makes it no answer at all, so `y`, `yes` and `delete`
/// are only ever the typed words; an answer that is not UTF-8 is taken as it is.
#[test]
fn unit_read_answer_refuses_a_nul_and_a_long_answer() {
    let fx = Fx::new();
    let inputs: [(&str, &[u8]); 10] = [
        ("y", b"y\n"),
        ("delete", b"delete\n"),
        ("eof", b""),
        ("nul_after", b"y\0\n"),
        ("nul_before", b"\0yes\n"),
        ("nul_inside", b"de\0lete\n"),
        ("trailing_space", b"delete \n"),
        ("not_utf8", b"y\xff\n"),
        ("six_bytes_not_utf8", b"y\xffnope\n"),
        ("seven_bytes", b"yes1234\n"),
    ];
    let mut cases = Vec::new();
    for (name, bytes) in inputs {
        fs::write(fx.root.path().join(format!("in_{name}")), bytes).expect("input");
        cases.push((
            name.to_string(),
            format!("read_answer <\"$ROOT/in_{name}\"; printf '[%s] len=%s\\n' \"$ANSWER\" \"${{#ANSWER}}\""),
        ));
    }
    let units = table(&fx, "", &cases);
    for (name, want) in [
        ("y", "[y] len=1\n"),
        ("delete", "[delete] len=6\n"),
        ("eof", "[] len=0\n"),
        ("nul_after", "[] len=0\n"),
        ("nul_before", "[] len=0\n"),
        ("nul_inside", "[] len=0\n"),
        ("trailing_space", "[] len=0\n"),
        ("seven_bytes", "[] len=0\n"),
    ] {
        let u = case(&units, name);
        assert_eq!((u.code, u.out.as_str()), (0, want), "{name}: {u:?}");
    }
    let u = case(&units, "not_utf8");
    assert_eq!(u.code, 0, "{u:?}");
    assert!(u.out.ends_with("] len=2\n"), "{u:?}");
    let u = case(&units, "six_bytes_not_utf8");
    assert!(
        u.out.ends_with("] len=6\n") && !u.out.starts_with("[y]"),
        "{u:?}"
    );
}

/// TECHNICAL 3.6 step 0, 3.8 (`recover`; S50b-fix ac4): recovery of a journal that names a fresh entry,
/// when that entry and its folder are both gone, finishes and can be repeated: it cannot wedge the
/// next install.
#[test]
fn unit_recover_skips_a_fresh_entry_whose_parent_is_gone() {
    let fx = Fx::new();
    let home = home_named(&fx, "gone");
    let bin = home.join(".local/bin/watane");
    write_file(
        &home.join(".local/share/watane/receipt.new.99999999"),
        "pending",
    );
    write_file(
        &home.join(".local/share/watane/journal.99999999"),
        &format!("new {0}\nmoved {0}\n", bin.display()),
    );
    let units = table(
        &fx,
        "",
        &[(
            "twice".to_string(),
            "HOME=$ROOT/gone; check_home; set_entries; recover; echo first=$?; recover; echo second=$?; ls -A \"$SHAREW\"; test ! -e \"$HOME/.local/bin\" && echo no_bin_folder"
                .to_string(),
        )],
    );
    let u = case(&units, "twice");
    assert_eq!((u.code, u.err.as_str()), (0, ""), "{u:?}");
    assert_eq!(
        u.out, "recovered interrupted watane install\nfirst=0\nsecond=0\nno_bin_folder\n",
        "{u:?}"
    );
}

/// TECHNICAL 8.5, 3.6 step 4 (`valid_entry`, `undo_swap`, S50c ac3): a dead run's journal that
/// names the install folder after `~/.local/bin` was relinked out of the home makes the undo
/// remove, move or restore nothing outside the home, and its cleanup of `.new.<n>` names stays
/// inside. Mutants (each alone): `valid_entry` without `allowed` (case `journal`); in `undo_swap`
/// the line `allowed "$ub_p" || continue` of the cleanup (case `staging`).
#[test]
fn unit_ac3a_undo_swap_through_a_relinked_bin_touches_nothing_outside_the_home() {
    let fx = Fx::new();
    let mut cases: Vec<(String, String)> = Vec::new();
    let mut outs = Vec::new();
    for name in ["journal", "staging"] {
        let home = home_named(&fx, name);
        let share = home.join(".local/share/watane");
        fs::create_dir_all(&share).expect("share");
        let o = outside(&fx, &format!("{name}_o"));
        symlink(&o, home.join(".local/bin")).expect("link");
        write_file(&share.join("receipt.new.99999999"), "pending");
        if name == "journal" {
            let b = home.join(".local/bin/watane");
            write_file(&o.join("watane"), "the person's");
            write_file(&o.join("watane.old.99999999"), "old");
            write_file(
                &share.join("journal.99999999"),
                &format!("old {0}\nnew {0}\nmoved {0}\n", b.display()),
            );
        } else {
            write_file(&o.join("watane.new.99999999"), "the person's");
        }
        outs.push((name, o));
        cases.push((
            name.to_string(),
            format!(
                "HOME=$ROOT/{name}; check_home; set_entries; undo_swap 99999999; echo \"rc=$?\""
            ),
        ));
    }
    let before: Vec<_> = outs.iter().map(|(_, o)| snapshot(o)).collect();
    let units = table(&fx, "", &cases);
    for ((name, o), was) in outs.iter().zip(before) {
        let u = case(&units, name);
        assert!(u.has_out_line("rc=0"), "{name}: {u:?}");
        assert_eq!(snapshot(o), was, "{name}: changed outside the home: {u:?}");
    }
}

/// TECHNICAL 3.4.2, INST-S-35 (`check_foreign`): a dead run's journal makes a path watane's only
/// when it says the path moved and no `<path>.new.<n>` is left; with the staging name still
/// there the path is not proven installed, so a file at it is foreign. Mutant: without
/// `if exists "$cf_p.new.$cf_n"; then continue; fi`.
#[test]
fn unit_ac3b_a_moved_entry_with_its_staging_name_left_is_not_proven_installed() {
    let fx = Fx::new();
    let dead = dead_pid();
    for name in ["staged", "unstaged"] {
        let home = home_named(&fx, name);
        let b = home.join(".local/bin/watane");
        write_file(&b, "a file");
        write_file(
            &home.join(format!(".local/share/watane/journal.{dead}")),
            &format!("new {0}\nmoved {0}\n", b.display()),
        );
        if name == "staged" {
            write_file(
                &home.join(format!(".local/bin/watane.new.{dead}")),
                "staged",
            );
        }
    }
    let cases: Vec<(String, String)> = ["staged", "unstaged"]
        .iter()
        .map(|n| {
            (
                (*n).to_string(),
                format!("HOME=$ROOT/{n}; check_home; set_entries; RELEASE=0.1.0; UPGRADED=0; check_foreign"),
            )
        })
        .collect();
    let units = table(&fx, "", &cases);
    let u = case(&units, "staged");
    assert_eq!(u.code, 8, "{u:?}");
    foreign_lines(u, &["~/.local/bin/watane"]);
    let u = case(&units, "unstaged");
    assert_eq!((u.code, u.err.as_str()), (0, ""), "{u:?}");
}

/// TECHNICAL 3.6.1: only another live process prevents stale-file recovery.
#[test]
fn unit_pid_gone_distinguishes_self_live_and_dead() {
    let fx = Fx::new();
    let units = fx.units(
        r#"kill() { [ "$*" = "-0 42" ]; }"#,
        &[],
        &[
            ("self", "pid_gone \"$$\""),
            ("live", "pid_gone 42"),
            ("dead", "pid_gone 43"),
        ],
    );
    for (name, expected) in [("self", 0), ("live", 1), ("dead", 0)] {
        assert_eq!(case(&units, name).code, expected, "{name}");
    }
}

/// TECHNICAL 3.4.2: a live PID's journal is skipped, even when it would otherwise prove the entry.
#[test]
fn ac5_check_foreign_uses_only_a_dead_runs_journal() {
    let fx = Fx::new();
    for (name, pid) in [("live", 99_999_998), ("dead", 99_999_999)] {
        let home = home_named(&fx, name);
        let bin = home.join(".local/bin/watane");
        write_file(&bin, "an existing program");
        write_file(
            &home.join(format!(".local/share/watane/journal.{pid}")),
            &format!("new {0}\nmoved {0}\n", bin.display()),
        );
    }
    let cases = ["live", "dead"].map(|name| {
        (
            name.to_string(),
            format!("HOME=$ROOT/{name}; check_home; set_entries; check_foreign"),
        )
    });
    let units = table(&fx, r#"kill() { [ "$*" = "-0 99999998" ]; }"#, &cases);
    let live = case(&units, "live");
    assert_eq!(live.code, 8, "{live:?}");
    foreign_lines(live, &["~/.local/bin/watane"]);
    let dead = case(&units, "dead");
    assert_eq!((dead.code, dead.err.as_str()), (0, ""), "{dead:?}");
}

/// TECHNICAL 3.5, 8.4 (`read_receipt`): a receipt that is a link is not read, whatever it leads
/// to. Mutant: `read_receipt` without `[ ! -L "$1" ]`.
#[test]
fn unit_ac3c_a_receipt_that_is_a_link_is_not_read() {
    let fx = Fx::new();
    let real = fx.root.path().join("real_receipt");
    write_file(
        &real,
        "watane-receipt 1\nversion 0.1.0\npath /x\nresolved bin /y\n",
    );
    symlink(&real, fx.root.path().join("link_receipt")).expect("link");
    let show = r#"; echo "ok=$R_OK ver=$R_VERSION lines=$R_LINES resolved=$R_RESOLVED""#;
    let units = fx.units(
        "",
        &[],
        &[
            (
                "file",
                &format!("read_receipt \"$ROOT/real_receipt\"{show}"),
            ),
            (
                "link",
                &format!("read_receipt \"$ROOT/link_receipt\"{show}"),
            ),
        ],
    );
    assert_eq!(
        case(&units, "file").out,
        "ok=1 ver=0.1.0 lines=path /x resolved=resolved bin /y\n"
    );
    assert_eq!(case(&units, "link").out, "ok=0 ver= lines= resolved=\n");
}

/// TECHNICAL 3.8, 3.6 step 0 (`clean_stale`, `recover`): the names of a live run are not cleaned
/// or recovered, and nothing is removed through a folder that leads out of the home. Mutants
/// (each alone): `clean_stale` without `pid_gone "$cs_n" || continue` (case `stale_live`), or
/// without `allowed "$cs_e" || continue` (case `stale_link`); `recover` without
/// `pid_gone "$rc_n" || continue` (case `recover_live`).
#[test]
fn unit_ac3d_stale_cleanup_and_recovery_leave_a_live_run_and_a_link_alone() {
    let fx = Fx::new();
    let mut live = std::process::Command::new(fx.real.join("sleep"))
        .arg("600")
        .spawn()
        .expect("sleep");
    let (live_pid, dead) = (live.id(), dead_pid());
    let run = |name: &str, call: &str| {
        (
            name.to_string(),
            format!("HOME=$ROOT/{name}; check_home; set_entries; {call}; echo \"rc=$?\""),
        )
    };
    let sl = home_named(&fx, "stale_live");
    write_file(
        &sl.join(format!(".local/bin/watane.new.{live_pid}")),
        "live",
    );
    write_file(&sl.join(format!(".local/bin/watane.old.{dead}")), "dead");
    write_file(&sl.join(".local/share/watane/receipt"), "x");
    let lk = home_named(&fx, "stale_link");
    fs::create_dir_all(lk.join(".local")).expect("local");
    let o = outside(&fx, "stale_link_o");
    write_file(&o.join(format!("watane.new.{dead}")), "the person's");
    symlink(&o, lk.join(".local/bin")).expect("link");
    let rl = home_named(&fx, "recover_live");
    let share = rl.join(".local/share/watane");
    write_file(&share.join(format!("journal.{live_pid}")), "old x\n");
    write_file(&share.join(format!("receipt.new.{live_pid}")), "pending");
    let cases = vec![
        run("stale_live", "clean_stale"),
        run("stale_link", "clean_stale"),
        run("recover_live", "recover"),
    ];
    let units = table(&fx, "", &cases);
    live.kill().expect("kill");
    live.wait().expect("wait");
    for n in ["stale_live", "stale_link", "recover_live"] {
        let u = case(&units, n);
        assert!(u.has_out_line("rc=0"), "{n}: {u:?}");
    }
    assert!(
        sl.join(format!(".local/bin/watane.new.{live_pid}"))
            .exists()
    );
    assert!(!sl.join(format!(".local/bin/watane.old.{dead}")).exists());
    assert!(o.join(format!("watane.new.{dead}")).exists());
    assert!(share.join(format!("journal.{live_pid}")).exists());
    assert!(share.join(format!("receipt.new.{live_pid}")).exists());
}

/// TECHNICAL 3.4 step 4 (`check_home`): HOME is a plain absolute folder that does not resolve to
/// `/`; each refusal ends with exit 2 and its own text. Mutants (each alone): without
/// `plain_path` (cases `dots`, `relative`); with the test of `/` gone and a HOME of `/` accepted
/// (case `root`: `if false; then` in place of the `plain_path` line), or without the
/// `[ "$HOME_R" != / ]` line (case `link_to_root`).
#[test]
fn unit_ac3e_check_home_refuses_what_is_not_a_plain_home() {
    let fx = Fx::new();
    fs::create_dir(fx.root.path().join("a")).expect("a");
    symlink("/", fx.root.path().join("to_root")).expect("link");
    let hint = "Set HOME to your home folder, then run this again.\n";
    let values = [
        ("root", "/", "HOME is not a plain absolute folder: /"),
        (
            "dots",
            "$ROOT/a/..",
            "HOME is not a plain absolute folder: $ROOT/a/..",
        ),
        ("relative", "a", "HOME is not a plain absolute folder: a"),
        ("empty", "", "HOME is not a plain absolute folder: "),
        ("missing", "$ROOT/none", "HOME is not a folder: $ROOT/none"),
        (
            "link_to_root",
            "$ROOT/to_root",
            "HOME resolves to /: $ROOT/to_root",
        ),
    ];
    let cases: Vec<(String, String)> = values
        .iter()
        .map(|(n, v, _)| ((*n).to_string(), format!("HOME=\"{v}\"; check_home")))
        .collect();
    let mut cases = cases;
    cases.push((
        "plain".into(),
        "HOME=$ROOT/a; check_home; echo \"HOME_R=$HOME_R\"".into(),
    ));
    let units = table(&fx, "", &cases);
    let root = fx.root.path().display().to_string();
    for (n, _, msg) in values {
        let u = case(&units, n);
        assert_eq!(u.code, 2, "{n}: {u:?}");
        assert_eq!(
            u.err,
            format!("watane install: {}\n{hint}", msg.replace("$ROOT", &root)),
            "{n}"
        );
    }
    let u = case(&units, "plain");
    let a = fs::canonicalize(fx.root.path().join("a")).expect("canonical");
    assert_eq!(u.out, format!("HOME_R={}\n", a.display()));
}

/// TECHNICAL 3.4 step 1, 8.4 (`do_uninstall`): an uninstall stops at the journal of a live run
/// with exit 5 and removes nothing, and finishes what a dead run left (its journal is gone).
/// Mutants (each alone): without `check_no_live_install` (case `live`), or without `recover`
/// (case `dead`).
#[test]
fn unit_ac3f_uninstall_checks_for_a_live_run_and_recovers_a_dead_one() {
    let fx = Fx::new();
    let mut live = std::process::Command::new(fx.real.join("sleep"))
        .arg("600")
        .spawn()
        .expect("sleep");
    let (live_pid, dead) = (live.id(), dead_pid());
    let lv = fx.installed_home("live");
    write_file(
        &lv.join(format!(".local/share/watane/journal.{live_pid}")),
        "old x\n",
    );
    let dd = fx.installed_home("dead");
    write_file(
        &dd.join(format!(".local/share/watane/journal.{dead}")),
        "old x\n",
    );
    let cases: Vec<(String, String)> = ["live", "dead"]
        .iter()
        .map(|n| {
            (
                (*n).to_string(),
                format!("HOME=$ROOT/{n}; check_home; set_entries; do_uninstall"),
            )
        })
        .collect();
    let units = table(&fx, "", &cases);
    live.kill().expect("kill");
    live.wait().expect("wait");
    let u = case(&units, "live");
    assert_eq!(u.code, 5, "{u:?}");
    assert_eq!(
        u.first_err(),
        format!("watane install: another install is running (pid {live_pid})")
    );
    assert!(lv.join(".local/share/watane/receipt").is_file(), "{u:?}");
    assert!(lv.join(".local/bin/watane").exists(), "{u:?}");
    let u = case(&units, "dead");
    assert_eq!(u.code, 0, "{u:?}");
    assert!(u.has_out_line("uninstalled watane 0.1.0"), "{u:?}");
    assert!(
        !dd.join(format!(".local/share/watane/journal.{dead}"))
            .exists(),
        "{u:?}"
    );
}

/// TECHNICAL 3.2, 3.7 (`download`): the secure mode (the real release, no `WATANE_RELEASE_BASE`)
/// asks `curl` for https only, TLS 1.2 or later, and a failure on an HTTP error; the plain mode
/// (a test or mirror base) asks for none of the first two. A shell function stands in for `curl`
/// and records its arguments. Mutants: `curl` without `--proto '=https'`, without `--tlsv1.2`,
/// or without `-f`, in the secure line.
#[test]
fn unit_ac3g_the_secure_download_asks_curl_for_https_tls_and_failure_on_error() {
    let fx = Fx::new();
    let fake = r#"TMP=$ROOT; curl() { printf '%s\n' "$*" >"$ROOT/curl.args"; }"#;
    let units = fx.units(
        "",
        &[],
        &[
            (
                "secure",
                &format!(
                    "{fake}; download https://h/x \"$ROOT/out\" secure; cat \"$ROOT/curl.args\""
                ),
            ),
            (
                "plain",
                &format!(
                    "{fake}; download http://h/x \"$ROOT/out\" plain; cat \"$ROOT/curl.args\""
                ),
            ),
        ],
    );
    let root = fx.root.path().display();
    assert_eq!(
        case(&units, "secure").out,
        format!(
            "-fsSL --proto =https --tlsv1.2 --retry 2 --connect-timeout 15 -o {root}/out https://h/x\n"
        )
    );
    assert_eq!(
        case(&units, "plain").out,
        format!("-fsSL --retry 2 --connect-timeout 15 -o {root}/out http://h/x\n")
    );
}

/// TECHNICAL 3.6 step 4 (`undo_swap`): the undo takes back only what the journal says moved; an
/// entry named `new` but not `moved` (the swap stopped before it) is left as it is. Mutant: in
/// `undo_swap` the line `in_list "$J_MOVED" "$ub_p" || continue` of the `J_NEW` loop.
#[test]
fn unit_undo_swap_leaves_an_entry_that_did_not_move() {
    let fx = Fx::new();
    let home = home_named(&fx, "h");
    let b = home.join(".local/bin/watane");
    write_file(&b, "kept");
    let share = home.join(".local/share/watane");
    write_file(&share.join("receipt.new.99999999"), "pending");
    write_file(
        &share.join("journal.99999999"),
        &format!("new {}\n", b.display()),
    );
    let units = fx.units(
        "",
        &[],
        &[(
            "undo",
            "HOME=$ROOT/h; check_home; set_entries; undo_swap 99999999; echo \"rc=$?\"",
        )],
    );
    let u = case(&units, "undo");
    assert_eq!(u.out, "rc=0\n", "{u:?}");
    assert_eq!(fs::read_to_string(&b).expect("kept"), "kept", "{u:?}");
}

/// TECHNICAL 12.1: each case runs under `set -eu`, as the script does. A command that fails ends
/// the case with its status and the output it had, an unset variable ends it too, and the next
/// case starts clean.
#[test]
fn unit_cases_run_under_set_eu() {
    let fx = Fx::new();
    let units = fx.units(
        "",
        &[],
        &[
            ("stops", "echo before; false; echo after"),
            ("unset", "echo \"$NO_SUCH_VARIABLE\"; echo after"),
            ("next", "echo next"),
        ],
    );
    let u = case(&units, "stops");
    assert_eq!((u.code, u.out.as_str()), (1, "before\n"), "{u:?}");
    let u = case(&units, "unset");
    assert!(u.code != 0 && u.out.is_empty(), "{u:?}");
    let u = case(&units, "next");
    assert_eq!((u.code, u.out.as_str()), (0, "next\n"), "{u:?}");
}
