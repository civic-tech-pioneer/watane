Subsystem: install
Effort: low
Depends on: s50b-fix-7
Slice: S50b-fix — install's tests pass on macOS

## Goal
The status scan (`install/tests/status_audit.rs`) rejects every spelling of a negated program
predicate, and a failed undo no longer claims nothing changed.
1. Adversary red test `adv_s50b_fix_scan_misses_negated_predicates` (commit 416498e) passes
   unchanged: the scan accepts `elif ! tool`, `if [ -n "$x" ] && ! tool`, `while ! tool`,
   `if  ! tool` (two spaces), `if<TAB>! tool` and a bare `! tool`, because it treats `!` only
   after `if`/`&&`/`||` and reads only lines containing `||`, `if !` or `$(`.
2. When the undo itself fails with a non-signal error, `install_files` prints "Nothing was
   changed: the version that was installed still works." and exits 5, while
   `~/.local/bin/watane` and `watane.app` may be missing until the next run recovers them.
   Decision (foreman): in that case print instead that the files could not be put back and that
   running the script again restores the installed version (exit 5 stays); keep the old text
   when the undo ended well.

## Acceptance
- AC1 `adv_s50b_fix_scan_misses_negated_predicates` passes unchanged; the scan reads every line
  with a `!` in command position (after `if`, `elif`, `while`, `until`, `&&`, `||`, `;`, `|`,
  or at the start of a command, any whitespace) and applies the same rule to it.
- AC2 Decision (foreman) for 3.8: the scanned forms are the listed spellings plus these. The
  other forms the adversary named (`if prog; then` with no checking else, `until prog`,
  `prog && x`, a bare pipeline, `prog; :` in the `set +e` undo subshell, `if helper` where the
  helper does `|| return`) are reviewed by hand, not scanned. Name them in 3.8 so a reviewer
  knows; the scan does not need to reject them.
- AC3 A test: the undo fails for good (a fake `mv` of `*.old.*` exits 1) after a failed swap;
  the run exits 5, does NOT print "Nothing was changed", prints the new text, and the next run
  restores and installs. The existing "Nothing was changed" assertions for an undo that ended
  well stay.
- AC4 Every other test passes; shellcheck 0.

## Spec
install/TECHNICAL 3.8 (the scan's scope; the failed-undo text), 3.9 (the exit-5 texts). Record in
install/NOTES.md.

## Interface
none.

## Files
`install/install.sh`, `install/tests/*`, `install/TECHNICAL.md`, `install/NOTES.md`.

## Changed tests
none expected; if a test asserts "Nothing was changed" on a run whose undo failed, it changes to
the new text (list it).

## Out of scope
Exceptions matched by function name when a function is not written `name() {` (adversary RISK,
contrived); the cleanup-time lost INT (capped class: known limit, 3.8); anything outside install/.
