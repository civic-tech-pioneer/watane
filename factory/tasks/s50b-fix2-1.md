Subsystem: install
Effort: medium
Depends on: none
Slice: S50b-fix2 — install's tests pass on Linux again, and a Ctrl-C under load on the Mac

## Goal
`cargo test -p watane-install -- --include-ignored` is green on the Mac under load and on Linux
(Linux report: `git show origin/check/s50b-fix-linux:factory/runs/s50b-fix-linux.txt`), and
`shellcheck -s sh install.sh package.sh` reports nothing (shellcheck 0.9 too). Four causes:
1. Mac, under load: `regress_s50b_fix.rs` `regress_s50b_fix_ctrl_c_during_the_checks_names_no_false_cause`
   sees a Ctrl-C at 480..540 ms exit 2 with "the tool id is missing". `install.sh` step 2
   (`main_uid=$(id -u 2>/dev/null) || die 2 "the tool id is missing" ...`) drops `id`'s 130
   without passing it to `check_signal` before `die`. Fix that site, and any other site in
   steps 1 to 4 (`check_version`, `check_tools`, `check_platform`, ...) where a failed status
   reaches `die` without `check_signal "$status"` first. Then find out why the scan test of
   s50b-fix-7 (`status_audit.rs`) did not flag `... || die` and make it flag it.
2. Linux: `adv_s50b_fix.rs` `adv_s50b_fix_ctrl_c_in_recovery_undo_prints_a_cause` names pid 42
   in its stale journal; on Linux pid 42 is a live kernel thread, so the run stops with
   "another install is running" (exit 5). Use a pid that cannot be live on Linux or macOS
   (above Linux's 4194304 pid_max, if the script's journal parsing accepts it; else derive one
   in the test that `kill -0` proves dead). Same for any other test with a fixed small pid.
3. Linux: `script.rs` `inst_s7_target_missing` hangs over 10 min with a defunct `script` child.
   `tests/support/pty.rs` `run_bytes` calls `child.wait()` before `drop(stdin)`; util-linux
   `script` keeps running until its stdin closes. Close stdin before the wait (all pty tests).
4. shellcheck 0.9 SC2015 at `install.sh:257`, `package.sh:30`, `package.sh:44`: rewrite each
   `A && B || C` as an explicit `if`, same behaviour.

## Acceptance
- AC1 The four named tests pass unchanged except the pid in (2).
- AC2 `status_audit.rs` has a test that fails on a `$(cmd) || die ...` site with no
  `check_signal`, and passes on the fixed script.
- AC3 `shellcheck -s sh install.sh package.sh` exits 0 (`waits.rs` ac3 passes).
- AC4 Every other test of watane-install passes, `-- --include-ignored` too; clippy and fmt clean.
- Keep the test waits as S50b-fix set them (600 s, at most 4 installer runs per binary): this
  Mac's endpoint security slows every exec under load.

## Spec
install/TECHNICAL 3.4 (steps 1 to 4), 3.8 (signals, the scan's scope), 12.1 (the pty runner).
Record decisions in install/NOTES.md.

## Interface
none.

## Files
`install/install.sh`, `install/package.sh`, `install/tests/*`, `install/TECHNICAL.md`,
`install/NOTES.md`.

## Changed tests
`adv_s50b_fix.rs` `adv_s50b_fix_ctrl_c_in_recovery_undo_prints_a_cause`: the stale pid only.
Searched `install/tests` for fixed pids: only this test.

## Out of scope
The install lock (S50c); anything outside install/.
