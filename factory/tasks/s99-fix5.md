Subsystem: host
Effort: low
Depends on: none
Slice: S99 — Alice has a first conversation with Claude on dev's fake `claude` (stage B, watane half)
## Goal
CI (ubuntu-latest, PR #13, job 137315693983) fails two host tests that need IPv6, so the cloud could
not run them:
1. `program::ac1_preview_creates_home_and_reports_ready` (host/tests/it/program.rs:246): a fresh
   preview home now has `state/`, because `Host::open` raises `generation` in the `host` table of state
   at every open (19.1, 7; S99). That is the intended behaviour: the test's list of names that a fresh
   preview must not have still holds `state` from before S99.
2. `program::connection::ac13_a_preview_with_fake_claude_runs_it_for_a_claude_code_start`
   (host/tests/it/program/connection.rs:80): `s.as_str()` is `None`: its `/call` requests send no
   `Origin` header, so the host answers 403 (8.2) and `value` is null.
## Acceptance
AC1. Test 1 asserts `state/state.redb` exists after the preview start, and still asserts that `log`,
     `dist`, `app` and `toolsets.json` do not. host 3.1 `Host::open` gains a step-11 line: raise
     `generation` in the `host` table of state (19.1), which creates `state/state.redb` at every open.
     Check that 3.1's "A preview home has none of them (W5)" (the four reserved names) still holds.
AC2. Test 2 sends `Origin: http://claude.localhost:<port>` on each `/call` (as program.rs's counter
     test does, line ~185), and on a failure prints the response body (so CI shows the error, not a
     bare unwrap).
AC3. Nothing else changes. This container has no IPv6, so these two tests cannot run here: read the
     code paths they exercise (8.2 Origin check; the claude fixture's `api.start`/`api.ended`) and say
     what you checked. Run clippy and fmt.
## Spec
host TECHNICAL 3.1, 7, 8.2, 19.1.
## Interface
none
## Files
host/tests/it/program.rs, host/tests/it/program/connection.rs, host/TECHNICAL.md.
## Changed tests
The two above.
## Out of scope
Anything else.
