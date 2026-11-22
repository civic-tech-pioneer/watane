Subsystem: host
Effort: low
Depends on: none
Slice: S99 — Alice has a first conversation with Claude on dev's fake `claude` (stage B, watane half)
## Goal
No session write (`say`, `interrupt`, `compact`, from this chain or any later one) reaches `claude`'s
stdin before `claude` has answered `initialize` and the host has checked that answer (19.3 step 2).
S99's adversary, pass 3: today the writer thread writes the whole session queue as soon as the process
starts, so a `say` queued behind `initialize` reaches `claude` unchecked.
## Acceptance
AC1. `claude::adv_s99_say_in_the_start_chain_reaches_stdin_before_the_answer_to_initialize` (the
     adversary's red test, commit 8742b81, kept as written) passes.
AC2. Writes are held until the answer passes step 2, then written in queue order; if step 2 fails
     (mismatch, exit) none is written. Not held until `system/init` (real Claude Code may print it only
     after the first user message).
AC3. host 19.3 step 5 says so in one sentence.
AC4. Every other test that passed still passes (74 IPv6 bind failures are known here).
## Spec
host TECHNICAL 19.3, control protocol steps 1, 2 and 5.
## Interface
none
## Files
host/src/agent/claude/run.rs, mod.rs, host/tests/it/claude.rs, host/tests/fixtures/fake-claude/*, host/TECHNICAL.md.
## Changed tests
none expected.
## Out of scope
A hand-off past the target's 32 streams (adversary RISK; a spec question for 19.2).
