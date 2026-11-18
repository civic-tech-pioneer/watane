Subsystem: install
Effort: low
Depends on: s50b-fix-5
Slice: S50b-fix — install's tests pass on macOS

## Goal
The adversary's two red tests of commit 17fb839 (`install/tests/adv_s50b_fix.rs`) pass
unchanged. macOS `/bin/sh` (bash 3.2) can lose the parent's INT when a child that got the same
Ctrl-C exits 130; s50b-fix-5 recovers it only where a failure path calls `check_signal "$status"`.
Wherever install.sh runs a program and drops its status, a Ctrl-C is lost and the run goes on
(exit 0). Every such site gets `check_signal` with that status, right after the program.

## Acceptance
- AC1 `adv_s50b_fix_ctrl_c_in_toolchain_detection_finishes_with_success` and
  `adv_s50b_fix_ctrl_c_in_xattr_reaches_the_toolchain_step` pass unchanged.
- AC2 Each site that runs a program and drops or overwrites its status passes that status to
  `check_signal` before going on. The adversary found: `detect_toolchain` (both `$(...)`, the
  second with `|| dt_sr=""`), `do_install`'s `xattr ... || true`, `check_platform`'s `uname`,
  `sysctl`, `sw_vers`, `read_answer`'s `$(dd | od)`, `final_output`'s `xcode-select -p`, the bare
  `mkdir "$TMP/x"`, and the second `detect_toolchain` after `rustup_step` (which then prints
  "toolchain: still incomplete" with exit 6 on a signalled run). Search for every other
  `$(`, `|| true`, `|| x=""` and status-dropping `if`/`case` on a program, and treat it the same.
  A test for each kind of site the two red tests do not cover (one per kind is enough: a
  substitution, a `|| true`, a pipeline), using a fake that sends INT to the group and exits 130.
- AC3 Real failures keep their text and code; every other test passes; `shellcheck -s sh
  install.sh package.sh` exits 0.

## Spec
install/TECHNICAL 3.8: add "every program whose status the script does not otherwise act on
passes it to the signal check, so a signal the shell lost still ends the run". Also record these
foreman decisions in 3.8: a real exit of 129/130/141/143 from rustup or rustup-init is treated as
that signal (no "toolchain step failed" line); a signal during the final cleanup may leave the
temporary folder; a signal while a question waits is acted on when the read returns. Record in
install/NOTES.md.

## Interface
none.

## Files
`install/install.sh`, `install/tests/*`, `install/TECHNICAL.md`, `install/NOTES.md`.

## Changed tests
none.

## Out of scope
Anything outside `install/`.
