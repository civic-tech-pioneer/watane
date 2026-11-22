Subsystem: host
Effort: low
Depends on: none
Slice: S99 — Alice has a first conversation with Claude on dev's fake `claude` (stage B, watane half)
## Goal
Fail closed on lines `claude` writes before it answers `initialize` (S99 adversary, pass 2, RISK 1):
today `Reading.held` in host/src/agent/claude/run.rs keeps them, unbounded, and pushes them after
`initialized` without any init check.
## Acceptance
AC1. A non-control line that `claude` writes before its answer to `initialize` kills the group and
     ends the stream `mismatch("init")`; no item but none is pushed (no `initialized`). Test with the
     host's fixture fake (host/tests/fixtures/fake-claude/) playing an `assistant` line before the answer.
AC2. Control lines before the answer (a `control_request` that the host answers deny at once, per
     15 "A2 in two slices") are handled as today; nothing is held, so `Reading.held` and its
     unbounded buffer are gone.
AC3. Host 19.3 control protocol step 2 or 3 says so in one sentence.
AC4. Every other test that passed still passes (74 IPv6 bind failures are known in this container).
## Spec
host TECHNICAL 19.3, the control protocol, steps 1 to 3.
## Interface
none
## Files
host/src/agent/claude/run.rs, control.rs, host/tests/it/claude.rs, host/tests/fixtures/fake-claude/*, host/TECHNICAL.md.
## Changed tests
Any claude test whose fake writes a non-control line before the answer (search host/tests/it/claude.rs
and the fixture fake); none expected.
## Out of scope
Lines real Claude Code writes between the answer and `system/init` (RISK 2): a probe with the real binary.
