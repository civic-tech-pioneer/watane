//! Regressions found by the S135 review (PR #49, r1), each red before its fix.

use std::fs;
use std::os::unix::fs::symlink;

use crate::support::unit::*;
use crate::support::*;

/// TECHNICAL 3.6 step 3, 3.9 (S135 r1 P1.1): `~/.local/bin` relinked to the home itself after the
/// check says "your home folder itself", as `check_folders` does, never a bare `~`.
#[test]
fn regress_s135_changed_during_install_to_the_home_has_no_bare_tilde() {
    let fx = Fx::new();
    let home = fx.root.path().join("h");
    fs::create_dir_all(home.join(".local/bin")).expect("bin");
    fs::create_dir_all(home.join(".local/share/watane")).expect("share");
    symlink(&home, home.join("binlink")).expect("link");
    let units = fx.units(
        "",
        &[],
        &[(
            "relinked",
            r#"HOME=$ROOT/h; check_home; set_entries; check_folders
rmdir "$HOME/.local/bin" && mv "$HOME/binlink" "$HOME/.local/bin" || exit 99
if swap_entries; then rc=0; else rc=$?; fi; echo "rc=$rc"; printf 'changed<%s>\n' "$SWAP_CHANGED""#,
        )],
    );
    let u = case(&units, "relinked");
    assert!(u.has_out_line("rc=1"), "{u:?}");
    assert!(
        u.has_out_line(
            "changed<~/.local/bin changed during the install: it now resolves to your home folder itself>"
        ),
        "{u:?}"
    );
}

/// Shell helpers of the cases below. `rcpt <file> <path>...` writes a recognised receipt that
/// names the paths and records the four install folders, `~/.local/bin` as `$BINRES` when set.
const PRELUDE: &str = r#"
check_home; set_entries
rcpt() {
  rf=$1; shift
  {
    printf 'watane-receipt 1\nversion 0.1.0\ntarget t\n'
    for rp in "$@"; do printf 'path %s\n' "$rp"; done
    printf 'resolved .local/bin %s\n' "${BINRES:-$HOME_R/.local/bin}"
    printf 'resolved .local/share/watane %s/.local/share/watane\n' "$HOME_R"
    printf 'resolved .local/share/watane-dev %s/.local/share/watane-dev\n' "$HOME_R"
    printf 'resolved Applications %s/Applications\n' "$HOME_R"
  } >"$rf"
}
mkdir -p "$SHAREW" "$HOME/.local/bin"
pid=99999999
bin=$HOME/.local/bin/watane
node=$HOME/.local/bin/watane-node
dev=$HOME/.local/bin/watane-dev
fresh_journal() { printf 'new %s\nmoved %s\n' "$1" "$1" >"$SHAREW/journal.$pid"; }
kept() { if [ -e "$1" ]; then echo kept; else echo gone; fi; }
"#;

/// Each case in a world of its own: the cases plant files, and none may see another's.
fn run(cases: &[(&str, &str)]) -> Vec<Unit> {
    cases
        .iter()
        .flat_map(|one| Fx::new().units(PRELUDE, &[], &[*one]))
        .collect()
}

/// TECHNICAL 3.6 step 4 (S135 r1 P1.2, mutant 9 of `undo_swap`): with a readable staged receipt, a
/// `new` entry is taken out only when the staged receipt names it, whatever the published one says.
/// Mutant: in the `new` loop of `undo_swap` delete the `if [ "$ub_new_ok" = 1 ]` branch's
/// `in_list "$ub_new_paths"` line. Gain: a planted journal makes the undo remove a file that the
/// run never installed.
#[test]
fn regress_s135_undo_takes_a_new_entry_out_only_if_the_staged_receipt_names_it() {
    let units = run(&[
        (
            "staged_names_it",
            r#"rcpt "$RECEIPT" "$node"; rcpt "$RECEIPT.new.$pid" "$node"
: >"$node"; fresh_journal "$node"; undo_swap "$pid"; kept "$node""#,
        ),
        (
            "staged_omits_it",
            r#"rcpt "$RECEIPT" "$node"; rcpt "$RECEIPT.new.$pid" "$dev"
: >"$node"; fresh_journal "$node"; undo_swap "$pid"; kept "$node""#,
        ),
    ]);
    assert!(case(&units, "staged_names_it").has_out_line("gone"));
    assert!(case(&units, "staged_omits_it").has_out_line("kept"));
}

/// TECHNICAL 3.6 step 4 (S135 r1 P1.2, mutant 10): when only the published receipt is readable,
/// its paths gate the `new` entries too. Mutant: delete the `elif [ "$ub_old_ok" = 1 ]` branch
/// (`in_list "$ub_old_paths"`) of that loop.
#[test]
fn regress_s135_undo_with_an_unreadable_staged_receipt_uses_the_published_paths() {
    let units = run(&[
        (
            "published_names_it",
            r#"rcpt "$RECEIPT" "$node"; echo junk >"$RECEIPT.new.$pid"
: >"$node"; fresh_journal "$node"; undo_swap "$pid"; kept "$node""#,
        ),
        (
            "published_omits_it",
            r#"rcpt "$RECEIPT" "$dev"; echo junk >"$RECEIPT.new.$pid"
: >"$node"; fresh_journal "$node"; undo_swap "$pid"; kept "$node""#,
        ),
    ]);
    assert!(case(&units, "published_names_it").has_out_line("gone"));
    assert!(case(&units, "published_omits_it").has_out_line("kept"));
}

/// TECHNICAL 3.6 step 4, 3.8 (S135 r1 P1.3): the staging names of an undone entry go whether or not
/// the staged receipt records the same folder as the published one; `recover` leaves none.
#[test]
fn regress_s135_recover_removes_a_staging_name_where_the_receipts_disagree() {
    let units = run(&[(
        "disagree",
        r#"rcpt "$RECEIPT" "$bin"; BINRES=/elsewhere rcpt "$RECEIPT.new.$pid" "$bin"
printf 'old %s\n' "$dev" >"$SHAREW/journal.$pid"
: >"$node.new.$pid"; : >"$dev.new.$pid"
recover
kept "$node.new.$pid"; kept "$dev.new.$pid"; kept "$SHAREW/journal.$pid""#,
    )]);
    let u = case(&units, "disagree");
    assert_eq!(u.out.matches("gone").count(), 3, "{u:?}");
}

/// TECHNICAL 3.4.2 (S135 r1 P1.4, mutant 15 of `check_foreign`): with a readable receipt, a path
/// that a dead run's journal moved in counts as watane's only when a receipt names it.
/// Mutant: delete the `if [ "$cf_ownership" = 1 ]` line in `check_foreign`. No `read_receipt` stub.
#[test]
fn regress_s135_foreign_check_needs_a_receipt_that_names_the_moved_path() {
    let units = run(&[
        (
            "named",
            r#"RELEASE=0.1.0; rcpt "$RECEIPT" "$bin"; rcpt "$RECEIPT.new.$pid" "$bin" "$node"
: >"$node"; fresh_journal "$node"; check_foreign; echo "rc=$?""#,
        ),
        (
            "unnamed",
            r#"RELEASE=0.1.0; rcpt "$RECEIPT" "$bin"; rcpt "$RECEIPT.new.$pid" "$bin"
: >"$node"; fresh_journal "$node"; check_foreign"#,
        ),
    ]);
    let named = case(&units, "named");
    assert!(named.has_out_line("rc=0"), "{named:?}");
    let unnamed = case(&units, "unnamed");
    assert_eq!(unnamed.code, 8, "{unnamed:?}");
    assert!(
        unnamed.has_err_line("  ~/.local/bin/watane-node"),
        "{unnamed:?}"
    );
}

/// TECHNICAL 3.4.2, 3.6 step 4 (S135 r1 P1.4, mutant 14): the staged receipt's folder record can
/// veto a moved path that the published record allows. Mutant: in `check_foreign` delete the
/// staged half of the `recovery_allowed_both` call (pass `0` for the staged flag).
#[test]
fn regress_s135_foreign_check_obeys_the_staged_receipts_folder_record() {
    let units = run(&[
        (
            "same_folder",
            r#"RELEASE=0.1.0; rcpt "$RECEIPT" "$bin"; rcpt "$RECEIPT.new.$pid" "$bin" "$node"
: >"$node"; fresh_journal "$node"; check_foreign; echo "rc=$?""#,
        ),
        (
            "staged_elsewhere",
            r#"RELEASE=0.1.0; rcpt "$RECEIPT" "$bin"; BINRES=/elsewhere rcpt "$RECEIPT.new.$pid" "$bin" "$node"
: >"$node"; fresh_journal "$node"; check_foreign"#,
        ),
    ]);
    assert!(case(&units, "same_folder").has_out_line("rc=0"));
    let vetoed = case(&units, "staged_elsewhere");
    assert_eq!(vetoed.code, 8, "{vetoed:?}");
    assert!(
        vetoed.has_err_line("  ~/.local/bin/watane-node"),
        "{vetoed:?}"
    );
}

/// TECHNICAL 8.1 (S135 r1 P1.5, mutants 32-38 of `do_status`): each filter that lets a moved path
/// of a dead pre-commit run show as `leftover:`, one case each, and a path the receipt already
/// listed shows once.
#[test]
fn regress_s135_status_lists_a_moved_path_once_and_only_when_every_filter_holds() {
    // the baseline: the published receipt names `watane`, the staged one `watane` and the node;
    // the journal moved the node in, as a new entry; the node is in place
    let base = r#"rcpt "$RECEIPT" "$bin"; rcpt "$RECEIPT.new.$pid" "$bin" "$node"
: >"$node"; fresh_journal "$node""#;
    let case_of = |name: &'static str, change: &str| -> (String, String) {
        (name.to_string(), format!("{base}\n{change}\ndo_status"))
    };
    let cases = [
        case_of("baseline", ":"),
        // the staged receipt does not name it
        case_of("not_owned", r#"rcpt "$RECEIPT.new.$pid" "$bin""#),
        // the published receipt records ~/.local/bin elsewhere
        case_of(
            "published_elsewhere",
            r#"BINRES=/elsewhere rcpt "$RECEIPT" "$bin""#,
        ),
        // the staged receipt records ~/.local/bin elsewhere
        case_of(
            "staged_elsewhere",
            r#"BINRES=/elsewhere rcpt "$RECEIPT.new.$pid" "$bin" "$node""#,
        ),
        // the swap never moved it in: its staging name is still there
        case_of("still_staged", r#": >"$node.new.$pid""#),
        // nothing is in place at the path
        case_of("absent", r#"rm -f "$node""#),
        // an old entry with no backup: the swap did not reach it
        case_of(
            "old_no_backup",
            r#"printf 'old %s\nmoved %s\n' "$node" "$node" >"$SHAREW/journal.$pid""#,
        ),
        // an old entry with its backup: it was swapped
        case_of(
            "old_with_backup",
            r#"printf 'old %s\nmoved %s\n' "$node" "$node" >"$SHAREW/journal.$pid"; : >"$node.old.$pid""#,
        ),
        // the published receipt lists it as well
        case_of("also_installed", r#"rcpt "$RECEIPT" "$bin" "$node""#),
    ];
    let refs: Vec<(&str, &str)> = cases
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_str()))
        .collect();
    let units = run(&refs);
    let leftover = "leftover: ~/.local/bin/watane-node";
    let count = |u: &Unit| u.out.lines().filter(|l| *l == leftover).count();
    assert_eq!(
        count(case(&units, "baseline")),
        1,
        "{:?}",
        case(&units, "baseline")
    );
    for name in [
        "not_owned",
        "published_elsewhere",
        "staged_elsewhere",
        "still_staged",
        "absent",
        "old_no_backup",
    ] {
        assert_eq!(
            count(case(&units, name)),
            0,
            "{name}: {:?}",
            case(&units, name)
        );
    }
    assert_eq!(count(case(&units, "old_with_backup")), 1);
    let both = case(&units, "also_installed");
    assert_eq!(count(both), 0, "{both:?}");
    assert_eq!(
        both.out
            .lines()
            .filter(|l| *l == "~/.local/bin/watane-node")
            .count(),
        1,
        "{both:?}"
    );
}

/// TECHNICAL 8.1 (S135 r1 P1.13): status that reports a lock or leftovers ends with one line that
/// names the command to run; a clean status has none.
#[test]
fn regress_s135_status_ends_with_the_next_step_it_reports() {
    let base = r#"rcpt "$RECEIPT" "$bin""#;
    let lock = r#"mkdir "$HOME/.watane-install.lock""#;
    let left = r#": >"$bin.old.$pid""#;
    let cases = [
        ("clean", format!("{base}\ndo_status")),
        ("lock", format!("{base}\n{lock}\ndo_status")),
        ("left", format!("{base}\n{left}\ndo_status")),
        ("both", format!("{base}\n{lock}\n{left}\ndo_status")),
    ];
    let refs: Vec<(&str, &str)> = cases.iter().map(|(n, b)| (*n, b.as_str())).collect();
    let units = run(&refs);
    let last = |name: &str| {
        case(&units, name)
            .out
            .lines()
            .last()
            .unwrap_or("")
            .to_string()
    };
    let next = |name: &str| {
        case(&units, name)
            .out
            .lines()
            .filter(|l| l.starts_with("Next:"))
            .count()
    };
    assert_eq!(next("clean"), 0);
    assert_eq!(
        last("lock"),
        "Next: if no install is running, run: rm -rf ~/.watane-install.lock"
    );
    assert_eq!(
        last("left"),
        "Next: run: sh ~/.local/share/watane/install.sh (it recovers the interrupted install first)"
    );
    assert_eq!(
        last("both"),
        "Next: if no install is running, run: rm -rf ~/.watane-install.lock, then run: sh ~/.local/share/watane/install.sh"
    );
    for name in ["lock", "left", "both"] {
        assert_eq!(next(name), 1, "{name}");
    }
}

/// TECHNICAL 8.4 step 1 (S135 r1 P1.6): the empty Watane folders are pruned by one routine, only
/// where the recorded location says and only when empty.
#[test]
fn regress_s135_prune_removes_only_empty_watane_folders_where_recorded() {
    let units = run(&[
        (
            "recorded",
            r#"mkdir -p "$HOME/.local/share/watane-dev"
rcpt "$RECEIPT" "$bin"; read_receipt "$RECEIPT"; rm "$RECEIPT"
prune_watane_dirs "$R_RESOLVED"; kept "$SHAREW"; kept "$HOME/.local/share/watane-dev""#,
        ),
        (
            "elsewhere",
            r#"mkdir -p "$HOME/.local/share/watane-dev"
rcpt "$RECEIPT" "$bin"; read_receipt "$RECEIPT"; rm "$RECEIPT"
prune_watane_dirs "resolved .local/share/watane /elsewhere"; kept "$SHAREW""#,
        ),
        (
            "not_empty",
            r#"rcpt "$RECEIPT" "$bin"; read_receipt "$RECEIPT"
prune_watane_dirs "$R_RESOLVED"; kept "$SHAREW""#,
        ),
    ]);
    assert_eq!(case(&units, "recorded").out.matches("gone").count(), 2);
    assert!(
        case(&units, "elsewhere").has_out_line("kept"),
        "{:?}",
        case(&units, "elsewhere")
    );
    assert!(
        case(&units, "not_empty").has_out_line("kept"),
        "{:?}",
        case(&units, "not_empty")
    );
}

/// TECHNICAL 3.6 step 0 (S135 r1 P1.7): `recovered interrupted watane install` prints when only
/// stale names were removed, and not while a journal stays unretired.
/// Mutants: delete `RECOVERED=1` after `rm_path "$cs_f"` in `clean_stale`; delete
/// `&& [ "$rc_pending" = 0 ]` in `recover`.
#[test]
fn regress_s135_recovered_line_for_stale_names_and_not_for_a_skipped_journal() {
    let units = run(&[
        (
            "stale_only",
            r#"rcpt "$RECEIPT" "$bin"; : >"$bin.new.$pid"; recover; kept "$bin.new.$pid""#,
        ),
        ("nothing", r#"rcpt "$RECEIPT" "$bin"; recover"#),
        (
            // the journal of 99999999 names ~/.local/bin, which the receipt records elsewhere, so it is
            // skipped; the stale name of another dead pid in another folder is still removed
            "skipped_journal",
            r#"BINRES=/elsewhere rcpt "$RECEIPT" "$bin"; rcpt "$RECEIPT.new.$pid" "$bin" "$node"
: >"$node"; fresh_journal "$node"; : >"$SHAREW/VERSION.new.99999998"
recover; kept "$SHAREW/VERSION.new.99999998"; kept "$SHAREW/journal.$pid""#,
        ),
    ]);
    let line = "recovered interrupted watane install";
    let stale = case(&units, "stale_only");
    assert!(
        stale.has_out_line(line) && stale.has_out_line("gone"),
        "{stale:?}"
    );
    assert!(!case(&units, "nothing").has_out_line(line));
    let skipped = case(&units, "skipped_journal");
    assert!(!skipped.has_out_line(line), "{skipped:?}");
    assert!(
        skipped.has_out_line("gone") && skipped.has_out_line("kept"),
        "the stale name went and the journal stayed: {skipped:?}"
    );
}

/// TECHNICAL 3.6.2 (S135 r1 P1.8): the lock message shows a pid only for a lock that is a real
/// folder holding a regular `pid` file; a link, a file, or a linked `pid` shows none.
/// Mutant: delete `[ ! -L "$1" ] &&` in `lock_pid_suffix`.
#[test]
fn regress_s135_lock_shows_a_pid_only_for_a_real_folder() {
    let units = run(&[
        (
            "folder",
            r#"mkdir "$HOME/l"; echo 4242 >"$HOME/l/pid"; printf '<%s>\n' "$(lock_pid_suffix "$HOME/l")""#,
        ),
        (
            "file",
            r#"echo 4242 >"$HOME/l"; printf '<%s>\n' "$(lock_pid_suffix "$HOME/l")""#,
        ),
    ]);
    assert!(case(&units, "folder").has_out_line("< (pid 4242)>"));
    // a link to a folder that holds a pid, and a folder whose pid is a link: made by the test
    let fx = Fx::new();
    fs::create_dir(fx.home.join("real")).expect("real");
    fs::write(fx.home.join("real/pid"), "4242\n").expect("pid");
    symlink("real", fx.home.join("link")).expect("link");
    fs::create_dir(fx.home.join("lock")).expect("lock");
    symlink("../real/pid", fx.home.join("lock/pid")).expect("pid link");
    let linked = fx.units(
        PRELUDE,
        &[],
        &[
            (
                "link",
                r#"printf '<%s>\n' "$(lock_pid_suffix "$HOME/link")""#,
            ),
            (
                "linked_pid",
                r#"printf '<%s>\n' "$(lock_pid_suffix "$HOME/lock")""#,
            ),
        ],
    );
    let units: Vec<Unit> = units.into_iter().chain(linked).collect();
    for name in ["file", "link", "linked_pid"] {
        assert!(
            case(&units, name).has_out_line("<>"),
            "{name}: {:?}",
            case(&units, name)
        );
    }
}

/// TECHNICAL 3.6.2, 3.4.1 (S135 r1 P1.9): a lock left by a dead run stops the install with exit 5
/// and the lock message before any folder check, so `~/.local` linked out of the home does not
/// turn it into exit 2.
#[test]
fn regress_s135_a_stale_lock_is_reported_before_a_linked_local() {
    let fx = Fx::new();
    let outside = fx.root.path().join("outside");
    fs::create_dir(&outside).expect("outside");
    symlink(&outside, fx.home.join(".local")).expect("link");
    fs::create_dir(fx.home.join(".watane-install.lock")).expect("lock");
    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 5, "{out:?}");
    assert_eq!(
        out.first_err(),
        "watane install: another install may be running",
        "{out:?}"
    );
}

/// TECHNICAL 3.6 step 4 (S135 r1 P1.11): a published receipt that is not recognised names no `old`
/// path, so the undo puts none back, even with a readable staged receipt; it takes out a `new`
/// entry only if the staged receipt names it. Such a receipt is a corrupted one (8.5 rule 4).
#[test]
fn regress_s135_undo_with_an_unrecognised_published_receipt_restores_no_old_entry() {
    let units = run(&[(
        "unrecognised",
        r#"echo junk >"$RECEIPT"; rcpt "$RECEIPT.new.$pid" "$bin" "$node"
echo new >"$bin"; echo old >"$bin.old.$pid"; : >"$node"
printf 'old %s\nmoved %s\nnew %s\nmoved %s\n' "$bin" "$bin" "$node" "$node" >"$SHAREW/journal.$pid"
undo_swap "$pid"
cat "$bin"; kept "$node"; kept "$SHAREW/journal.$pid""#,
    )]);
    let u = case(&units, "unrecognised");
    assert_eq!(u.out, "new\ngone\ngone\n", "{u:?}");
}

/// TECHNICAL 3.6 step 4 (validator, S137-1): in the `new` loop of the undo, the staged receipt's
/// folder record vetoes a move that the published record allows: the entry stays and so does the
/// journal. Mutant: pass `0` for the staged flag of that `recovery_allowed_both` call.
#[test]
fn regress_s135_undo_obeys_the_staged_receipts_folder_record() {
    let units = run(&[(
        "staged_elsewhere",
        r#"rcpt "$RECEIPT" "$bin"; BINRES=/elsewhere rcpt "$RECEIPT.new.$pid" "$bin" "$node"
: >"$node"; fresh_journal "$node"; undo_swap "$pid"; kept "$node"; kept "$SHAREW/journal.$pid""#,
    )]);
    let u = case(&units, "staged_elsewhere");
    assert_eq!(u.out.matches("kept").count(), 2, "{u:?}");
}

/// TECHNICAL 8.4 step 1, 8.5 (validator, S137-1): the prune skips a Watane folder that resolves
/// outside the home whatever the locations say, so an empty folder there is never removed. The
/// locations name what an earlier `allowed` left in `al_rr`, which a missing veto would match.
/// Mutant: drop `|| continue` after `allowed` in `prune_watane_dirs`.
#[test]
fn regress_s135_prune_leaves_a_folder_outside_the_home() {
    let fx = Fx::new();
    let out = fx.root.path().join("out");
    fs::create_dir_all(out.join("watane")).expect("out/watane");
    fs::create_dir_all(out.join("watane-dev")).expect("out/watane-dev");
    fs::create_dir_all(fx.home.join(".local")).expect("local");
    symlink(&out, fx.home.join(".local/share")).expect("share link");
    let units = fx.units(
        PRELUDE,
        &[],
        &[(
            "share_linked_out",
            r#"allowed "$bin"
prune_watane_dirs "resolved .local/share/watane $al_rr
resolved .local/share/watane-dev $al_rr"
kept "$ROOT/out/watane"; kept "$ROOT/out/watane-dev""#,
        )],
    );
    let u = case(&units, "share_linked_out");
    assert_eq!(u.out.matches("kept").count(), 2, "{u:?}");
}

/// TECHNICAL 8.1 (validator, S137-1): a journal reported alone (a dead commit whose backups are
/// gone) still ends status with the next step. Mutant: drop `ds_has_left=1` after the journal line.
#[test]
fn regress_s135_status_next_step_for_a_journal_alone() {
    let units = run(&[(
        "journal_only",
        r#"rcpt "$RECEIPT" "$bin"; printf 'moved %s\n' "$bin" >"$SHAREW/journal.$pid"; do_status"#,
    )]);
    let u = case(&units, "journal_only");
    assert!(
        u.has_out_line("leftover: ~/.local/share/watane/journal.99999999"),
        "{u:?}"
    );
    assert_eq!(
        u.out.lines().last(),
        Some(
            "Next: run: sh ~/.local/share/watane/install.sh (it recovers the interrupted install first)"
        ),
        "{u:?}"
    );
}
