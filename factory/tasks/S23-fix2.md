Subsystem: dev
Effort: low
Depends on: none
Slice: S23 — The maker's agent stumbles less (slow test after S23-2).

## Goal
The slow test `ac7a_successful_serve_gives_own_page_or_null` (dev/tests/serve.rs) expects the old
`serve` `next` ("Give the person this page: <url>"). Since S23-2 a new preview (`reused: false`)
adds the MCP setup sentence (TECHNICAL 2.4 serve). Update the test's expectation to the
specified text for a first serve (reused:false), without calling `serve_next` to build it.

## Acceptance
- AC1 `cd dev && cargo build -p watane-host && WATANE_BIN=$PWD/../target/debug/watane cargo test -p watane-dev --test serve -- --include-ignored ac7a` passes. Run only this test and the default tests; product code does not change.
- AC2 Any other test in dev/tests/*.rs that pins the old first-serve `next` is updated the same way (grep "Give the person this page").

## Spec
dev TECHNICAL 2.4 (serve next).

## Interface
None.

## Files
dev/tests/serve.rs and other dev/tests/*.rs only as AC2 needs.

## Out of scope
Product code. Do not edit dev/AGENTS.md.
