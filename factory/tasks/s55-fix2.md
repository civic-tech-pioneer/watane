Subsystem: app
Effort: low
Depends on: none
Slice: S55 — Alice publishes on one machine, Bob installs on another and uses the mod (J3 between two machines)

## Goal
When a journey waits for a consent window that never comes because the command already failed
(Bob cannot reach Alice's web host), the test fails with the app's own cause, not with
`assertion failed: app.wait("consent", TIMEOUT)`. A maintainer whose guest cannot reach the host
then reads the network error in the vm_two_machines.sh output.

## Acceptance
- AC1 The red test `regress_s55::regress_s55_unreachable_site_names_the_cause`
  (app/tests/it/regress_s55.rs, commit e4add09) passes; nothing else changes.
- AC2 `support::publish::consent` (or the waiting it does) stops waiting as soon as the command
  started by `begin` has settled with an error (`window.__stageC` holds `{error}`), and panics with
  a message that contains that error's text. When `begin` was not used, or the command is still
  pending, it behaves as now. Every existing caller of `consent` keeps passing.

## Spec
app/TECHNICAL.md 10 (driver), 11 (test plan). Test support only.

## Interface
none.

## Files
app/tests/support/publish.rs (and app/tests/support/journey.rs if the wait lives there),
app/tests/it/regress_s55.rs (only if the red test needs a non-weakening adjustment).

## Changed tests
none; the red test turns green.

## Out of scope
Product code. install scripts (s55-fix3).
