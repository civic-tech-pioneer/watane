Subsystem: dev
Effort: low
Depends on: none
Slice: S56 — Leftover sweep (dev messages): what a maker's coding agent reads at the moment something
has already gone wrong names the real cause and gives a `next` it can run.

## Goal
S56-3 made the `usage.args` message clap's first line without `error: `. For a missing required
argument that line is `the following required arguments were not provided:` and the argument's name
(`<SLUG>`, `<PROVIDER>`) is on the next, indented line, now cut off. The message must keep the
argument: take clap's text up to (not including) the blank line before `Usage:` (or before `For more
information`), drop the leading `error: `, and join its lines with single spaces after trimming each
(`the following required arguments were not provided: <PROVIDER>`). Single-line errors are unchanged;
the "no command given" case is unchanged.

## Acceptance
- AC1 the red test `regress_s56_missing_argument_message_names_the_argument` in dev/tests/envelope.rs
  passes (do not change it).
- AC2 `--json add` gives exactly `the following required arguments were not provided: <PROVIDER>`
  (assert in the same file or a unit test in main.rs if the text is built by a function there).
- AC3 nothing else changes: `cargo test -p watane-dev --test envelope` and the lib tests pass; clippy
  `-D warnings` and fmt pass.

## Spec
dev/TECHNICAL.md 2.2 row `usage.args` and 2.3: say the message is clap's error without `error: `
and without its usage and help lines, on one line.

## Interface
none

## Files
dev/src/main.rs, dev/tests/envelope.rs (new assertion only), dev/TECHNICAL.md (those rows)

## Changed tests
none besides the red test, which must go green unchanged. If envelope.rs's spec-marker test
(`ac6_s39_specs_cover_paths_derivation_fake_and_argument_errors`) pins the TECHNICAL text, update it
to the new text.

## Out of scope
every other message
