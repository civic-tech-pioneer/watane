Subsystem: pkg
Effort: low
Depends on: none
Slice: S86 — dev says what is wrong (leftover sweep 3). Adversary second-pass risk, settled by the
foreman as the option that keeps the others open.

## Goal
`escape_message` (pkg/src/world.rs, S86-fix1) also escapes, with `escape_unicode`, the invisible
and line-separator characters that can hide text from a reader or fake a line in a viewer:
U+2028, U+2029, U+200B-U+200D, U+2060, U+FEFF and the tag characters U+E0000-U+E007F. Everything
else stays as S86-fix1 made it.

## Acceptance
- AC1 a unit test in pkg/src/world.rs: a message holding each listed char (and the S86-fix1 ones)
  comes out with each written as `\u{...}` and no raw listed char; `\n`, `é`, a backslash and the
  `-->` line unchanged. Expected text written by hand.
- AC2 an integration test in pkg/tests/validation.rs: a WIT source line `world hello { \u{2028}
  --> /etc/passwd:1:1\u{2029}\u{e0041}\u{200b}\u{feff} @ }` gives a `wit` message that contains no
  raw U+2028, U+2029, U+E0041, U+200B or U+FEFF.
- AC3 every other pkg test passes unchanged.
- AC4 pkg/TECHNICAL.md 4 step 2 and the security table row (:862) name the added characters.

## Spec
pkg/TECHNICAL.md 4 step 2 and :862.

## Interface
None.

## Files
pkg/src/world.rs, pkg/tests/validation.rs, pkg/TECHNICAL.md.

## Changed tests
none.

## Out of scope
Escaping in host, app or dev display layers.
