Subsystem: net
Effort: low
Depends on: none
Slice: S87 — a light machine reports a full machine's changed bytes again

## Goal
The adversary found that since S87-1 every failed `need_blob` adds an `Event::Unreachable`, so
twenty failed fetches (one `confirm_own_hint` with over 20 own releases is enough) push a
machine's only `refused` line out of Machines' 20 problems. After this task `need_blob` emits
its `Event::Unreachable` at most once for a machine until a package fetch from that machine
succeeds; the returned error is unchanged every time.

## Acceptance
- AC1 Three `need_blob` calls for three different packages no connected full machine has: three
  `Unreachable` errors as today, and exactly one `Event::Unreachable` (for the first) on a
  receiver taken before the calls.
- AC2 After a `need_blob` from that machine succeeds, the next failing `need_blob` emits one
  `Event::Unreachable` again.
- AC3 S87-1's tests (`ac1_s87_...`, `ac2_s87_...`, `ac3_s87_...` in net/tests/it/light.rs) still
  pass; adapt `ac2_s87` only if its loop reuses one light node across iterations (say so).
- AC4 Every net test, clippy, fmt.

## Spec
net/TECHNICAL.md 5.9 `need_blob` (updated in this brief's commit).

## Interface
None new. The event's node and cause stay as S87-1 made them.

## Files
net/src/light.rs, net/src/lib.rs or net/src/sync.rs only for the per-machine flag in `Core`
(keep it next to the keeper's state, smallest form: a set of node keys), net/tests/it/light.rs.

## Changed tests
none expected (see AC3).

## Out of scope
The app's problems list; a local I/O error naming the remote machine (recorded for the PO).
