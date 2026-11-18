Subsystem: dev
Effort: low
Depends on: none
Slice: S86 — dev says what is wrong (leftover sweep 3). Adversary risk fix.

## Goal
`search_command` (dev/src/cli.rs ~578) builds the search rerun used in `next` and fixes. A word
that starts with `-` (given after `--`, e.g. `watane-dev search -- --home=/x`) is pasted back as
`watane-dev search --json '--home=/x'`, which clap reads as a flag. After: the rerun puts the
filters (`--publisher`, `--interface`, `--home`) before the words and emits ` --` before the
words whenever any word starts with `-`; otherwise the text stays exactly as today.

## Acceptance
- AC1 a cli.rs unit test: `search_command(["--home=/x"], None, None, "")` →
  `watane-dev search --json -- '--home=/x'` (exact, with the filter order you produce), and
  parsing that line's argv with `Cli::try_parse_from` gives `text == ["--home=/x"]` and no home.
- AC2 words without a leading `-` give exactly today's text (every existing search `next`
  assertion passes unchanged).

## Spec
dev/TECHNICAL.md 2.4 (search `next`). Add one sentence there for the `--`.

## Interface
None.

## Files
dev/src/cli.rs, dev/TECHNICAL.md.

## Changed tests
none expected (dev/tests/search.rs words never start with `-`); list any you change.

## Out of scope
Everything else.
