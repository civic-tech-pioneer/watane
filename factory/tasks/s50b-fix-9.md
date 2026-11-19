Subsystem: install
Effort: low
Depends on: s50b-fix-8
Slice: S50b-fix — install's tests pass on macOS

## Goal
The adversary's two red tests of commit 72a297e pass unchanged.
1. `status_audit.rs` `adv_s50b_fix_scan_misses_case_arm_background_substitution_and_continued_quote`:
   the scan also treats `)` (a case arm) and a single `&` as command position, rejects a negation
   inside `$( ! … )` / `$(! …)` whatever follows (its `$?` is the inverted 0/1), and carries
   quote state across lines so a string opened on one line and closed on the next hides nothing.
2. `adv_s50b_fix.rs` `adv_s50b_fix_staging_cleanup_failure_claims_files_not_put_back`: "The files
   could not be put back…" is printed only when it is true. Decision (foreman): when the first
   undo fails, `install_files` retries it once itself; then it prints "could not be put back"
   only when an entry the run moved is actually missing from its path (the old version is not
   whole), else "Nothing was changed…". A failure that only leaves staging names behind is not
   a failure to put back (the next run's stale cleanup removes them). Exit 5 either way.
Foreman decision on the scan's scope: the scan is a regression guard for honest edits, not a
security boundary (whoever can edit install.sh controls it). After this task its remaining
limits are listed in TECHNICAL 3.8 as hand-review forms; later adversary passes do not hunt them.

## Acceptance
- AC1 Both red tests pass unchanged.
- AC2 A test for the transient case: the first undo's `mv` of `*.old.*` fails once and the
  retry succeeds: the run prints "Nothing was changed…", exits 5, the old version is whole.
- AC3 TECHNICAL 3.8: the scan's scope sentence (regression guard; the hand-review forms,
  including any shell construct the line tokenizer still cannot read); 3.9's exit-5 texts.
- AC4 Every other test passes; shellcheck 0.

## Spec
install/TECHNICAL 3.8, 3.9. Record in install/NOTES.md.

## Interface
none.

## Files
`install/install.sh`, `install/tests/*`, `install/TECHNICAL.md`, `install/NOTES.md`.

## Changed tests
none expected.

## Out of scope
Anything outside install/.
