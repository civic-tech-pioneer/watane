Subsystem: host
Effort: low
Depends on: S9-1, S9-2
Slice: S9 — Hostile pages and the program, checked.

## Goal
`hostile::ac3_call_refuses_wrong_method_origin_type_size_and_body` (test target `preview`)
fails about 1 run in 10: the 2 MiB `/call` body at `host/tests/hostile.rs` ~line 58 gets
`ConnectionReset` (BodyWrite) because the server answers 413 and closes before reqwest has
written the whole body. That server behaviour is correct (TECHNICAL 8.1/8.2, S9-1 AC4: "413 or
a closed connection, never 200"). Make the test accept both outcomes, the same way
`ac4_chunked_and_declared_oversized_bodies_never_change_state` does, and keep checking that the
state is unchanged afterwards.

## Acceptance
- AC1 `for i in $(seq 30); do cargo test -p watane-host --test preview -- hostile::ac3_call; done`
  passes 30 of 30.
- AC2 When the 413 arrives, it is still checked (`413 bad_request`, `Connection: close`); a
  reset is accepted only as a write or read error of that one request; no other change.
- AC3 `cargo test -p watane-host`, clippy, fmt pass.

## Spec
`host/TECHNICAL.md` 8.1, 8.2.

## Interface
None.

## Files
`host/tests/hostile.rs` only.

## Out of scope
Any change in `host/src/`.
