Subsystem: host
Effort: medium
Depends on: none
Slice: S103 — Alice gives a mod's agents a folder, and can take it back (review r2, host part)
## Goal
Answers Codex review r2 of PR #26 (comment 7134242845): a tool cannot open handles on the
client's authority (F2.1); nothing a tool defers keeps the client's authority after the handle
closes (F2.2); a stale tool fails with the promised `unknown-tool` / `-32602` before any export
resolution or decoding (F2.3); plus polish P2.1–P2.3 (host and app tests). The foreman wrote the
rules into host TECHNICAL 19.5: the paragraph after "Authority at the run" and the bullet
"Borrowed authority".
## Acceptance
Red first for each AC (say which failed before the fix).
AC1. F2.1: inside an agent tool's chain (the tool's call and a mod it calls through a link),
     `agent-tools.open` fails `not-granted`; the same open by the client outside an agent chain
     succeeds. Fixture: extend `toolc` (or a new small fixture) with a tool that opens a handle
     and one that calls a linked mod that opens one.
AC2. F2.2: a timer armed inside an agent tool's chain fires with the arming mod's own identity as
     origin client (a request it makes asks with `origin` = that mod, not the client; the
     `timers` row holds `"origin":"<arming mod identity>"`); a stream opened inside the chain (a
     mod stream, and a `claude-code` start in a scratch workspace) ends `gone` when the handle
     closes by `close`, by a withdrawn toolset consent and by the end of the opening mod; an
     equal stream or timer made by the client outside any agent chain goes on.
AC3. F2.3: hold an agent run before the snapshot / export resolution (move or add the
     `test-streams` hold so it sits before both), update the tool mod to a version without the
     selected export (and one with a changed signature): the job's item is
     `{"err":{"code":"unknown-tool",..}}`, `tools/call` answers `-32602`, the mod saw no call.
AC4. P2.1: the MCP test's `AgentHold` is owned inside the thread scope (an early assertion
     failure cannot hang the scope). P2.2: `consent::ac7_...` asserts all 19 patterns per tool
     and root (add `.envrc`, `build.rs`, `**/.github/workflows/**` and the other omitted ones) as
     literals. P2.3: `s103_r1_ac1_a_reinstalled_origin_client...` asserts the job's `gone`
     outcome; app `consent::tests` exercise a malformed Agent denial through the app's submission
     path (`consent_answer`'s logic, or the nearest testable function), check the question
     stays queued, and close it with a plain Deny.
AC5. The `agent_tools::ac6_withdrawing...` race fix of 6ee9de2 stays (run it 20 times in a loop
     under load: no failure). clippy and nightly fmt clean; `factory/check.sh s103-r2-host
     <base>` passes. Keep Windows building: S110 (now on main) gates Unix-only code with `cfg`;
     add a gate with a one-line reason where new code needs one, never skip a security check.
## Spec
host TECHNICAL 19 (Terms), 19.5 (Authority at the run, Borrowed authority, Job), 19.6 (timers),
19.7 (Withdrawal); FUNCTIONAL HOST-F-68, HOST-F-70, HOST-F-73.
## Interface
none new.
## Files
`host/src/runtime/call.rs`, `host/src/agent/` (`tools.rs`, `mod.rs`, `timers.rs`, `streams.rs`,
`consent.rs`, `claude/`), `host/tests/fixtures/` (rebuilt with build.sh),
`host/tests/it/{agent_tools,consent}.rs`, `app/src/consent.rs` (tests only, P2.3),
`host/TECHNICAL.md` (to state what was built precisely).
## Changed tests
Only the strengthenings above.
## Out of scope
Account consent and `https-stream` (A4): only its dependency on the handle when started in an
agent chain, if `https-stream.start` exists today; else note it in 19.5 for A4.
