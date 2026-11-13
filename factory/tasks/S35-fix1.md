Subsystem: dist
Effort: low
Depends on: none
Slice: S35 — Eve's dist finds Carol's release through Dave's listing (dist S7)
## Goal
A repeated `fetch_for` of a provider release that is already cached gives the same
`Fetched.link` as the first fetch: the consumer's (most trusted) base, not a dead base that a
chosen index's listing put on Carol's listed record. The app follows that link after an install.

## Acceptance
- AC1 `dist/tests/provider.rs::regress_s35_cache_hit_keeps_consumer_base` passes (commit
  `51f2080`; do not change the test).
- AC2 Nothing else changes: every other dist test passes (run with `RUST_TEST_THREADS=1` if
  parallel timeouts show; they predate this slice); clippy and fmt clean.

## Spec
`dist/TECHNICAL.md` 5.7 step 8 (amended for this fix: on a cache hit of a channel that is neither
followed nor chosen, the base is the first provider base in 5.11 step 2 trust order, else the
listed record's front base, else `None`), 5.11 steps 2 and 5.

## Interface
No change.

## Files
`dist/src/fetch.rs` (and another `dist/src/*.rs` only if the hint-base helper lives there).

## Changed tests
None.

## Out of scope
The `NotFollowed` text for skipped local bases; reading fewer bases after a trusted one served.
