Subsystem: pkg
Effort: low
Depends on: none
Slice: S91 — a maker or a mod hits an edge and gets a named cause, not raw I/O text.

## Goal
In `add_provider`, a `.wit` file that `files()` listed and that is a directory (or under a
directory that became a file) when it is read gives rule `path` with a named cause, not
`[limit] expected a regular file`.

## Acceptance
AC1. The red test `pkg/tests/it/s91.rs` `regress_s91_add_provider_wit_file_swapped_for_directory`
     (commit 1f5de93) passes: every refusal under the swap is rule `path`, none `Io`.
AC2. A unit test in pkg/src/add.rs (deterministic: the existing `hook` or a fixture state)
     shows a listed `.wit` that is a directory at read time gives rule `path` with the
     `CHANGED` text or `refusal::DIRECTORY` ("expected a regular file, found a directory");
     pick the one TECHNICAL 4.8 then states.
AC3. Nothing else changes: `Mod::read`'s `read_file` callers keep their causes (folder.rs's
     "expected a regular file" under the caller's rule for a top-level file part stays as is).

## Spec
pkg/TECHNICAL.md 4.8 (the races paragraph S91-1 wrote); 4.7 for CHANGED.

## Interface
None.

## Files
pkg/src/add.rs, pkg/src/folder.rs (only if the mapping lives there, without changing
`Mod::read`'s causes), pkg/TECHNICAL.md 4.8, pkg/tests/it/s91.rs only to keep it compiling.
NOT pkg/src/world.rs.

## Changed tests
None besides the red test turning green.

## Out of scope
`watane-dev add`'s "is not a mod folder" for an invalid provider (dev); parent-vs-entry path
in pack's listing.
