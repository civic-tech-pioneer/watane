//! Adversarial tests of S94 (TECHNICAL 12.1): guard behaviours of `install.sh` that the suite at
//! `9c743c3` proved in its default set and the restructured default set no longer does; each is
//! red against the mutant of `install.sh` named in its `///` line, and green on `install.sh`.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;

use crate::support::unit::*;
use crate::support::*;

/// A folder of the world, outside every home, with its physical path.
fn outside(fx: &Fx, name: &str) -> PathBuf {
    let p = fx.root.path().join(name);
    fs::create_dir_all(&p).expect("outside");
    fs::canonicalize(&p).expect("canonical")
}

/// TECHNICAL 3.6 step 2 (AC2 of S50b): a link planted at `receipt.new.<pid>` or `journal.<pid>` is
/// never written through. Mutant: in `stage_files` (and, apart, in `write_journal`) delete the two
/// lines `! exists "...$$" || return 1` and `set -C`; the default set then passes while the victim
/// outside the home is overwritten (only the `slow:` `script::ac2_a_planted_link_is_not_written_through`
/// sees it). Gain: a local program that plants the link makes the install write over any file the
/// person can write, outside the home.
#[test]
fn adv_s94_a_planted_link_at_the_receipt_or_the_journal_is_not_written_through() {
    let fx = Fx::new();
    let mut cases = Vec::new();
    for (name, plant, call) in [
        ("receipt", "$RECEIPT.new.$$", "RELS=''; stage_files"),
        ("journal", "$SHAREW/journal.$$", "write_journal"),
    ] {
        let home = fx.root.path().join(name);
        fs::create_dir_all(home.join(".local/share/watane")).expect("share");
        let victim = fx.root.path().join(format!("victim_{name}"));
        write_file(&victim, "victim\n");
        symlink(&victim, fx.root.path().join(format!("plant_{name}"))).expect("plant");
        cases.push((
            name,
            format!(
                "HOME=$ROOT/{name}; check_home; set_entries; FOLDERS_RESOLVED=''; mv \"$ROOT/plant_{name}\" \"{plant}\" || exit 99; {call}; echo \"rc=$?\""
            ),
        ));
    }
    let refs: Vec<(&str, &str)> = cases.iter().map(|(n, b)| (*n, b.as_str())).collect();
    let units = fx.units("", &[], &refs);
    for (name, _) in &cases {
        let u = case(&units, name);
        assert_eq!(
            fs::read_to_string(fx.root.path().join(format!("victim_{name}"))).expect("victim"),
            "victim\n",
            "{name}: written through the planted link: {u:?}"
        );
        assert!(!u.has_out_line("rc=0"), "{name}: {u:?}");
    }
}

/// TECHNICAL 3.6 step 3, 3.4.1 (S50c, INST-S-31): an install folder relinked out of the home after
/// `check_folders` stops `swap_entries` before it moves anything, and says so. Mutant: in
/// `swap_entries` replace the two lines `SWAP_CHANGED=...` and `return 1` by `:`; the default set
/// then passes (only the `slow:` `adv_s50c::adv_s50c_link_after_check_lets_upgrade_delete_linked_folder`
/// and `regress_s50c::ac2_folder_changed_during_staging_aborts_before_swap` see it). Gain: a local
/// program that relinks `~/.local/bin` while the run stages has the swap move files, and the commit
/// delete `.old.` names, through the link outside the home.
#[test]
fn adv_s94_swap_stops_at_a_folder_relinked_after_the_check() {
    let fx = Fx::new();
    let home = fx.root.path().join("h");
    fs::create_dir_all(home.join(".local/bin")).expect("bin");
    fs::create_dir_all(home.join(".local/share/watane")).expect("share");
    let out = outside(&fx, "out");
    symlink(&out, home.join("binlink")).expect("link");
    let units = fx.units(
        "",
        &[],
        &[(
            "relinked",
            r#"HOME=$ROOT/h; check_home; set_entries; check_folders
rmdir "$HOME/.local/bin" && mv "$HOME/binlink" "$HOME/.local/bin" || exit 99
swap_entries; echo "rc=$?"; printf 'changed<%s>\n' "$SWAP_CHANGED""#,
        )],
    );
    let u = case(&units, "relinked");
    assert!(u.has_out_line("rc=1"), "{u:?}");
    assert!(
        u.has_out_line(&format!(
            "changed<~/.local/bin changed during the install: it now resolves to {}>",
            out.display()
        )),
        "{u:?}"
    );
    assert_eq!(
        fs::read_dir(&out).expect("out").count(),
        0,
        "nothing written outside the home"
    );
}

/// TECHNICAL 3.4.1, 3.6 step 4 (IN-47): a folder the run made, swapped for a link before the
/// cleanup, is not removed through the link, whether it leads out of the home or into it.
/// Mutants: in `remove_created` delete `below_home "$rc_r" || continue` (case `out`), or delete
/// `[ ! -L "$rc_f" ] || continue` (case `into`); the default set then passes (only the `slow:`
/// `s89::fix1_a_link_swapped_in_after_note_created_is_not_followed_out_of_the_home` and
/// `s89::fix1_a_created_folder_swapped_for_a_link_into_the_home_is_kept` see them). Gain: a local
/// program removes an empty folder of the person's, outside the home or inside it.
#[test]
fn adv_s94_cleanup_keeps_a_created_folder_swapped_for_a_link() {
    let fx = Fx::new();
    // out: ~/.local, made by the run, becomes a link out of the home; its bin is the person's
    fs::create_dir(fx.root.path().join("out")).expect("home");
    let o = outside(&fx, "o");
    fs::create_dir(o.join("bin")).expect("bin");
    symlink(&o, fx.root.path().join("out/locallink")).expect("link");
    // into: ~/Applications, made by the run, becomes a link to a folder of the home
    fs::create_dir_all(fx.root.path().join("into/keep")).expect("keep");
    symlink(
        fx.root.path().join("into/keep"),
        fx.root.path().join("into/applink"),
    )
    .expect("link");
    let units = fx.units(
        "",
        &[],
        &[
            (
                "out",
                r#"HOME=$ROOT/out; check_home; note_created
mv "$HOME/locallink" "$HOME/.local" || exit 99
COMMITTED=0; remove_created; echo "rc=$?""#,
            ),
            (
                "into",
                r#"HOME=$ROOT/into; check_home; note_created
mv "$HOME/applink" "$HOME/Applications" || exit 99
COMMITTED=0; remove_created; echo "rc=$?""#,
            ),
        ],
    );
    let u = case(&units, "out");
    assert!(u.has_out_line("rc=0"), "{u:?}");
    assert!(o.join("bin").is_dir(), "removed outside the home: {u:?}");
    let u = case(&units, "into");
    assert!(u.has_out_line("rc=0"), "{u:?}");
    assert!(
        fx.root.path().join("into/keep").is_dir(),
        "removed through a link into the home: {u:?}"
    );
}

/// TECHNICAL 8.5, 3.8 (S50c ac3): an uninstall whose share folder is a link out uses neither the
/// receipt's journals there nor the staging names they would explain: the dead run's
/// `journal.<n>` and `receipt.new.<n>` (outside the home) and its `bin/watane.new.<n>` stay.
/// Mutants: in `recover` delete the three lines `if ! allowed "$RECEIPT"; then`, `return 0`, `fi`;
/// or in `do_uninstall` replace `if allowed "$RECEIPT"; then clean_stale` by
/// `if true; then clean_stale`. The default set then passes (only the `slow:`
/// `s50c::ac3_disallowed_receipt_preserves_staging_evidence` sees them). Gain: a local program that
/// relinks `~/.local/share/watane` has the uninstall delete files outside the home (the journal and
/// `receipt.new.<n>` where the link leads) and the staging evidence of a killed run.
#[test]
fn adv_s94_uninstall_through_a_linked_share_folder_keeps_staging_names() {
    let fx = Fx::new();
    let home = fx.installed_home("d");
    let sw = home.join(".local/share/watane");
    write_file(&sw.join("journal.99999999"), "journal evidence");
    write_file(&sw.join("receipt.new.99999999"), "receipt evidence");
    write_file(
        &home.join(".local/bin/watane.new.99999999"),
        "staging evidence",
    );
    let o = outside(&fx, "o");
    fs::rename(&sw, o.join("watane")).expect("move");
    symlink(o.join("watane"), &sw).expect("link");
    let units = fx.units(
        "",
        &[],
        &[(
            "linked",
            r#"HOME=$ROOT/d; check_home; set_entries; do_uninstall; echo "rc=$?""#,
        )],
    );
    let u = case(&units, "linked");
    assert!(u.has_out_line("rc=0"), "{u:?}");
    let read = |p: PathBuf| fs::read_to_string(p).ok();
    assert_eq!(
        read(o.join("watane/journal.99999999")).as_deref(),
        Some("journal evidence"),
        "{u:?}"
    );
    assert_eq!(
        read(o.join("watane/receipt.new.99999999")).as_deref(),
        Some("receipt evidence"),
        "{u:?}"
    );
    assert_eq!(
        read(home.join(".local/bin/watane.new.99999999")).as_deref(),
        Some("staging evidence"),
        "{u:?}"
    );
}

/// INST-S-27, TECHNICAL 3.6 step 5, 8.5: the commit of an upgrade removes a path that only the old
/// receipt names only when 8.5 allows it; a receipt line naming `~/Documents` or a path outside the
/// home is skipped. Mutant: in `finish_commit` delete the line
/// `receipt_allowed "$fc_p" || { check_signal "$?"; continue; }` of the loop over the old
/// receipt's lines; the default set then passes (only the `slow:`
/// `script::inst_s27_an_upgrade_keeps_what_a_receipt_names_beyond_the_install` and
/// `s50c::ac1_upgrade_cleanup_checks_old_receipt_resolution` see it; the slow reason says the
/// uninstall run proves "the removal half", but the uninstall goes through `do_uninstall`'s own
/// call, not this one). Gain: a hostile receipt makes the next upgrade delete any file or folder
/// the person owns, inside the home or out of it.
#[test]
fn adv_s94_an_upgrade_commit_keeps_what_the_old_receipt_names_beyond_the_install() {
    let fx = Fx::new();
    let home = fx.installed_home("u");
    let o = outside(&fx, "o");
    write_file(&home.join("Documents/marker"), "marker");
    write_file(&o.join("x/marker"), "marker");
    let sw = home.join(".local/share/watane");
    let receipt = fs::read_to_string(sw.join("receipt")).expect("receipt");
    fs::write(
        sw.join("receipt.old.99999999"),
        format!(
            "{receipt}path {0}/Documents\npath {0}/.local/../Documents\npath {1}/x\n",
            home.display(),
            o.display()
        ),
    )
    .expect("old receipt");
    let units = fx.units(
        "",
        &[],
        &[(
            "commit",
            r#"HOME=$ROOT/u; check_home; set_entries; finish_commit 99999999; echo "rc=$?""#,
        )],
    );
    let u = case(&units, "commit");
    assert!(u.has_out_line("rc=0"), "{u:?}");
    assert!(home.join("Documents/marker").is_file(), "{u:?}");
    assert!(o.join("x/marker").is_file(), "{u:?}");
    assert!(!sw.join("receipt.old.99999999").exists(), "{u:?}");
}

/// TECHNICAL 3.6 step 3 and 4 (S50c ac3): undo puts the receipt's `.old.<n>` copy back only when
/// the journal says the receipt moved; a copy cut short (no `moved` line for the receipt) never
/// replaces the complete receipt. Mutant: in `undo_swap` delete the line
/// `if [ "$ub_p" = "$RECEIPT" ]; then in_list "$J_MOVED" "$ub_p" || continue; fi`; the default set
/// then passes (only the `slow:` `s50c::ac3_partial_receipt_copy_keeps_complete_old_receipt` sees
/// it). Gain: a run killed while it copies the receipt leaves a cut receipt, so the installed
/// version is no longer recognised (its files become foreign, an uninstall finds nothing).
#[test]
fn adv_s94_undo_keeps_the_receipt_when_its_copy_was_cut_short() {
    let fx = Fx::new();
    let home = fx.installed_home("r");
    let sw = home.join(".local/share/watane");
    let before = fs::read_to_string(sw.join("receipt")).expect("receipt");
    let mut journal = String::new();
    for l in before.lines() {
        if let Some(p) = l.strip_prefix("path ") {
            journal.push_str(&format!("old {p}\n"));
        }
    }
    journal.push_str(&format!("old {}\n", sw.join("receipt").display()));
    fs::write(sw.join("journal.99999999"), journal).expect("journal");
    fs::write(sw.join("receipt.new.99999999"), "watane-receipt 1\n").expect("new");
    fs::write(sw.join("receipt.old.99999999"), "watane-rec").expect("cut copy");
    let units = fx.units(
        "",
        &[],
        &[(
            "cut",
            r#"HOME=$ROOT/r; check_home; set_entries; undo_swap 99999999; echo "rc=$?""#,
        )],
    );
    let u = case(&units, "cut");
    assert!(u.has_out_line("rc=0"), "{u:?}");
    assert_eq!(
        fs::read_to_string(sw.join("receipt")).expect("receipt"),
        before,
        "{u:?}"
    );
}

/// INST-S-31, TECHNICAL 3.4.1: a whole run whose `~/.local/bin` is a link to a missing folder
/// outside the home ends with exit 2 and names where it leads. The unit level cannot show this:
/// it runs the functions under `set +e`, and the real run is `set -eu`. Mutant: in
/// `check_folders` replace `if ! resolve_dir "$ch_f" || ! below_home "$RES"; then` by
/// `resolve_dir "$ch_f"; if ! below_home "$RES"; then`; the real run then ends with exit 1 and no
/// text, while `unit::unit_check_folders_refuses_an_install_folder_outside_the_home` (case `o5`)
/// still passes (only the `slow:` `script::inst_s31_every_install_folder_outside_the_home_and_a_link_inside_it`
/// sees it).
#[test]
fn adv_s94_a_dangling_install_folder_link_out_of_the_home_exits_2() {
    let fx = Fx::new();
    let o = outside(&fx, "o");
    fs::create_dir_all(fx.home.join(".local")).expect("local");
    symlink(o.join("bin"), fx.home.join(".local/bin")).expect("link");
    let out = fx.run(&[]);
    assert_eq!(out.code, 2, "{out:?}");
    let home = fs::canonicalize(&fx.home).expect("canon");
    assert_eq!(
        out.first_err(),
        format!(
            "watane install: ~/.local/bin resolves to {}/bin, outside your home folder {}",
            o.display(),
            home.display()
        )
    );
    assert!(fx.rel.requests().is_empty());
    assert!(!o.join("bin").exists());
}

/// TECHNICAL 3.6 step 5, 8.5 rule 4 (S50c regress ac1): the commit removes `<path>.old.<n>` only
/// while the path's folder resolves as the committed receipt recorded; a folder relinked after the
/// commit (here `~/.local/share/watane-dev` to `~/Documents`) keeps the person's
/// `vendor.old.<n>`. Mutant: in `finish_commit` delete the line
/// `receipt_allowed "$fc_p" || { check_signal "$?"; continue; }` of the loop over `$ENTRIES`; the
/// default set then passes (only the `slow:`
/// `regress_s50c::ac1_cleanup_keeps_old_name_when_folder_changes_after_commit` sees it). Gain: a
/// local program that relinks an install folder after the commit has the cleanup delete a folder
/// of the person's that it now leads to.
#[test]
fn adv_s94_the_commit_keeps_an_old_name_when_its_folder_changed_after_the_commit() {
    let fx = Fx::new();
    let home = fx.installed_home("f");
    let dev = home.join(".local/share/watane-dev");
    fs::rename(&dev, home.join("saved-dev")).expect("move");
    write_file(&home.join("Documents/vendor.old.99999999/marker"), "mine");
    symlink(home.join("Documents"), &dev).expect("link");
    let units = fx.units(
        "",
        &[],
        &[(
            "relinked",
            r#"HOME=$ROOT/f; check_home; set_entries; finish_commit 99999999; echo "rc=$?""#,
        )],
    );
    let u = case(&units, "relinked");
    assert!(u.has_out_line("rc=0"), "{u:?}");
    assert_eq!(
        fs::read_to_string(home.join("Documents/vendor.old.99999999/marker")).ok(),
        Some("mine".to_string()),
        "{u:?}"
    );
    assert!(
        u.err.contains(
            "skipped ~/.local/share/watane-dev/vendor: ~/.local/share/watane-dev resolves to"
        ),
        "{u:?}"
    );
}
