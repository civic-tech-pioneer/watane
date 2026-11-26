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
    let base = r#"rcpt "$RECEIPT" "$bin" "$SHAREW/install.sh""#;
    let lock = r#"mkdir "$HOME/.watane-install.lock""#;
    let left = r#": >"$bin.old.$pid"; : >"$SHAREW/install.sh""#;
    let left_no_script = r#": >"$bin.old.$pid""#;
    let cases = [
        ("clean", format!("{base}\ndo_status")),
        ("lock", format!("{base}\n{lock}\ndo_status")),
        ("left", format!("{base}\n{left}\ndo_status")),
        ("both", format!("{base}\n{lock}\n{left}\ndo_status")),
        (
            "left_no_script",
            format!("{base}\n{left_no_script}\ndo_status"),
        ),
        (
            "both_no_script",
            format!("{base}\n{lock}\n{left_no_script}\ndo_status"),
        ),
        // a fresh install killed at its first move: no receipt, and `install.sh` is only staged
        (
            "fresh_kill_before_the_script",
            "printf 'new %s\\nmoved %s\\n' \"$bin\" \"$bin\" >\"$SHAREW/journal.$pid\"\n: >\"$bin\"; : >\"$SHAREW/install.sh.new.$pid\"\ndo_status"
                .to_string(),
        ),
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
    assert_eq!(
        last("left_no_script"),
        "Next: run the install command again (it recovers the interrupted install first)"
    );
    assert_eq!(
        last("both_no_script"),
        "Next: if no install is running, run: rm -rf ~/.watane-install.lock, then run the install command again"
    );
    let fresh = case(&units, "fresh_kill_before_the_script");
    assert_eq!(fresh.code, 7, "{fresh:?}");
    assert_eq!(
        fresh.out.lines().last(),
        Some("Next: run the install command again (it recovers the interrupted install first)"),
        "{fresh:?}"
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
        r#"rcpt "$RECEIPT" "$bin" "$SHAREW/install.sh"; printf 'moved %s\n' "$bin" >"$SHAREW/journal.$pid"; : >"$SHAREW/install.sh"; do_status"#,
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

/// TECHNICAL 8.4 step 1 (validator, S137-1 item 3): the line `The files of the interrupted install
/// were removed.` follows the `recovered interrupted watane install` line and is left out with it
/// while another journal stays pending.
#[test]
fn regress_s135_uninstall_says_what_it_removed_only_when_it_said_recovered() {
    let units = run(&[
        (
            "pending",
            r#"BINRES=/elsewhere rcpt "$RECEIPT.new.$pid" "$bin" "$node"
: >"$node"; fresh_journal "$node"; : >"$SHAREW/VERSION.new.99999998"
do_uninstall"#,
        ),
        (
            "tidied",
            r#"rcpt "$RECEIPT.new.$pid" "$bin"; : >"$SHAREW/VERSION.new.99999998"
do_uninstall"#,
        ),
    ]);
    let line = "The files of the interrupted install were removed.";
    let pending = case(&units, "pending");
    assert_eq!(pending.code, 7, "{pending:?}");
    assert!(!pending.has_err_line(line), "{pending:?}");
    assert!(!pending.has_out_line("recovered interrupted watane install"));
    let tidied = case(&units, "tidied");
    assert!(tidied.has_err_line(line), "{tidied:?}");
    assert!(tidied.has_out_line("recovered interrupted watane install"));
}

/// TECHNICAL 8.1 (validator, S137-1 round 2): a link at `install.sh` is not named as the script
/// to run, wherever it leads. Mutant: drop `[ ! -L "$SHAREW/install.sh" ]` in `do_status`.
#[test]
fn regress_s135_status_next_step_does_not_name_a_linked_script() {
    let fx = Fx::new();
    fs::create_dir_all(fx.share("watane")).expect("share");
    write_file(&fx.home.join("other.sh"), "echo other\n");
    symlink(fx.home.join("other.sh"), fx.share("watane/install.sh")).expect("link");
    let units = fx.units(
        PRELUDE,
        &[],
        &[(
            "linked_script",
            r#"rcpt "$RECEIPT" "$bin" "$SHAREW/install.sh"; : >"$bin.old.$pid"; do_status"#,
        )],
    );
    let u = case(&units, "linked_script");
    assert_eq!(
        u.out.lines().last(),
        Some("Next: run the install command again (it recovers the interrupted install first)"),
        "{u:?}"
    );
}

/// TECHNICAL 8.1 (S137-fix1 AC1): the script is named only when a readable receipt lists it: a
/// regular `install.sh` that no receipt lists (here an empty staged receipt and a stale lock) is
/// not watane's, and a staged receipt that lists it counts when there is no published one.
/// Mutant: drop `[ "$ds_owned" = 1 ] &&` in `do_status`.
#[test]
fn regress_s135_status_names_the_script_only_when_a_readable_receipt_lists_it() {
    let plant = r#"mkdir "$HOME/.watane-install.lock"; : >"$SHAREW/install.sh"
printf 'moved %s\n' "$bin" >"$SHAREW/journal.$pid""#;
    let units = run(&[
        (
            "unlisted",
            &format!("{plant}\n: >\"$RECEIPT.new.$pid\"\ndo_status"),
        ),
        (
            "staged_lists_it",
            &format!(
                "{plant}\nrcpt \"$RECEIPT.new.$pid\" \"$bin\" \"$SHAREW/install.sh\"\ndo_status"
            ),
        ),
    ]);
    let again = "Next: if no install is running, run: rm -rf ~/.watane-install.lock, then run the install command again";
    let named = "Next: if no install is running, run: rm -rf ~/.watane-install.lock, then run: sh ~/.local/share/watane/install.sh";
    assert_eq!(case(&units, "unlisted").out.lines().last(), Some(again));
    assert_eq!(
        case(&units, "staged_lists_it").out.lines().last(),
        Some(named),
        "{:?}",
        case(&units, "staged_lists_it")
    );
}

/// TECHNICAL 8.4 step 1 (S137-fix1 AC2): when the `.new` cleanup skips a folder, so a staging name
/// stays, uninstall prints no claim that the files were removed; the line still prints when nothing
/// was left. Mutant: drop `&& [ "$CS_LEFT" = 0 ]` in `do_uninstall`.
#[test]
fn regress_s135_uninstall_does_not_claim_removal_when_a_staging_name_stays() {
    let units = run(&[
        (
            // the staged receipt records ~/.local/bin elsewhere: its stale name stays
            "left_behind",
            r#"BINRES=/elsewhere rcpt "$RECEIPT.new.$pid" "$bin"
: >"$bin.new.$pid"; : >"$SHAREW/VERSION.new.99999998"
do_uninstall"#,
        ),
        (
            "nothing_left",
            r#"rcpt "$RECEIPT.new.$pid" "$bin"; : >"$SHAREW/VERSION.new.99999998"
do_uninstall"#,
        ),
    ]);
    let line = "The files of the interrupted install were removed.";
    let kept = case(&units, "left_behind");
    assert_eq!(kept.code, 7, "{kept:?}");
    assert!(
        kept.has_out_line("recovered interrupted watane install"),
        "{kept:?}"
    );
    assert!(!kept.has_err_line(line), "{kept:?}");
    assert!(case(&units, "nothing_left").has_err_line(line));
}

/// TECHNICAL 8.1 (validator, S137-fix1): a published receipt that does not list `install.sh`
/// does not make a regular file there watane's. Mutant: set `ds_owned=1` for any published receipt.
#[test]
fn regress_s135_status_does_not_name_a_script_the_published_receipt_omits() {
    let units = run(&[(
        "omitted",
        r#"rcpt "$RECEIPT" "$bin"; : >"$SHAREW/install.sh"; : >"$bin.old.$pid"; do_status"#,
    )]);
    let u = case(&units, "omitted");
    assert_eq!(
        u.out.lines().last(),
        Some("Next: run the install command again (it recovers the interrupted install first)"),
        "{u:?}"
    );
}

/// TECHNICAL 8.4 step 1 (validator, S137-fix1): a stale name kept because its run's receipt
/// location is vetoed, or because its folder resolves outside the home, also stops the removal
/// line; another dead run's stale name is removed, so `recovered` still prints. Mutants: drop
/// `CS_LEFT=1` after the `recovery_allowed "$RECEIPT"` and the `allowed "$cs_e"` checks.
#[test]
fn regress_s135_uninstall_does_not_claim_removal_for_a_vetoed_or_outside_stale_name() {
    let tidy = r#": >"$SHAREW/VERSION.new.99999998""#;
    let receipt_vetoed = format!(
        r#"rcpt "$RECEIPT.new.$pid" "$bin"
printf 'resolved .local/share/watane /elsewhere\n' >>"$RECEIPT.new.$pid"
: >"$bin.new.$pid"; {tidy}; do_uninstall"#
    );
    let mut units = run(&[("receipt_vetoed", receipt_vetoed.as_str())]);
    let fx = Fx::new();
    let out = fx.root.path().join("out");
    fs::create_dir_all(&out).expect("out");
    fs::write(out.join("watane.new.99999999"), "").expect("stale");
    fs::create_dir_all(fx.home.join(".local")).expect("local");
    symlink(&out, fx.home.join(".local/bin")).expect("bin link");
    let outside = format!(r#"{tidy}; do_uninstall"#);
    units.extend(fx.units(PRELUDE, &[], &[("outside", outside.as_str())]));
    let line = "The files of the interrupted install were removed.";
    for name in ["receipt_vetoed", "outside"] {
        let u = case(&units, name);
        assert_eq!(u.code, 7, "{name}: {u:?}");
        assert!(
            u.has_out_line("recovered interrupted watane install"),
            "{name}: {u:?}"
        );
        assert!(!u.has_err_line(line), "{name}: {u:?}");
    }
    assert!(out.join("watane.new.99999999").exists());
}

/// TECHNICAL 8.1 (S137 r1 F1.1): the script is named only when the receipt that lists it records
/// the physical folder it is in: a receipt with no `resolved .local/share/watane` line gets the
/// fallback line; one that records another folder (S137 r2 P2.1) never names the script.
/// Mutant: drop the `in_list "$ds_owned_at"` check in `do_status`.
#[test]
fn regress_s137_status_names_the_script_only_in_the_folder_the_receipt_records() {
    let plant = r#"rcpt "$RECEIPT" "$bin" "$SHAREW/install.sh"; : >"$SHAREW/install.sh"; : >"$bin.old.$pid""#;
    let units = run(&[
        ("recorded", &format!("{plant}\ndo_status")),
        (
            "mismatched",
            &format!(
                "{plant}\nsed 's|^resolved .local/share/watane .*|resolved .local/share/watane '\"$HOME_R\"'/elsewhere|' \"$RECEIPT\" >\"$RECEIPT.t\" && mv \"$RECEIPT.t\" \"$RECEIPT\" || exit 99\ngrep -q '/elsewhere$' \"$RECEIPT\" || exit 98\ndo_status"
            ),
        ),
        (
            "unrecorded",
            &format!(
                "{plant}\ngrep -v '^resolved .local/share/watane ' \"$RECEIPT\" >\"$RECEIPT.t\" && mv \"$RECEIPT.t\" \"$RECEIPT\" || exit 99\ndo_status"
            ),
        ),
    ]);
    let tail = "(it recovers the interrupted install first)";
    let recorded = case(&units, "recorded");
    assert_eq!(
        recorded.out.lines().last(),
        Some(format!("Next: run: sh ~/.local/share/watane/install.sh {tail}").as_str()),
        "{recorded:?}"
    );
    let unrecorded = case(&units, "unrecorded");
    assert_eq!(
        unrecorded.out.lines().last(),
        Some(format!("Next: run the install command again {tail}").as_str()),
        "{unrecorded:?}"
    );
    // a record of another folder also vetoes the leftover (8.5), so no `Next:` line names the script
    let mismatched = case(&units, "mismatched");
    assert_eq!(mismatched.code, 0, "{mismatched:?}");
    assert!(!mismatched.out.contains("run: sh "), "{mismatched:?}");
    assert!(
        mismatched.has_out_line("~/.local/share/watane/install.sh"),
        "{mismatched:?}"
    );
}

/// TECHNICAL 8.4 step 1 (S137 r1 F1.2): after a fresh pre-commit crash, `~/.local/bin` moved
/// aside and relinked outside the home: the undo leaves the moved entry alone (it skips it) and
/// retires the journal, and uninstall then makes no claim that the files were removed. Without
/// the relink the claim still prints. Mutant: drop `CS_LEFT=1` after `read_journal` in `recover`.
#[test]
fn regress_s137_uninstall_does_not_claim_removal_when_recovery_skipped_an_entry() {
    let staged = r#"rcpt "$RECEIPT.new.$pid" "$node"; fresh_journal "$node""#;
    let fx = Fx::new();
    let out = fx.root.path().join("out");
    fs::create_dir_all(&out).expect("out");
    let aside = fx.home.join(".local/bin.aside");
    fs::create_dir_all(&aside).expect("aside");
    fs::write(aside.join("watane-node"), "").expect("moved entry");
    symlink(&out, fx.home.join(".local/bin")).expect("bin link");
    let relinked = format!("{staged}\ndo_uninstall");
    let mut units = fx.units(PRELUDE, &[], &[("relinked", relinked.as_str())]);
    let in_place = format!("{staged}; : >\"$node\"\ndo_uninstall");
    units.extend(run(&[("in_place", in_place.as_str())]));
    let line = "The files of the interrupted install were removed.";
    let u = case(&units, "relinked");
    assert_eq!(u.code, 7, "{u:?}");
    assert!(
        u.has_out_line("recovered interrupted watane install"),
        "{u:?}"
    );
    assert!(!u.has_err_line(line), "{u:?}");
    assert!(aside.join("watane-node").exists());
    let kept = case(&units, "in_place");
    assert_eq!(kept.code, 7, "{kept:?}");
    assert!(kept.has_err_line(line), "{kept:?}");
}

/// TECHNICAL 3.6 step 0, 8.4 step 1 (S137 r2 F2.1): a committed swap whose journal cannot be
/// read, beside a readable published receipt. Install's recovery finishes the commit and retires
/// the journal as before S137 r1 (it never reads the journal); uninstall's recovery does the same
/// and, since it could not inspect the journal, claims no removal (`CS_LEFT`). Red at 2ccb2b8: the
/// inspection read every journal and the shell exited 2. Needs a user that file modes bind, so
/// it passes vacuously as root.
#[test]
fn regress_s137_an_unreadable_committed_journal_is_still_recovered() {
    let uid = std::process::Command::new("id")
        .arg("-u")
        .output()
        .expect("id");
    if String::from_utf8_lossy(&uid.stdout).trim() == "0" {
        eprintln!("skipped: root reads a file of mode 000");
        return;
    }
    let plant = r#"rcpt "$RECEIPT" "$bin"; : >"$bin"; : >"$bin.old.$pid"
printf 'old %s\nmoved %s\n' "$bin" "$bin" >"$SHAREW/journal.$pid"; chmod 000 "$SHAREW/journal.$pid""#;
    let after =
        r#"echo "left=$CS_LEFT"; kept "$SHAREW/journal.$pid"; kept "$bin.old.$pid"; kept "$bin""#;
    let units = run(&[
        ("install", &format!("{plant}\nrecover\n{after}")),
        ("uninstall", &format!("{plant}\nrecover prune\n{after}")),
    ]);
    for (name, left) in [("install", "left=0"), ("uninstall", "left=1")] {
        let u = case(&units, name);
        assert_eq!(u.code, 0, "{u:?}");
        assert!(
            u.has_out_line("recovered interrupted watane install"),
            "{u:?}"
        );
        assert!(u.has_out_line(left), "{u:?}");
        assert_eq!(u.out.matches("gone").count(), 2, "{u:?}");
        assert!(u.has_out_line("kept"), "{u:?}");
    }
}
