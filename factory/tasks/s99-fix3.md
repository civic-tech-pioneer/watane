Subsystem: host
Effort: medium
Depends on: none
Slice: S99 — Alice has a first conversation with Claude on dev's fake `claude` (stage B, watane half)
## Goal
A `claude-code` (and `claude-code-shell`) stream started in a call chain is a stream of that chain at
once, as a `streams.open` stream is (19.2, Commit): in the same chain its consumer can `say`,
`interrupt`, `compact`, `streams.cancel` it and `streams.hand-off` it. This is what the unchecked
provider does in one call (agentic-harness TECHNICAL 10: `start` calls `claude-code.start`, then
`streams.hand-off`, and returns), and what agentic-harness S35 found missing (factory/PLAN.md, the S99
line's PO note): today `view_of` in host/src/agent/mod.rs knows only `Effect::Open`, and
`session_call` checks only `hub.info`, so the hand-off fails `unknown-stream` (surfacing as
`bad-launch: stream: unknown-stream`).
## Acceptance
AC1. One top-level call of a mod with `claude-code` that calls `claude-code.start(launch)` then
     `streams.hand-off(s)` succeeds; after the commit the stream's consumer is the caller of that call
     (the mod that called it), which gets `initialized` and the fake's lines as items, and can `say`.
     Test with host fixtures: a provider-like fixture mod (extend `tests/fixtures/mods/claude` or add
     one) called by a consumer fixture mod (e.g. `claude-b` or a new one) through a call link.
AC2. In one chain: `start`, then `say(s, "hi")` by the starting mod, then `hand-off(s)`: the fake's log
     shows the `user` message "hi" once, after `initialize`; the consumer after the commit is the
     hand-off target. A `say` after the `hand-off` in the same chain by the old consumer is `not-yours`.
AC3. In one chain: `start` then `streams.cancel(s)`: no process starts (the fake's log stays empty),
     the stream ends `cancelled`, no scratch folder is made.
AC4. A chain that starts, hands off and then fails (trap) applies nothing: no process, no stream, no
     scratch folder (19.6, chain rule).
AC5. Applying: the hand-off's catalog checks of 19.6 (At apply: generations of the target and origin
     client) hold as for an `Open` stream's hand-off; effects apply in chain order, so the `say` of
     AC2 reaches stdin only after the process has answered `initialize` (queue it on the session).
AC6. host 19.2 Commit (or 19.3, The session functions) says in one sentence that a host stream
     started in a chain is visible to that chain's later stream and session calls, as an `Open` is.
AC7. Every other test that passed still passes (74 IPv6 bind failures are known in this container).
## Spec
host TECHNICAL 19.2 (Commit, Acting side, hand-off), 19.3 (session functions), 19.6 (requests, At
apply); agentic-harness TECHNICAL 10 (/home/user/civic-tech-pioneer/agentic-harness/design/TECHNICAL.md),
read only.
## Interface
No WIT change. The `View` of a stream started in the chain: producer host, consumer the starting mod,
origin client the chain's, `in_chain: true`.
## Files
host/src/agent/mod.rs, host/src/agent/claude/*.rs, host/src/agent/streams.rs, host/tests/it/claude.rs,
host/tests/fixtures/mods/* (and build.sh's list), host/TECHNICAL.md.
## Changed tests
Any test that asserts `unknown-stream` for a same-chain `say`, `cancel` or `hand-off` of a just-started
claude stream (search host/tests/it/claude.rs; the s99-2 implementer noted "say in the same chain as
start returns unknown-stream").
## Out of scope
Asks and checks (S100).
