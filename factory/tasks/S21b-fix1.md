Subsystem: app
Effort: medium
Depends on: S21b-3
Slice: S21b — J3 in the app (fix: two intermittent window tests).

## Goal
The foreman's full run (`cargo test --features driver --no-fail-fast -- --include-ignored
--test-threads=1` in `app/`, after rebasing on main) fails intermittently in two window tests. Find
each cause and fix it in the test or in the app, whichever is wrong, without weakening a test.

## Acceptance
- AC1 `tests/api.rs` `ac6_publish_through_consent_window_and_host_guards` passes 10 of 10 runs with
  `--test-threads=1`. Seen 2 failures in about 6 serial runs: `driver error: {"err":"channel is
  empty and sending half is closed","id":43}` (then the publish thread sees `IncompleteMessage`).
  Suspect: line 177 clicks `#deny` inside `eval`, the consent window is destroyed before the eval
  callback returns; deferring the click with `setTimeout` alone did not fix it (1 of 3 failed).
- AC2 `tests/j2.rs` `ac8_remove_defaults_to_keep_data_and_unknown_keys_fail` passes 10 of 10 runs.
  Seen 2 failures in about 4 runs: `DOM timed out: main: !document.querySelector('[data-local=recommendations]')`
  after `#keep-data` is clicked. Check whether S21b-3's `ui/app.js` (polling, stage C refresh) can
  render a stale mods list after the remove, or whether the test is racy; say which, and whether it
  also flakes on `main` before S21b.
- AC3 the rest of the app driver suite, J2 and J3 (`--include-ignored`, never front tests) still pass;
  clippy `--features driver -D warnings` and fmt pass.

## Spec
`app/TECHNICAL.md` 7 (polling), 10 (driver), 11 ("Window tests wait for page-load events, never a
fixed time").

## Interface
None changes.

## Existing tests whose assertion changes
None; a fix may change how a test waits, not what it asserts.

## Files
`app/tests/{api,j2}.rs`, `app/tests/support/*`, `app/ui/app.js`, `app/src/{driver,views,commands,core}.rs`,
`app/NOTES.md`.

## Out of scope
Everything else. No change outside `app/`.
