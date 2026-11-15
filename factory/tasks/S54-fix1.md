Subsystem: app
Effort: low
Depends on: S54-1
Slice: S54 — A real static host (fix after user-sim REJECTED outcome 3)

## Goal

The real J3 stops failing on a Pages race. In user-sim's real run, right after Pages first served
the site, a never-published path under it answered HTTP 503 (a minute later: 404), so Bob's
missing-base install got `HTTP 503 from <url>` instead of dist's "not found". The test waits, with a
deadline, until the missing URL answers 404 before Bob's install, and checks Ivy's listing (AC4)
before the 404 step (AC5), so a Pages hiccup cannot hide AC4.

## Acceptance

AC1 The red test `app/tests/support_selftest.rs::regress_s54_real_j3_waits_for_cdn_404_before_missing_install`
(commit 65ba478) passes. If you can replace its source-text check with a check of behaviour at the
same cost (no network), do; otherwise keep it.
AC2 The wait is the harness's: `RealGithub::wait_served` generalised or a sibling
`wait_status(&self, url, status: u16, deadline)` that polls like `wait_served` (ureq, every 10 s,
same deadline panic text with last status and last error, token redacted). Covered by a no-network
self-test like the existing `ac2_pages_wait_*` ones. Deadline in J3: 5 minutes.
AC3 In `ac3_j3_real_github`, the Ivy/DD-26 block (AC4 of S54-1) runs before the missing-base block
(AC5 of S54-1); every assertion of both blocks stays as it is.
AC4 app/NOTES.md, S54 section: add that a never-published path under a freshly served Pages site
answered 503 for about a minute before 404 (user-sim's run, 2026-11-16), and that dist reports it
as `HTTP 503 from <url>` with no retry.
AC5 Builds and clippy with `driver` and `driver,real-github`; support_selftest green.

## Spec

factory/tasks/S54-1.md; app/tests/support/github.rs (`wait_served`).

## Interface

none

## Files

app/tests/j3.rs (`ac3_j3_real_github` only), app/tests/support/github.rs,
app/tests/support_selftest.rs, app/NOTES.md.

## Changed tests

`ac3_j3_real_github`: block order and the new wait (AC2, AC3). The red regression test (AC1).

## Notes

Build only in main's target (CARGO_TARGET_DIR is set; never change it). No token: do not run the real
test; the foreman runs it. Stay out of app/src, app/ui, support/journey.rs, driver.rs, j2, j4,
manager (S74).

## Out of scope

dist retrying 5xx answers (a question for the product owner).
