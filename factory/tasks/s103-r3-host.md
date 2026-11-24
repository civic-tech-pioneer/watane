Subsystem: host
Effort: small
Depends on: none
Slice: S103 — Alice gives a mod's agents a folder, and can take it back (review r3, host part)
## Goal
Answers Codex review r3 of PR #26 (comment 7135640734) with the deputy's rulings. F3.1: the
host's tests build on Windows, now a required check. S103's Unix-only fixtures are compiled on
Unix only, each with a one-line reason, and the Windows test of the Claude launch refusal
stays. F3.2 belongs to S111 (stage D builds `https-stream.start`), so no change here. Polish:
P3.1 and P3.2, where each takes only a few lines. Built by the cloud foreman directly (no
relay: the routine allows one push).
## Acceptance
AC1. F3.1: `host/tests/it` modules `agent_tools` and `consent` are `#[cfg(unix)]`, and so are
     the `launch.rs` unit tests that use Unix paths or symlinks. Each gate has a one-line reason.
     `cargo test -p watane-host --locked --no-run` compiles for Windows.
     `agent::claude::s110_refuses_claude_without_process_group_controls` (Windows) is kept, and
     `test (windows-latest)` is green on the pushed head.
AC2. P3.1: the MCP open refusal leaves the question count unchanged. The MCP timer and stream
     refusals answer a tool error whose text starts with `not_granted:`.
AC3. P3.2: host 19.5, Borrowed authority, says why `agent-tools.call` fails the whole call
     instead of returning its `not-granted` case: so that the tool's effects roll back.
AC4. The touched tests pass, clippy and nightly fmt are clean, and
     `factory/check.sh s103-r3-host <base>` passes.
## Spec
host TECHNICAL 19.5 (Borrowed authority).
## Interface
none new.
## Files
`host/tests/it/main.rs`, `host/tests/it/agent_tools.rs`, `host/src/agent/claude/launch.rs`,
`host/TECHNICAL.md`.
## Changed tests
`agent_tools::s103_r2_f2_1_an_open_from_a_tools_call_is_not_granted`,
`agent_tools::s103_r2_f2_2_a_timer_under_borrowed_authority_is_not_granted` and
`agent_tools::s103_r1_ac1_a_tools_call_runs_the_tool_as_the_handles_origin_client` gain
assertions. Their behaviour does not change.
## Out of scope
F3.2 (S111). P3.1's direct read of the timer rows: `state.redb` can be read only after the host
closes, and that is more than a few lines in these tests.
