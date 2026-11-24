Subsystem: host
Effort: small
Depends on: none
Slice: S103 — Alice gives a mod's agents a folder, and can take it back (review r2, host part)
## Goal
Answers Codex review r2 of PR #26 (comment 7134242845) with the deputy's narrowed ruling: a tool
that runs on an origin client's borrowed authority (a job or `/mcp/agent` `tools/call`, and its
synchronous callees) may only do work that ends with that call (F2.1, F2.2). F2.3 is an accepted
risk, written as a known limit. Polish P2.1 and P2.2. Built by the cloud foreman directly (no
relay: the routine allows one push).
## Acceptance
Red first: each AC's test failed on b728fcb before the fix. Replaces the brief of 7ccac61,
written before the deputy's narrowed ruling (no deferred binding to the handle; F2.3 accepted).
AC1. F2.1: a job and a `tools/call` of a tool that calls `agent-tools.open(["c"])`, whose origin
     client holds the consent of `c`, get `not-granted`; no question is asked. The same `open` as
     an ordinary call of the mod returns a handle.
AC2. F2.2: a tool that arms a timer (job, `tools/call`) fails `not_granted` and leaves nothing
     (its call rolled back); a tool that starts `claude-code` (a stream) fails `not_granted` and
     asks nothing. The ordinary `after` of the mod is armed.
AC3. P2.1: the straddle test owns its hold inside the thread scope. P2.2: the per-root test lists
     all 19 patterns per tool and root.
AC4. clippy and nightly fmt clean; `factory/check.sh s103-r2-host b728fcb` passes.
## Spec
host TECHNICAL 19 (Terms), 19.5 (Borrowed authority, Known limit), 14 (A mod in the middle).
## Interface
none new.
## Files
`host/src/agent/mod.rs`, `host/src/agent/tools.rs`, `host/src/runtime/call.rs`,
`host/tests/fixtures/{mods,src}/toolc` (gains `open`, `arm`), `host/tests/it/{agent_tools,consent}.rs`,
`host/TECHNICAL.md`.
## Changed tests
`agent_tools::s103_r1_ac1_a_job_runs_the_tool_as_the_handles_origin_client` and
`..._a_tools_call_runs_...`: the tool's `claude-code.start` is now refused, no question (was: the
question named the handle's origin client). The literal tool lists of `toolc` gain
`toolc_api_arm`, `toolc_api_open`.
## Out of scope
F2.3 (accepted risk). P2.3 (app submission path; reinstalled-origin `gone` cannot be observed:
the consumer is the removed mod).
