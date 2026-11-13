Subsystem: pkg
Effort: low
Depends on: S33-1
Slice: S33 — The agent's texts, second pass (user-sim rejection 1).

## Goal
A hand-made `wit/deps/<own slug>/` that declares the mod's own package at another version (mod
`mods:s@0.2.0`, copy `mods:s@0.1.0`) still fails with the WIT error "invalid, unversioned or
repeated package". pkg/src/world.rs compares `group.main.name == own_name` including the version;
the own-package check must compare namespace and name only.

## Acceptance
- AC1 The red test `pkg/tests/validation.rs` `regress_s33_own_copy_other_version` passes.
- AC2 Nothing else changes: `cargo test -p watane-pkg`, clippy `--all-targets --all-features -D
  warnings`, `cargo fmt --check` pass.

## Spec
pkg `TECHNICAL.md` 4.7. Do not edit spec documents.

## Interface
none (same messages as S33-1).

## Files
`pkg/src/world.rs`, `pkg/src/folder.rs` if needed.

## Changed tests
none.

## Out of scope
everything else.
