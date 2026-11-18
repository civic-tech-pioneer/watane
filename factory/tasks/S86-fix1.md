Subsystem: pkg
Effort: medium
Depends on: none
Slice: S86 — dev says what is wrong (leftover sweep 3). Adversary breach fix.

## Goal
A `Rule::Wit` violation's message never carries a raw control character or bidi override from the
untrusted WIT source. S86-1 renders wit-parser's error with its source map, which copies the
offending source line verbatim into `Violation.message` (pkg/src/world.rs). After this task the
message is escaped before `cap_message`: every char that is a control character other than the
`\n` separating the rendered lines (so `\r`, `\t`, ESC, BEL, DEL, C1 controls) and every bidi
format character (U+061C, U+200E, U+200F, U+202A-U+202E, U+2066-U+2069) is written as its
`char::escape_debug`/`escape_unicode` form (`\u{1b}`, `\r`, `\u{202e}`). The ` --> path:line:col`
line and everything else stay as they are, so dev's `wit_location` still reads the location.

## Acceptance
- AC1 the red test pkg/tests/validation.rs `adv_s86_wit_message_carries_no_control_characters`
  (commit fe3a786) passes, unchanged.
- AC2 every other pkg test passes unchanged (including the S86-1 `-->` tests and
  `ac3_rules_wit_message_is_first_2000_bytes`; if its oracle comparison breaks only because the
  oracle text contains a char now escaped, say so and do not weaken it otherwise).
- AC3 pkg/TECHNICAL.md section 9 security table row "A malformed input cannot stop the host" (:862)
  and section 4 step 2 say the `wit` message escapes control and bidi characters of the source.

## Spec
pkg/TECHNICAL.md 4 step 2, the security table at :862.

## Interface
None changes. The `-->` line format stays (S86-3 depends on it).

## Files
pkg/src/world.rs, pkg/TECHNICAL.md.

## Changed tests
none (searched pkg/tests/validation.rs and pkg/src/world.rs tests: the S86 tests use ASCII sources).

## Out of scope
Escaping in host, app or dev display layers.
