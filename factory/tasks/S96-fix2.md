Subsystem: host
Effort: medium
Depends on: S96-fix1
Slice: S96 — Stage A, end to end (host A1). Fix task for the second adversary pass.

## Goal
The four red tests of the adversary's second pass (`host/tests/it/adv_s96.rs`, the tests named
`adv_s96_*` committed in e42cedc, 5c3cac9, 0198bcf, a9d6852) pass unchanged. Root: a check made
when a call starts is trusted at its commit or apply. The rule to build (19.6 "At apply", 19
Terms): every check that a catalog change can falsify is made again where the effect lands,
under the same serialization as the catalog change.

## Acceptance
- AC1 `adv_s96_stream_opened_by_a_chain_in_flight_outlives_the_revoke` passes: when an `Open`
  (and every other stream effect of 19.2 Commit and every request of 19.6) is applied, the host
  checks the same conditions as `end_lost_streams` (consumer's `Call` grant and link through
  `via`, provider unchanged) and those of 19.6 "At apply"; a stream that fails them is applied
  and ended `gone` at once (both sides get `ended`), so the producer's next push is
  `unknown-stream`. Covers revoke, `manage.withdraw` and unbind alike.
- AC2 `adv_s96_stream_opened_by_a_chain_in_flight_outlives_an_update` passes: an effect records
  the package (hash or the catalog generation of each side) at its call; at apply a side that was
  updated, removed or reinstalled since ends the stream `gone`.
- AC3 `adv_s96_disable_while_a_firing_waits_loses_the_timer` passes: a firing that finds its mod
  disabled (or updated: `NoMod` after the lock) keeps its row (19.6: "its timers wait"; an
  updated mod keeps its timers) and is re-armed by the next `enable`/load; it is not deleted as a
  failure. Also close the race the adversary named: a firing that returns early re-arms (or
  leaves to `reload`) its row, so a `reload` that skipped it while it was in flight cannot lose
  it. A test: disable during the wait, enable, the timer fires once.
- AC4 `adv_s96_timer_set_by_a_call_running_across_the_remove_carries_the_removed_origin` passes:
  the origin client of the timer rows a chain writes is checked again at the depth-0 commit, in
  the same redb write transaction that writes the rows, serialized against `remove`'s rewrite
  (either the commit sees the origin gone and writes the timer's own mod, or the rewrite runs
  after the commit and rewrites it). Write this in 19.6 Timers.
- AC5 All other host tests pass (default and the slow ones of touched files), clippy, fmt; the
  slow `agent_load::` test still meets the stage A budgets.

## Spec
host TECHNICAL 19 Terms, 19.2 (Commit, Ends), 19.6 (At apply, Timers), 4.4, 18 Q46.

## Interface
No WIT or 19.9 change.

## Files
host/src/agent/ (mod.rs, streams.rs, timers.rs), host/src/runtime/call.rs, host/src/state.rs,
host/src/lib.rs, host/src/catalog.rs, host/src/manage.rs, host/tests/it/adv_s96.rs (add tests;
never weaken the red ones), host/tests/fixtures/{src,mods}/advo*/, host/TECHNICAL.md (19.2,
19.6).

## Changed tests
None; the four red tests go green unchanged.

## Out of scope
The throughput of `timer_ids` under the chains lock; host-wide budget counting of reservations
(a spec question for later); disable-and-enable of a side while an open is in flight (the stream
did not exist at the disable: it survives, write that in 19.2 Ends as decided).
