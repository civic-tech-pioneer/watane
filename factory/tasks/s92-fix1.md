Subsystem: node
Effort: low
Depends on: none
Slice: S92 — Blame the right machine: when Alice's install fails, she is told the real cause and whose it is.

## Goal
user-sim REJECTED S92 on one point: `DistHoldings::store_blob` (node/src/holdings.rs:130-134)
turns dist's error into text with `e.to_string()`, and `DistError::Io`'s Display is only its path,
so the OS cause is lost. On a full disk Alice reads "package <hex8> was not stored: <a temp path>"
with no "No space left on device". After this task the holder's text carries the whole cause chain
(each `source()` joined with ": "), in `store_blob` and in `ingest`'s `Ingested::Failed` (same
`to_string()` there, holdings.rs:126).

## Acceptance
- AC1 The red test `holdings::tests::regress_s92_store_blob_error_keeps_the_os_cause`
  (node/src/holdings.rs) passes, unchanged.
- AC2 Nothing else changes: every node test green (`cargo test -- --include-ignored`), clippy
  `-D warnings`, `cargo +nightly fmt --check`.

## Spec
node/TECHNICAL.md 4.1 (`DistHoldings`): add one line that a dist error is given as its cause chain
(text of each source, joined by ": ").

## Interface
None. `Holdings::store_blob -> Result<(), String>` unchanged.

## Files
`node/src/holdings.rs` (a small chain helper may live there or in node/src/convert.rs),
`node/TECHNICAL.md`.

## Changed tests
Searched node tests for `Ingested::Failed(` texts: `ac4_before_its_dist_is_set...` asserts "the home
is opening" (not a dist error; unchanged). None changes.

## Out of scope
Everything else user-sim noted (server off reads "not followed"; "unreachable" for a machine that
answered; the bare top line of `DistError::Io`; dist's `BadPackage` for a local unpack failure):
leftovers in the delivery.
