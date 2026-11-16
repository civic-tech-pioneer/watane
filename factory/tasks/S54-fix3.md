Subsystem: app
Effort: low
Depends on: S54-fix2
Slice: S54 — A real static host (fix after the foreman's real run of S54-fix2)

## Goal

The real J3 stops failing on GitHub Pages' answers to a missing path. The foreman's real run
(2026-11-16, after S54-fix2) failed at j3.rs:473: `wait_status` had seen a 404 for
`<missing base>head`, yet Dan's install moments later got `HTTP 503 from <that URL>`. Pages' CDN
answers a missing path on a freshly built site with 503 or 404 depending on the edge, so no wait makes
it reliable. The outcome that matters holds either way: Dan gets a clear `dist` error that names the
URL, fast, and no mod. dist does not retry 5xx; whether it should is the product owner's question,
not this task's.

## Acceptance

AC1 In `ac3_j3_real_github`, Dan's missing-base assertion accepts a message that contains `not found`
or matches `HTTP 5<two digits> from`; every other assertion of that block stays (code `dist`, the
message names `<missing base>head`, under 60 s, Dan's `mods_list` empty). Keep the `wait_status`
404 wait before it (it makes the 404 path the likely one). Print one line to stderr naming which
answer came (`S54 missing base: not found` or `S54 missing base: HTTP 503`), with no token in it.
AC2 app/NOTES.md S54 section: replace the "503 for about a minute, then 404" line with what is now
known: two real runs saw 503 for a missing path after Pages had served the site, once after a 404
had already been seen for the same URL; the test accepts either and dist reports `HTTP 503 from
<url>` without a retry.
AC3 Builds and clippy with `driver` and `driver,real-github`; support_selftest green (the S54-fix1
source check must still pass).

## Spec

factory/tasks/S54-1.md, S54-fix1.md, S54-fix2.md.

## Interface

none

## Files

app/tests/j3.rs (`ac3_j3_real_github` only), app/NOTES.md.

## Changed tests

`ac3_j3_real_github` (AC1).

## Notes

Build only in main's target (CARGO_TARGET_DIR is set; never change it). No token: do not run the real
test; the foreman runs it. Stay out of app/src, app/ui, support/journey.rs, driver.rs, j2, j4,
manager (S74).

## Out of scope

dist retrying 5xx answers.
