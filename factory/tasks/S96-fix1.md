Subsystem: host
Effort: medium
Depends on: S96-2
Slice: S96 — Stage A, end to end (host A1). Fix task for the adversary's breaches and risks.

## Goal
The adversary's three red tests in `host/tests/it/adv_s96.rs` pass, and three of its risks are
closed: a timer can never carry the authority of a removed mod, and a cancelled timer never
fires; stream and timer limits hold across concurrent chains; a lost grant ends the streams that
depended on it; timer firings cannot pile up threads.

## Acceptance
- AC1 `adv_s96::adv_s96_timer_set_in_ended_after_remove_carries_the_removed_origin` passes: the
  `ended`/`ready` notifications that `end_mod` queues (and every other host-made call: item calls,
  firings) check their origin client (19, Terms: installed and enabled with the identity carried)
  when the call starts, after the mod's call lock is taken; when it is gone, the call runs with the
  called mod's own identity as origin client (as a timer does, 19 Terms).
- AC2 `adv_s96::adv_s96_timer_firing_checks_its_origin_before_the_wait` passes: `timers.rs`
  `fire()` checks the origin after the lock is taken, not before.
- AC3 `adv_s96::adv_s96_timer_cancelled_while_the_firing_waits_still_fires` passes: the row is read
  again under the lock; a row cancelled or replaced (other due time) meanwhile is ignored. A
  firing in progress is never re-armed by `Host::enable` → `timers.reload` (one firing per row
  due); a test shows enable during a waiting firing fires once.
- AC4 Limits across concurrent chains (19.2 In flight, Byte budgets, Limits; 19.6 too-many;
  hand-off "once"): the checks at the call count every chain's queued effects, not only the
  current chain's (reserve at the call; release the reservation when the chain rolls back or
  applies). So the 4 MiB / 256-item in-flight limit, the origin client's 128 MiB, the host's
  512 MiB, the 256 live timers and the single hand-off can never be passed by two chains in
  flight at once. Decision, write it into 19.2 Commit ("The checks ... count what the chain has
  queued" becomes "count what every chain in flight has queued"). Test: two chains in flight (a
  slow front fixture, e.g. extend the adversary's `advo`, which sleeps by WASI without fuel) each
  pushing half-plus of a limit: the second gets `full`; two concurrent `timers.after` at 255 live:
  one gets `too-many`; two hand-offs of one stream: one gets `not-yours` or `unknown-stream`.
- AC5 Lost grant (19.2 Ends: "lost grant ... gone"): when the consumer's `Call` grant (or link) to
  the producer's interface through which a mod stream was opened is revoked, withdrawn
  (`manage.withdraw`) or unbound, that stream ends `gone` for both sides, at once (Q24). Decision:
  write into 19.2 which grant a mod stream depends on ("the consumer's `Call` grant of the link
  through which `open` ran; none for a page or a host stream until A2"), and add host 18 Q46
  naming it as decided by the foreman for the product owner to confirm. Test: revoke, then the
  producer's next push fails `unknown-stream` and both `ended` lines read `gone`.
- AC6 Timer firings run on a bounded set of workers (at most one waiting firing per mod at a
  time; due timers of one mod fire in due order on that one), not one thread per due timer. Test:
  256 due timers of one mod fire, all 256 recorded once, and the thread count of the process
  (Linux `/proc/self/task`, skip the assertion elsewhere with a reason) stays below 64 meanwhile.
- AC7 Every other host test passes (default and the slow ones of touched files); clippy, fmt.

## Spec
host TECHNICAL 19 (Terms, "The origin client must still be there"), 19.2 (Commit, In flight,
Byte budgets, Limits, Ends), 19.6 (Timers), 4.4, 18 (add Q46). The adversary's notes in the
red tests' comments.

## Interface
No change to WIT or to the 19.9 test API. `advo` may gain functions (it is a test fixture).

## Files
host/src/agent/ (mod.rs, streams.rs, timers.rs), host/src/runtime/call.rs, host/src/lib.rs,
host/src/catalog.rs, host/src/manage.rs (withdraw), host/tests/it/adv_s96.rs (do not weaken or
delete the three red tests; add yours beside them or in streams.rs/requests.rs),
host/tests/fixtures/src/advo/ and mods/advo/, host/TECHNICAL.md (19.2, 18 Q46).

## Changed tests
None expected; the three adversary tests go from red to green unchanged.

## Out of scope
Two local mods sharing `local/<slug>` (already a noted limit, 19 Terms). A2 to A5.
