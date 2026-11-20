Subsystem: install
Effort: low
Depends on: none
Slice: S89 — Alice installs, interrupts, checks and uninstalls watane, and every line she reads is true and in her own terms

## Goal
A failure before any swap (the die 4/5 sites of `verify_archive` and `extract_archive`) claims
nothing about the installed version: with a receipt present at the start it prints
`Nothing was changed.`, on a fresh home `Nothing was installed.` (unchanged). Foreman decision:
"still works" is only true where the run checked it, and a pre-swap failure runs before `recover`
(3.6 step 0), so it cannot know. Also `rollback` treats a journal as present only when it is a
regular file and not a link (`[ -f ... ] && [ ! -L ... ]`), as `find_journals` and `undo_swap` do.

Also (user-sim, third run): on a fresh home, "The files could not be removed. Run this script
again to finish cleaning up." undersells what a rerun does (it cleans up and installs). Make it
`The files could not be removed. Run this script again: it removes them and installs watane.`
(TECHNICAL 3.8/3.9 first), and update the tests that pin the old text.

## Acceptance
AC1. `adv_s89::adv_s89_checksum_failure_after_an_interrupted_uninstall_says_the_version_still_works`
     and `adv_s89::adv_s89_checksum_failure_after_a_killed_swap_says_the_version_still_works`
     pass unchanged.
AC2. `s89::fix3_a_checksum_failure_on_a_reinstall_does_not_say_nothing_was_installed` and
     `s89::fix3_an_archive_failure_on_a_reinstall_does_not_say_nothing_was_installed` expect
     `Nothing was changed.` (changed tests).
AC3. TECHNICAL 3.9 (and 3.8 / IN-45 where they cite it) states the rule and why.
AC4. All install tests pass with `--include-ignored` apart from the known environment
     failures; shellcheck, clippy (-p watane-install), fmt clean.

## Spec
install/TECHNICAL.md 3.6 step 0, 3.8, 3.9.

## Interface
none.

## Files
install/install.sh, install/TECHNICAL.md, install/NOTES.md, install/tests/it/s89.rs.

## Changed tests
tests pinning "to finish cleaning up" (s89.rs fix3_*); the two fix3_*_on_a_reinstall_* tests in s89.rs (AC2). adv_s89.rs and sim_s89.rs must not change.

## Out of scope
The post-undo "Nothing was changed: the version that was installed still works." after a failed
swap (INST-F-18 territory, recorded as a leftover).
