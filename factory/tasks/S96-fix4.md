Subsystem: host
Effort: medium
Depends on: S96-fix3
Slice: S96 — Stage A, end to end (host A1). Fix task for the fourth adversary pass.

## Goal
`adv_s96::adv_s96_install_in_the_remove_gap_inherits_the_removed_origin` (red, from the fourth
pass) passes unchanged. Root: `Host::remove` frees the local name in the catalog, then deletes
the folder, and only then ends the mod's streams, rewrites its timers and reloads; an error or an
install in that gap skips the tail. Decision (foreman, inside host, cheap to undo): the tail that
protects authority runs before anything that can fail or be raced, and never depends on the
folder delete.

## Acceptance
- AC1 The red test passes. In `remove`, `hub.end_mod`, `state.timers_removed` (the rewrite of
  the timers that carry the removed mod, 4.4, 19.6) and the timer-row delete of the removed mod
  run before the catalog frees the name, or in the same step, so no install can see the name free
  while a row or a stream still carries the removed install. The folder delete comes last; its
  failure is reported (as today) but leaves no authority behind: rows rewritten, streams ended,
  timers reloaded. A test: a folder delete that fails (e.g. a read-only subfolder, Linux only,
  skipped elsewhere with a reason) still leaves the rows rewritten and the streams ended.
- AC2 An install of a local name that a `remove` of the same name has not finished waits for it
  or fails `busy` (pick the smaller change; write it in 4.4). The race of the red test cannot
  rename a half-deleted folder into `.old`.
- AC3 Cancel/resume ABA (19.6 "At apply": "a hand-off in between makes a stale cancel or resume do
  nothing"): the hub stream carries a hand-off count; `by` on `Cancel`/`Resume` records it at the
  call and `Hub::apply` compares it. A unit test in agent/streams.rs (A to X to A by the hub API,
  then the stale cancel does nothing), since no fixture hands a stream back.
- AC4 The no-timer-sink delete (19.6) re-checks the exports in the same step that deletes the
  row (or deletes only if the install generation it read is still current).
- AC5 Flake: `adv_s96_timer_set_across_remove_and_reinstall_carries_the_removed_origin` asserts
  that remove plus reinstall happen within 280 ms of a spin; under load (2 of 3 runs while
  cargo-mutants compiled) that precondition fails. CI runners (macOS, 3 cores) are loaded. Make
  the race window reliable without weakening what the test asserts: e.g. make the spin a parameter
  the test sets long (several seconds), or gate on a signal from the running call instead of a
  clock. Same for any other adv_s96 test whose precondition is a time window; run adv_s96:: 5
  times while `cargo build --release -p watane-host` runs in parallel, all green.
- AC6 An `Unchanged` reinstall of a mod in the error state (`Catalog::commit`) replaces its entry
  with error `None` but does not insert its component, so its firing then deletes the timer row
  as "no timer-sink" (found by the S96-fix3 validator, older than this slice). Fix: an entry whose
  error clears gets its component loaded like a changed one. A test (the AC5 test of S96-fix3,
  `ac5_a_timer_of_a_mod_in_error_waits_until_the_error_clears`, with the error cleared by an
  unchanged reinstall instead of an update).
- AC7 All other host tests (default and slow ones of touched files), clippy, fmt.

## Spec
host TECHNICAL 4.4 (remove), 19 Terms, 19.6 (At apply, Timers).

## Interface
No WIT, 19.9 or installed.json change.

## Files
host/src/lib.rs, host/src/catalog.rs, host/src/state.rs, host/src/agent/ (mod.rs, streams.rs,
timers.rs), host/tests/it/adv_s96.rs (add tests; never weaken the red one), host/TECHNICAL.md
(4.4, 19.6).

## Changed tests
None expected.

## Out of scope
Everything not named above.
