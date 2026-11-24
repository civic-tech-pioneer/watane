Subsystem: host
Effort: medium
Depends on: none
Slice: S103 — Alice gives a mod's agents a folder, and can take it back (review r1, host part)
## Goal
Answers Codex review r1 of PR #26 (comment 7132007404) for the host: an agent's tool runs with
the handle's origin client, never the tool mod's consents (F1.1); whether it may run is decided
again, consistently, just before it runs (F1.2); a home whose `toolsets.json` already holds a
toolset named `agent` opens (F1.3); and the review's polish on the tests (P1.3). The foreman
wrote the rules into host TECHNICAL (commit 3f735c7): 19 Terms (origin client of an agent call),
19.5 "Authority at the run", 10.1 (a legacy `agent` toolset).
## Acceptance
Red first: each AC's test fails on the current code before its fix (say so in the report).
AC1. F1.1: through a job (`agent-tools.call`) and through `/mcp/agent` `tools/call`, a fixture
     tool that makes a request needing a consent (for example a `claude-code.start` in a folder
     new for the origin client; a small fixture variant is fine) reaches the test `Confirm` with
     `origin` = the handle's origin client, not the tool's mod; a consent held only by the tool's
     mod is not used. The install generation of the origin client is the one the handle had: if
     the origin client was removed and reinstalled under the same identity, the tool does not run
     (`gone`).
AC2. F1.2: a job queued for a tool whose consent was narrowed (a `grant` of `Manage` to its mod, a
     `remove` and reinstall of its mod under the same local name, a withdrawn toolset consent)
     between the `call` and the moment it runs does not run the tool: the item is the err of 19.5
     ("Authority at the run") and the tool's mod saw no call (an inert fixture tool that records
     calls). The same through `/mcp/agent`: a `tools/call` whose lookup and dispatch straddle such a
     change answers `-32602` and the tool saw no call. Use a hook or a held gate in the test, not
     a sleep.
AC3. F1.3: a home whose `toolsets.json` is `{"agent":["counter:api"]}` (written by the test as the
     old format) opens with `Host::open` and with the real `watane` program (no `bad_name`);
     the toolset is listed and can be removed; `set_toolset("agent", ..)` is still `bad_name`; a
     toolset token for it, if one can be issued, gets 403 on `/mcp/agent`.
AC4. P1.3: `tests/it/consent.rs` withdrawal tests observe turn output on the withdrawn stream
     before the withdrawal and fresh output on the other stream after it; the per-root test
     asserts every protected rule for both roots, as literal strings; `tests/it/agent_tools.rs`'s
     reinstall test uses literal expected tool names.
AC5. clippy and nightly fmt clean; `factory/check.sh s103-r1-host 3f735c7` passes.
## Spec
host TECHNICAL 19 (Terms), 19.5 (Authority at the run, Job, Key), 19.6 (requests), 10.1, 10.3;
FUNCTIONAL HOST-F-70, HOST-F-73, HOST-F-75.
## Interface
none new.
## Files
`host/src/runtime/call.rs`, `host/src/agent/tools.rs`, `host/src/tools.rs`, `host/src/server.rs`
if the MCP dispatch lives there, `host/src/lib.rs`, `host/src/catalog.rs`, `host/tests/fixtures/`
(a fixture variant if needed, rebuilt with build.sh), `host/tests/it/{agent_tools,consent,mcp}.rs`,
`host/TECHNICAL.md` (only to state what you built more precisely).
## Changed tests
`host/tests/it/mcp.rs` test(s) asserting that loading a `toolsets.json` with `agent` fails, if any
(after: it opens). The P1.3 strengthenings above.
## Out of scope
The app (s103-r1-app). The pre-initialize Stop mapping and a picked path changed by another
same-user program (proposed accepted risks of the review).
