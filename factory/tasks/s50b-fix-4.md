Subsystem: install
Effort: medium
Depends on: s50b-fix-3
Slice: S50b-fix — install's tests pass on macOS

## Goal
User-sim's red test passes: a Ctrl-C (INT to the group) in the first second of a run, during
the checks of 3.4 steps 1 to 9, ends with exit 130 (3.8, INST-S-11) and names no false cause.
Today macOS `/bin/sh` (bash 3.2) sometimes carries on with the failed result of a command the
INT killed (`$(id -u)` at install.sh:~1065; the `printf | grep -Eq` of `check_version`
~414-417; the SHA256SUMS and archive checks) and prints "the tool id is missing" (exit 2),
"not a version: 0.1.0" (exit 1), "SHA256SUMS has more than one line", "could not be unpacked" or
"is not a watane archive" (exit 130). A 10-line toy script with the same two lines does it in
16 of 40 runs, so it is the shell, not watane. Nothing is damaged; only the reported cause is
wrong.

## Acceptance
- AC1 `install/tests/regress_s50b_fix.rs` `regress_s50b_fix_ctrl_c_during_the_checks_names_no_false_cause`
  (commit 06b894f) passes unchanged, three runs in a row on this Mac.
- AC2 Ways that keep the script POSIX and within 3.3's tools, in this order of preference:
  (a) do the checks that need no other program with builtins (`case` patterns for the version
  and the SHA256SUMS line; no pipe into `grep` where a `case` does it); (b) for the checks that
  must run a program (`id -u`, `tar`, `shasum`), let the INT trap set a flag, and have `die`
  (or the failure path of those checks) give the pending trap a chance to run first, so a run
  that saw INT ends 130 with no cause line. Find what works on bash 3.2 and dash; say which in
  install/NOTES.md.
- AC3 Every other test passes; `shellcheck -s sh install.sh package.sh` exits 0; the error
  texts of 3.9 do not change for a real failure (a missing tool, a bad version, a bad checksum
  still print their line and code).

## Spec
install/TECHNICAL 3.4 steps 1 to 9, 3.8 (signals), 3.9 (exit codes and texts). Add to 3.8 the
rule: an INT, HUP, TERM or PIPE before the swap ends the run with its code and no cause line.

## Interface
none.

## Files
`install/install.sh`, `install/tests/*`, `install/TECHNICAL.md`, `install/NOTES.md`.

## Changed tests
none: the red test is the acceptance and stays unchanged. Searched `install/tests/*.rs` for the
texts above: the tests that assert them (missing tool, bad version, checksum) are real failures
and keep their assertions.

## Out of scope
The hint past the commit point, `--status` cleaning up, the two rustup URLs, the `^D` newline
(user-sim "felt wrong" notes: for a later slice).
If no way makes AC1 reliable on bash 3.2, stop and answer QUESTIONS with what you measured.
