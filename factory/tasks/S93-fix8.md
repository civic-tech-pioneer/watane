Subsystem: dev
Effort: low
Depends on: none
Slice: S93 — dev's last misleading lines (adversary RISK on fix6, settled by the foreman).

## Goal
`generate_paths` (dev/src/builder.rs, S93-fix6) resolves each `generate!` `path:` literal from the
mod's source lexically only: join with `<copy>/logic`, normalize `.`/`..` as text, compare with the
copy (canonical and lexical forms, as now). It never calls `canonicalize` or any other filesystem
call on a literal, so a path the mod picks (e.g. macOS autofs `/net/<host>/...`) is never touched
during a build (dev AGENTS: no connection but HTTP to localhost). `sibling_paths` (paths rustc
already touched) is unchanged.

## Acceptance
- AC1 unit test: a literal `path: "/net/host/x/wit"` gives `/net/host/x/wit` (lexical) and the test
  shows no filesystem call is made (e.g. the function takes no I/O for literals: a pure helper over
  strings and the given copy paths, unit-tested with paths that do not exist).
- AC2 every existing builder test (unit and slow `builder::` with `--ignored`) passes unedited.

## Spec
dev/TECHNICAL 6.4 step 6: "resolved as text, never touched".

## Interface
None.

## Files
dev/src/builder.rs, dev/TECHNICAL.md 6.4 only.

## Changed tests
None.

## Out of scope
The stray-literal RISK (a generate! in a comment or dead file can replace the build's errors):
diagnostics only, left.
