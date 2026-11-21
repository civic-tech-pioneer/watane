Subsystem: host
Effort: medium
Depends on: S96-fix2
Slice: S96 — Stage A, end to end (host A1). Fix task for the third adversary pass.

## Goal
The three red tests of the adversary's third pass (`host/tests/it/adv_s96.rs`:
`adv_s96_open_in_flight_outlives_an_update_after_a_restart`,
`adv_s96_timer_set_across_remove_and_reinstall_carries_the_removed_origin`,
`adv_s96_cancel_in_flight_ends_the_stream_after_its_hand_off`) pass unchanged, and the catalog
lock is no longer held across a redb commit and its fsync. Root of two of them: an identity
alone does not tell two installs apart (19 Terms), so every authority check that a reinstall can
fool compares the install, not the identity.

## Acceptance
- AC1 (done by the S96-fix2 validator, ca75062; keep it) Install generations exist for every installed local name from `Catalog::open` on (number
  each name on load), so an effect never records `None` for an installed side; a recorded `None`
  (a side not installed at the call) fails the check at apply. `adv_s96_open_in_flight_outlives_an_update_after_a_restart` passes; the same for a hand-off after a restart (a test).
- AC2 The origin client a chain carries is (identity, install generation) in memory. Every check
  of "the origin client must still be there" (19 Terms: at call start after the lock, at the
  depth-0 commit of timer rows, at apply of Open/HandOff and every request of 19.6) compares the
  generation too. `adv_s96_timer_set_across_remove_and_reinstall_carries_the_removed_origin`
  passes; a test for the Open apply path with a reinstalled origin client if a fixture allows it
  cheaply (else say why not). The `timers` table keeps storing the identity only (7), and a
  firing's stored origin is checked by identity at firing as today (a reinstall after the row is
  rewritten by `remove` finds the row rewritten).
- AC3 `Effect::Cancel` and `Effect::Resume` carry the consumer the call acted for (as `Push` and
  `Close` carry `by`), and `Hub::apply` acts only when the stream's consumer is still that one.
  `adv_s96_cancel_in_flight_ends_the_stream_after_its_hand_off` passes; a test for a stale
  `resume` after a hand-off. Re-check the 19.6 "At apply" sentence on push/close/cancel/resume
  and make it true.
- AC4 The depth-0 commit no longer holds the catalog lock across `begin_write`, the redb commit
  or its fsync. Decision (foreman): serialize through redb instead. `remove` changes the catalog
  (and bumps the generation) before it opens the write transaction of its timer rewrite; the
  depth-0 commit reads the catalog (a short lock: origin identity, enabled, generation) inside its
  own write transaction, after `begin_write`. redb runs one write transaction at a time, so
  either the commit runs before the rewrite (which then fixes the row) or after the catalog
  change (and sees the origin gone). Write this in 19.6 Timers in place of the catalog-lock text.
  The earlier timer-origin tests (adv_s96 first and second pass) still pass.
- AC5 A timer of a mod in the `error` state waits (19.6: write it), and is armed again when the
  error clears (the reload after any catalog commit that changes a mod's error, not only after
  `updates`). A firing whose nested call fails `Disabled`/`NoMod` for a mod other than its own is
  a failure like any other (row deleted, as today for failures); only its own mod's
  `Disabled`/`NoMod` keeps the row. A unit or integration test each, if a fixture allows it
  cheaply; else say why not.
- AC6 All other host tests (default and the slow ones of touched files), clippy, fmt; the slow
  `agent_load::` still meets the stage A budgets (run it once).

## Spec
host TECHNICAL 19 Terms, 19.2 (Commit, Ends), 19.6 (At apply, Timers), 7, 4.4.

## Interface
No WIT, 19.9 or installed.json change.

## Files
host/src/agent/ (mod.rs, streams.rs, timers.rs), host/src/runtime/call.rs, host/src/state.rs,
host/src/catalog.rs, host/src/lib.rs, host/src/manage.rs, host/tests/it/adv_s96.rs (add tests;
never weaken the red ones), host/tests/fixtures/{src,mods}/advo*/, host/TECHNICAL.md (19, 19.2,
19.6).

## Changed tests
None; the three red tests go green unchanged.

## Out of scope
Throughput of `timer_ids` under the chains lock; host-wide budget counting of reservations.
