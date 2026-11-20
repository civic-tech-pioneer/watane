Subsystem: dev
Effort: low
Depends on: none
Slice: S93 — dev's last misleading lines (user-sim REJECTED twice on the no_source fix; settled
by the foreman).

## Goal
The red test `build_on::regress_s93_no_source_fix_without_slug_runs_as_written`
(dev/tests/it/build_on.rs, f201174) passes unedited. Decision: when `new --from` of a release
without source was given no slug, the fix's new mod cannot take the release's slug (it would depend
on itself: `mod.self_dependency`). The fix names the new mod `my-<release slug>` (cut to the slug
grammar's 40 bytes, pkg/src/names.rs, without a trailing `-`), in the folder `<cwd>/my-<release
slug>` (or the given `--dir` when there was one): `... make a new mod that depends on this one:
watane-dev new my-<slug> --dir <dest> && watane-dev add --release <ref> --dir <dest>[ --home <h>]`.
With a slug given, nothing changes (its slug differs from the release's unless the maker chose the
same one; if equal, use `my-<slug>` too).

## Acceptance
- AC1 the red test passes unedited.
- AC2 `build_on::regress_s93_no_source_fix_runs_as_written` and `ac2_no_source_writes_nothing`
  pass (update the latter's literal only if its case has no slug, and say so).
- AC3 unit test: the release slug `a` x 40 gives a 40-byte `my-aaa…` slug with no trailing dash.

## Spec
dev/TECHNICAL.md 2.2 `usage.no_source` row and 10.2 (the no-source step).

## Interface
None.

## Files
dev/src/build_on.rs (and cli.rs only if the slug is chosen there), dev/tests/it/build_on.rs for
changed existing literals only, dev/TECHNICAL.md 2.2 / 10.2. Not dev/src/builder.rs (S93-fix8).

## Changed tests
`build_on::ac2_no_source_writes_nothing` if it runs without a slug.

## Out of scope
The "felt wrong" items of user-sim's second run (preview host error wording, `new --dir` into a
missing parent, reads_outside naming the cache path): leftovers for the product owner.
