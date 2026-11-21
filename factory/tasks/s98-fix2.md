Subsystem: node
Effort: medium
Depends on: none
Slice: S98 — The macOS CI job is green again (guard)

## Goal
`causes::ac2_a_bad_copy_on_the_server_is_not_read_as_not_followed` (node/tests/it/causes.rs)
fails now and then: `light.fetch(..)` returns `Ok(Fetched { .. })` with the good `reading-list`
1.0.0 package where the test expects an error naming the server (causes.rs:238). Seen on the
macos-26 runner (S98 CI run 44900266411) and on 2026-11-20 on main (PLAN.md, main check at
`5899450`: "likely the anchor answering before its net turned off", unproven). It reproduces on
Linux in this container: 1 in 48 runs of `it causes::` with six copies of the binary running at
once. After this task the cause is known and named in app-independent words in node/NOTES.md, and
the test waits on the real condition (or the product is fixed, if the cause is a product bug).

First find the cause, before any fix. Candidates to rule in or out, with evidence (logs, a
counter, a reproduction loop): (a) the light machine gets the package from the anchor (the anchor's
`set_options(Default::default())` closes its endpoint synchronously, net/src/lib.rs `apply`, but a
fetch could already be in flight or a link cached); (b) the server serves the good bytes although
its file was overwritten (a blob store that keeps small blobs inline or in memory, or a file
`corrupt_file_of` never reached: the test only rewrites files whose length equals the package
size, so it depends on how the store lays out a 450-byte blob); (c) the light machine already held
the blob before the fetch. If the cause is that a verified-hash check is skipped (bytes that do not
match their hash are accepted), stop and report it: that is a security finding, not a test fix.

## Acceptance
- AC1 The cause, with the evidence that shows it, in node/NOTES.md (one entry).
- AC2 Red first where possible: a loop of the test under load fails before the fix and passes
  after (e.g. 60 runs of `cargo test -p watane-node --test it causes::` with six at once, 0
  failures); report the numbers.
- AC3 The test keeps its assertions (the chain names the server and never "is not followed"); it
  waits on the real condition (no sleep, no longer timeout); no other test weakened.
- AC4 `cargo test -p watane-node` (default and `-- --include-ignored` if it runs here), clippy
  `--all-targets -D warnings`, nightly fmt.

## Spec
node/TECHNICAL.md NO-14 (the bad copy reads as the server's cause); node/AGENTS.md.

## Interface
None to siblings, unless the cause is a product bug; then name the change in the report before
making it.

## Files
node/tests/it/causes.rs, node/tests/it/support (if a helper is needed), node/NOTES.md. Product
code (node/src, net/src, dist/src) only if the cause is a product bug, and say so.

## Changed tests
`causes::ac2_a_bad_copy_on_the_server_is_not_read_as_not_followed`: same assertions, waits on the
real condition. (`ac1_a_removed_light_machine_*` shares `stored()`; keep it passing.)

## Out of scope
app; the Windows build of watane-log.
