Subsystem: install
Effort: low
Depends on: s50b-fix-6
Slice: S50b-fix — install's tests pass on macOS

## Goal
The adversary's two red tests of commit e6563c3 (`install/tests/adv_s50b_fix.rs`) pass unchanged:
a Ctrl-C during a rollback (recovery's undo of a dead run, or the undo after a failed swap) ends
with 130 and no cause line, instead of exit 5 with "could not recover interrupted install" or
"could not put the files in place". Inside `undo_swap`'s subshell, `|| exit 1` replaces each
program's status, `rollback` ignores `undo_swap`'s status, and `install_files` and `recover`
check only the swap's own status, so the INT is lost.

## Acceptance
- AC1 `adv_s50b_fix_ctrl_c_in_recovery_undo_prints_a_cause` and
  `adv_s50b_fix_ctrl_c_in_swap_undo_prints_a_cause` pass unchanged.
- AC2 Every `|| exit 1` in `undo_swap` (the `mv`, `mkdir`, journal `rm -f`, `rm_path`,
  `finish_commit` calls, around install.sh:291-327) exits with the program's own status;
  `rollback` returns `undo_swap`'s status; `install_files` and `recover` pass it to
  `check_signal` before their `die 5`. The subshell's `SIGNAL=0` (so that the undo finishes)
  stays as it is. A repeated undo stays safe (the ac4 undo tests pass).
- AC3 Search the whole script once more for any other place where a status from a program or a
  function that runs programs is replaced by a constant (`|| exit <n>`, `|| return <n>`,
  `|| die`) without passing through `check_signal`; fix each and list them in install/NOTES.md.
- AC3b (product owner) Close the class by a rule, not by sites: every place in install.sh where
  a program's status is dropped or remapped (`|| exit n`, `|| return n`, `|| die`, `|| true`,
  `|| x=""`, `if ! prog`, a `$(...)` whose status is overwritten) passes through the signal
  check. Add one test (e.g. a scan of install.sh's text) that fails when a new site of these
  forms skips `check_signal`, with an explicit, commented allow-list for the few that must
  (the undo subshell's `SIGNAL=0` path, builtins-only substitutions). Write the rule into
  TECHNICAL 3.8.
- AC4 Real failures keep their text and code; every other test passes; shellcheck 0.

## Spec
install/TECHNICAL 3.8 (the rule of s50b-fix-6 now covers the undo). Record in install/NOTES.md.

## Interface
none.

## Files
`install/install.sh`, `install/tests/*`, `install/TECHNICAL.md`, `install/NOTES.md`.

## Changed tests
none.

## Out of scope
`--status` needing `mktemp` (adversary RISK: for the PO); anything outside `install/`.
