Subsystem: install
Effort: medium
Depends on: none
Slice: S89 — Alice installs, interrupts, checks and uninstalls watane, and every line she reads is true and in her own terms

## Goal
The adversary's red test passes, and three of its RISK items are closed in the way that keeps
every other option open: (1) "committed" for the interruption hint is decided by the state undo
already uses (the receipt's `.new.<pid>` is gone), not by a flag set one command after the `mv`;
(2) `say` hides printf's own error text only when a signal (PIPE) then ends the run; any other
write error (full disk, closed stdout, an inherited ignored SIGPIPE) still ends the run as it did
before S89 (non-zero, not silently carrying on); (3) `remove_created` re-resolves each folder
(`cd ... && pwd -P`) right before its `rmdir` and removes it only when it is still a real folder
inside the home (3.4.1's rule), so a link swapped in after `note_created` is never followed out of
`$HOME`; (4) the "install it later" curl line passes the URL through `shell_quote` when it is not exactly
`https://sh.rustup.rs` (the default line stays byte-identical, INST-S-3).

## Acceptance
AC1. `adv_s89::adv_s89_signal_during_the_receipt_move_keeps_the_install_but_prints_no_hint` passes
     (do not change the test).
AC2. `say` to stdout redirected to `/dev/full` (a run with `--status` and a receipt): exit is
     non-zero and not 0; a test pins the exit code you choose and TECHNICAL 3.9 names it.
     `--status | head -1` stays quiet (s89 AC3 tests unchanged).
AC3. A test: fresh install, a fake program swaps the run-created `~/.local/share/watane-dev`
     (empty) for a link to an empty folder outside the home before cleanup; after an interrupted
     run the outside folder still exists.
AC4. With `WATANE_RUSTUP_URL='http://x/a b;c'` the "install it later" line holds the URL quoted
     so that pasting it runs one curl; unset, the line is unchanged.
AC5. Every s89, script, s50c, regress_s50c, check_signals, status_audit test still passes (the
     three known environment failures aside); shellcheck clean.

## Spec
install/TECHNICAL.md 3.4.1, 3.7, 3.8, 3.9, IN-44..IN-48; edit them for each decision.

## Interface
none.

## Files
install/install.sh, install/TECHNICAL.md, install/FUNCTIONAL.md, install/NOTES.md,
install/tests/it/s89.rs, install/tests/it/status_audit.rs (only if a new checked helper must be listed).

## Changed tests
none expected; adv_s89.rs must not change.

## Out of scope
Ordering of rustup's stderr against watane's frame lines; macOS sh runs (PO).
