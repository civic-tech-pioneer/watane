Subsystem: dev
Effort: low
Depends on: none
Slice: S93 — dev's last misleading lines (adversary re-run BREACHED once on fix2).

## Goal
The red test `adv_s93_sibling_paths_build_folder_itself` (dev/src/builder.rs tests, commit 64ebaf3)
passes. In `sibling_paths`:
1. Only a strict descendant of the real build folder is a sibling (`<build>/`, `<build>/./`,
   `<build>/me/..` are never reported), so a token in the mod's own source cannot swallow its
   compile errors or the real sibling reports.
2. A path also ends at a bidi or invisible format character (at least U+200B-U+200F,
   U+202A-U+202E, U+2060-U+206F, U+FEFF), as it ends at a control character, so none reaches the
   `build.reads_outside` message raw (adversary RISK 1, settled by the foreman).
3. The build prefix counts only where a path starts: at the start of the text or after whitespace,
   a quote, a bracket, `(` or `:` — not inside a longer path like `/x<build>/other` (RISK 3).

## Acceptance
Unit tests in builder.rs (literals):
- AC1 the red test passes unedited.
- AC2 `"/c/build/other/wit\u{202e}gnp.a"` → exactly `[/c/build/other/wit]`.
- AC3 `"/x/c/build/other/wit"` → nothing; `"(/c/build/other/wit)"` → `[/c/build/other/wit]`.
- AC4 every existing `s93_*` builder test passes unedited.

## Spec
dev/TECHNICAL 6.4 step 6: "a strict descendant of the build folder".

## Interface
None.

## Files
dev/src/builder.rs, dev/TECHNICAL.md 6.4 only.

## Changed tests
None.

## Out of scope
pkg's escape set (RISK 2: the spec defines it; left as is).
