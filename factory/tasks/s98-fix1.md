Subsystem: app
Effort: low
Depends on: s98-2
Slice: S98 — The macOS CI job is green again (guard)

## Goal
`gui_shell::ac5_install_and_each_statement_show_fields_and_withhold` failed once in 12 runner
rounds (S98 diag run 44894758202, round 1: `ask.wait` got `None` at gui_shell.rs:350). The test
force-enables `#approve` in one eval and clicks it in the next. Between the two, the consent page's
own focus gate (`app/ui/consent.js`: `arm()`'s 800 ms timer sets `gateReady = document.hasFocus()`,
and `blur` calls `disarm()`) can disable the button again — the hidden test window has no focus,
and parallel tests' windows take it. The click then lands on a disabled button and `answer`
returns early. After this task the enable and the click happen in the same JavaScript turn, as
`support::journey::approve` already does.

## Acceptance
- AC1 The test enables and clicks `#approve` in one eval (one JS turn); every assertion it makes
  is unchanged (same values, same withheld grant).
- AC2 clippy (`--features driver`) and nightly fmt pass; the window test itself is verified by the
  foreman on the macos-26 runner.

## Spec
app/AGENTS.md: "Window tests wait for page-load events, never a fixed time. Never weaken a test".

## Interface
None.

## Files
app/tests/it/gui_shell.rs (that one test), app/NOTES.md (one line in the S98 entry).

## Changed tests
`gui_shell::ac5_install_and_each_statement_show_fields_and_withhold`: same assertions; enable and
click in one turn.

## Out of scope
`app/ui/consent.js` (the gate is right; the test raced it); any timeout or sleep.
