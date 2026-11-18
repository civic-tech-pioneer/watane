Subsystem: install
Effort: medium
Depends on: s50b-fix-4
Slice: S50b-fix — install's tests pass on macOS

## Goal
The adversary's three red tests on s50b-fix-4 (commit 750510d, `install/tests/adv_s50b_fix.rs`)
pass unchanged, and user-sim's red test keeps passing. s50b-fix-4 defers every signal before
the swap (`SIGNAL_DEFER`) and acts on it only at checkpoints, so after a Ctrl-C the run still
downloads the archive (test 1), prints a cause line with a signal's exit code (test 2, the
foreign-file check), and in a two-builtin window in `do_swap` installs and reaches the
toolchain offer (test 3).
Decision (foreman): a signal acts at once again, at every step, as before s50b-fix-4. Remove
the deferral. The false causes user-sim saw come only from failure paths that bash 3.2 reaches
before it runs the pending trap; so only those paths change: before `die` (and any other path
that prints a cause and exits: `check_foreign`'s exit 8, the checksum, unpack, version and tool
checks) give a pending trap its dispatch point (`check_signal`'s `( : )` and the mapping of a
child's 129/130/141/143 status), so a run that saw a signal ends with its code and no cause line.
Recovery (3.6 step 0) is interrupted at once as before; s50b-fix-3 made a cut-short undo safe
to repeat on the next run.

## Acceptance
- AC1 `adv_s50b_fix_term_during_download_still_fetches_the_archive`,
  `adv_s50b_fix_term_before_the_foreign_check_prints_a_cause`,
  `adv_s50b_fix_term_after_the_swap_checkpoint_installs_and_reaches_the_toolchain` pass unchanged.
  Test 3 inserts a `kill` line after `do_swap`'s `check_signal`; if the code it anchors on goes away
  with the deferral, say so and make sure the same moment (a signal just before the swap) is
  covered by a test that still asserts no install and no toolchain step.
- AC2 `regress_s50b_fix_ctrl_c_during_the_checks_names_no_false_cause` passes unchanged, three
  runs in a row on this Mac.
- AC3 The validator's tests of s50b-fix-4 (`install/tests/check_signals.rs`) keep passing, or a
  test that asserts deferral itself changes to assert the immediate exit (list it under Changed
  tests in your report). Real failures keep their 3.9 text and code. Every other test passes;
  `shellcheck -s sh install.sh package.sh` exits 0.

## Spec
install/TECHNICAL 3.8: replace the deferral rule with "a signal ends the run at once with its
code (undoing an active swap); a failure path first lets a pending signal end the run, so a
signalled run prints no cause line". 3.2, 3.4 where s50b-fix-4 cited deferral. Record in
install/NOTES.md.

## Interface
none.

## Files
`install/install.sh`, `install/tests/*`, `install/TECHNICAL.md`, `install/NOTES.md`.

## Changed tests
Tests in `install/tests/check_signals.rs` that assert a deferred signal (if any) change to the
immediate exit; no other assertion changes.

## Out of scope
Anything outside `install/`; user-sim's "felt wrong" notes.
