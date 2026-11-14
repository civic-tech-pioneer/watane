Subsystem: pkg
Effort: low
Depends on: S57-1
Slice: S57 — pkg, log and dist say what is wrong.

## Goal
User-sim found a second mid-read window S57-1 did not close: `open::entries` (pkg/src/open.rs,
~91-105) lists a directory and then `statat`s each entry; an entry removed between the two makes
the `?` return `NotFound`, which `links_below` and `inspect` in folder.rs map to `ModError::Io` at
the parent (`<mod>/ui: No such file or directory`), a text that blames a folder that is there.

Decided here: in the folder.rs walks (`links_below`, `inspect`), a `NotFound` (or `NotADirectory`)
from `open::entries` of a listed directory gives the S57-1 `path` violation
`refusal::CHANGED` ("changed while the mod was read; read it again") with file = that directory's
relative path, not `ModError::Io`. Other I/O errors stay `Io`. `open::entries` itself is unchanged
unless a small change there is simpler (e.g. returning the kind unchanged is enough).

## Acceptance
- AC1 `pkg/tests/s57.rs::regress_s57_entry_gone_between_list_and_stat` (red at c7e7330) passes.
- AC2 Nothing else changes: `cargo test -p watane-pkg -- --include-ignored --test-threads=1`,
  clippy `-D warnings`, fmt pass. Run the AC1 test 5 times; it passes each time.

## Spec
pkg/TECHNICAL.md 6.4: the mid-read change sentence S57-1 added also covers an entry gone between
the listing and its stat.

## Interface
None.

## Files
`pkg/src/folder.rs`, `pkg/src/open.rs` (only if needed), `pkg/TECHNICAL.md`.

## Changed tests
None (the red test is the new one; do not change it).

## Out of scope
add_provider's I/O texts under a race (`File exists`), pack's same window (pack_error), dist, log.
