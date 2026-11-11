Subsystem: dev
Effort: low
Depends on: none
Slice: S17 — The simplest tool to start with (user-sim rejection fix).

## Goal
The template guide's "Try the page without a browser" section says that the raw `/call` body
always needs `args`, with `"args":null` for a function without arguments. Today the page
section's "omit args or pass null" (true for `watane.call`) led an agent to omit `args` in curl,
and the host answered `bad_request: missing field args` (host TECHNICAL 8.2).

## Acceptance
- AC1 The red test `regress_s17_guide_curl_call_needs_args` in `dev/src/template.rs` passes with
  its `#[ignore]` removed and its body unchanged.
- AC2 Nothing else changes: the other template tests stay green (the guide stays within S17-2's
  line budget test), default `cargo test -p watane-dev`, clippy and fmt pass.

## Spec
host `TECHNICAL.md` 8.2 (`/call` body). `dev/template/AGENTS.md` "Try the page without a browser".

## Interface
None.

## Files
`dev/template/AGENTS.md` (that section only), `dev/src/template.rs` (remove the ignore line only).

## Out of scope
Every other note in `factory/harness-s17.md`.
