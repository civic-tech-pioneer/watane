Subsystem: dev
Effort: medium
Depends on: none
Slice: S47 — J5 passes again (dev fix)

## Goal

Bob made a mod with `new --from` from a package that has `logic.wasm` but no
`logic/` source (S47-1 keeps that file: it is the mod's only logic). Today
`watane-dev test` (and `serve`/`publish`, which share the pipeline in
dev/src/cli.rs around `builder::build`) deletes that `logic.wasm`
(`builder::build` removes it first, then `prepare` fails on the missing
`logic/Cargo.toml`) and reports `internal` "No such file or directory". After
this task a mod with `logic.wasm` and no `logic/Cargo.toml` is a prebuilt mod:
the pipeline skips the build and the unit tests (no Rust source to compile) and
runs the contract tests, the preview and the publish against the kept
`logic.wasm`, which it never removes. A mod with `logic/Cargo.toml` is
unchanged.

## Acceptance

AC1 The red test `builder::regress_s47_test_keeps_packaged_logic_without_source`
    (dev/tests/builder.rs, commit 8092d94) passes unchanged: `logic.wasm` keeps
    its bytes and no error is `internal`.
AC2 In that run, `result` has `check` and `contract`, and no `build` or `unit`
    (as a page-only mod has none), and the command exits 0 when the contract
    passes.
AC3 Every existing dev test stays green, slow ones too
    (`cargo test -- --include-ignored`).

## Spec

dev/TECHNICAL.md section 6 (the builder) and 7.1: add one rule, in the section
that owns the pipeline's order: "A mod with `logic.wasm` and no
`logic/Cargo.toml` is prebuilt: dev skips 6 and 7 and never removes its
`logic.wasm`." Map the red test in 15.2 next to DEV-S-35's row or as a note
under the table (whichever the map test accepts).

## Interface

None outside dev.

## Files

dev/src/cli.rs, dev/src/builder.rs (only if the guard belongs there),
dev/TECHNICAL.md, dev/tests/builder.rs (only to add AC2's checks to the red
test, not to weaken it).

## Changed tests

none (searched dev/tests for `logic.wasm` and the pipeline's `build` key: the
existing tests all use mods with `logic/Cargo.toml` or page-only mods).

## Out of scope

What `publish --push` does with a prebuilt mod (S42). The `next` text of
`new --from`.
