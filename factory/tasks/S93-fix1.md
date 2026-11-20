Subsystem: pkg
Effort: medium
Depends on: none
Slice: S93 — dev's last misleading lines (adversary BREACHED once).

## Goal
The red test `adv_s93_wit_message_other_invisible_chars` (pkg/src/world.rs tests, commit 0e296f9)
passes. `escape_message` stops listing invisible characters one slice at a time: it escapes, besides
every control character other than `\n`, every character that is Unicode
Default_Ignorable_Code_Point or general category Cf, plus the Hangul fillers (U+115F, U+1160,
U+3164, U+FFA0) and the line/paragraph separators already listed. Use a hand-written range table in
world.rs (no new dependency) taken from the Unicode DerivedCoreProperties Default_Ignorable_Code_Point
list; cite its Unicode version in a comment.

## Acceptance
- AC1 the red test passes unchanged.
- AC2 the existing `ac1_s86_*` and `ac3_s93_*` escape tests pass unchanged.
- AC3 a unit test: a visible non-ASCII text (`é€日本`, an emoji, Arabic letters) passes through
  unescaped.

## Spec
pkg/TECHNICAL.md section 4 step 2 and its security row: replace the parenthesised list with the
rule above (the set, not a list of single characters).

## Interface
None.

## Files
pkg/src/world.rs, pkg/TECHNICAL.md section 4 only (S91 owns the rest of pkg/src).

## Changed tests
None; the red test is the new expectation.

## Out of scope
dev's text output escaping (a separate question).
