Subsystem: net
Effort: low
Depends on: S60-1
Slice: S60 — Two machines exchange heads and entries over `watane/1` (net N1)

## Goal

The net test suite leaves nothing in the temp dir. `tests/support/mod.rs` keeps the fixture's
`TempDir`s in a `static OnceLock<Fixture>`, which is never dropped, so each test binary that uses
`support::fixture()` leaves four signed-log dirs (about 4.3 MiB) behind on every run.

## Acceptance

AC1 `net/tests/regress_s60.rs::regress_s60_fixture_temp_dirs_removed` (commit c05073b) passes.
AC2 Nothing else changes: every other net test passes unchanged; clippy `-D warnings` and fmt clean.
Suggested fix: sign the fixture entries in temp dirs that are dropped as soon as signing is done,
and keep only the entries (bytes) in the `OnceLock`.

## Spec

net/TECHNICAL.md 7; brief S60-1 AC9 ("tests clean their temp dirs").

## Interface

none changes.

## Files

`net/tests/support/mod.rs` (and other `net/tests/**` only if the fixture's shape forces it).

## Changed tests

none (the red test is the target; do not weaken it).

## Out of scope

anything in `net/src`.
