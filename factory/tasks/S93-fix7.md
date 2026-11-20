Subsystem: pkg
Effort: low
Depends on: none
Slice: S93 — dev's last misleading lines (user-sim REJECTED: caret at the byte column).

## Goal
The red test `world::tests::regress_s93_caret_under_the_char_column` (pkg/src/world.rs, d21be0a)
passes unedited: in the `wit` message's source snippet, the `^`/`^---` marker line's leading spaces
count characters of the snippet line as it is shown (after escaping), so the caret stands under the
char column. Rewrite only wit-parser's marker line (the `|` line after the snippet), in the same
place as the column fix (`fix_message`), before or after `escape_message` as the test needs.

## Acceptance
- AC1 the red test passes unedited.
- AC2 every existing `world::tests` test passes unedited.

## Spec
pkg/TECHNICAL.md section 4 step 2: "the caret stands under the character column".

## Interface
None.

## Files
pkg/src/world.rs, pkg/TECHNICAL.md section 4 only (S91 owns the rest of pkg/src).

## Changed tests
None.

## Out of scope
dev.
