Subsystem: dev
Effort: low
Depends on: none
Slice: S93 — dev's last misleading lines (adversary re-run on fix3 BREACHED: more format chars).

## Goal
The red test `adv_s93_sibling_paths_other_format_chars` (dev/src/builder.rs tests, commit 2aea9d2)
passes. Stop listing characters: in `sibling_paths`, the part of a path after the build prefix ends
at the first character that is not ASCII graphic (`!c.is_ascii_graphic()`: whitespace, every
control, every non-ASCII char) or is in PATH_EDGES. A slug is ASCII (pkg slug grammar), so
`<build>/<slug>` is never cut, and a cut later only names an ancestor folder of the file read,
which is still the sibling. The hand-written U+200B.. list goes. The prefix itself (the cache path,
which may hold a space or non-ASCII) is matched literally as today.

## Acceptance
- AC1 the red test passes unedited.
- AC2 `"/c/build/other/wit/é.wit"` → exactly `[/c/build/other/wit]` (unit test).
- AC3 every existing `s93_*` and `adv_s93_*` builder test passes unedited.

## Spec
dev/TECHNICAL 6.4 step 6: the end rule becomes "at the first character that is not printable
ASCII, or at a quote or bracket".

## Interface
None.

## Files
dev/src/builder.rs, dev/TECHNICAL.md 6.4 only.

## Changed tests
None (AC2 is new; check that `s93_fix3_*` does not assert a non-ASCII path survives).

## Out of scope
Path starts after other characters (`//c/build`, `=`, `,`): adversary RISK, left; rustc writes
those paths with a space or quote before them.
