Subsystem: install
Effort: medium
Depends on: none
Slice: S89 — Alice installs, interrupts, checks and uninstalls watane, and every line she reads is true and in her own terms

## Goal
After a failed swap (exit 5), the second line is decided by what undo actually did, not by what
existed before the swap: when undo failed (after its one retry, 3.8) for any entry, new or old,
on a fresh install or a reinstall, the run prints
`The files could not be put back. Run this script again to restore the installed version.` on a
reinstall, and on a fresh install `The files could not be removed. Run this script again to finish
cleaning up.` (define both in TECHNICAL 3.8/3.9; the fresh-install wording is a recommendation).
"Nothing was installed." / "Nothing was changed: ..." are printed only when undo ended well.
This closes the adversary's third red test and its RISK (an entry in the journal's new list,
absent from the old version, that undo failed to remove on a reinstall).

## Acceptance
AC1. `adv_s89::adv_s89_fresh_install_whose_undo_fails_says_nothing_was_installed` passes unchanged.
AC2. A test: reinstall/upgrade where the new version has an entry the old lacked (craft the
     receipt of the first install so that one entry is absent from its list, or another cheap
     fixture), undo fails to remove that entry: the run does not print "Nothing was changed".
AC3. Every existing exit-5 test (script, adv_s50b*, s89 fix2_*, sim_s89, adv_s89) still passes.
AC4. All install tests pass with `--include-ignored` apart from the known environment failures;
     shellcheck, clippy, fmt clean.

## Spec
install/TECHNICAL.md 3.6 step 4, 3.8, 3.9, IN-45.

## Interface
none.

## Files
install/install.sh, install/TECHNICAL.md, install/FUNCTIONAL.md, install/NOTES.md,
install/tests/it/s89.rs, install/tests/support/*, install/tests/fakes/*.

## Changed tests
adv_s89.rs and sim_s89.rs must not change (unless user-sim's second run adds a red test the
foreman names). Existing tests pinning the exit-5 second line: search and update only where AC1-2
change the truth.

## Out of scope
macOS sh runs (PO).
