Subsystem: app
Effort: low
Depends on: S54-fix1
Slice: S54 — A real static host (fix after user-sim's second REJECTED of outcome 3)

## Goal

The real J3's missing-base step reaches the network again. S54-fix1 moved the Ivy block first; once
Bob has chosen Ivy, dist holds a listed record of Alice's channel and `Dist::open_link` refreshes it
from its recorded bases (dist/src/fetch.rs ~385), so a link naming `<pages base>missing-<r>/` installs
from Alice's real base. That is dist's specified behaviour (a known channel is read from its recorded
bases), not a bug, and stays out of scope. The test does the missing-base install from a fresh home
(Dan: a new `App` home, no follows, no index) so the link's base is the only base dist knows.

## Acceptance

AC1 In `ac3_j3_real_github`: order stays Bob install, Ivy/DD-26 block, `wait_status(<missing>head,
404, 5 min)`, then the missing-base install by Dan (a fresh app home), asserting what S54-1 AC5
asserted: app `dist` error, message names a URL under the missing base and says `not found`, within
60 s; Dan's `mods_list` is empty. Bob's `channel_unfollow` before it is no longer needed: remove it
only if nothing else relies on it.
AC2 user-sim's red `support_selftest::regress_s54_missing_base_after_choosing_ivy_still_not_found`
(commit f056dfd) expects a dist change that is out of scope. Replace it with the behaviour the real
J3 now relies on, no network, dist only: a fresh dist home (no records) that opens Alice's link with
its base changed to an unpublished path on a loopback site gets `DistError::NotFound` for
`<missing base>head`. Keep the name prefix `regress_s54_missing_base`. If the existing
`ac5_missing_base_reports_http_not_found_after_install_fetch` already shows exactly this, delete the
red test instead and say so.
AC3 app/NOTES.md S54 section: one line that a known channel (followed or listed by a chosen index)
is read from its recorded bases, so the missing-base check uses a fresh home.
AC4 Builds and clippy with `driver` and `driver,real-github`; support_selftest green; fake J3 green.

## Spec

factory/tasks/S54-1.md, S54-fix1.md; dist/TECHNICAL.md 5.9 (listed records, bases).

## Interface

none

## Files

app/tests/j3.rs (`ac3_j3_real_github` only), app/tests/support_selftest.rs, app/NOTES.md.

## Changed tests

`ac3_j3_real_github` (AC1); `regress_s54_missing_base_after_choosing_ivy_still_not_found` (AC2).

## Notes

Build only in main's target (CARGO_TARGET_DIR is set; never change it). No token: do not run the real
test; the foreman runs it. Stay out of app/src, app/ui, support/journey.rs, driver.rs, j2, j4,
manager (S74).

## Out of scope

dist using a link's base for a known channel, or telling the person it ignored it (a question for the
product owner).
