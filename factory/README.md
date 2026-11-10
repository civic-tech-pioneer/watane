# The watane factory

A lean successor to the Colos Foreman factory. Part 1 says what Colos taught; part 2 is the setup.

## 1. What Colos taught (Nov 1 to 10, 567 commits)

Colos ran two eras, and they ended very differently.

**Era A: the Foreman pipeline (Nov 1 to 7).** Thirteen roles: triage, planner, breakdown, analyst,
acceptance author, Sol implementer, verifier, skeptic, adversary, fit audit, navigator, council and tuner.
It used OpenSpec, beads, a lineage checker and a gate.
- 19 changes accepted, about 60k lines of Rust, almost all substrate (keystore, channel store, sync,
  blob store). Nothing a person could use. Its own analysis on Nov 3: "27 of 135 T realized, but only
  2 of 83 F are done"; "Sol implementation took about 2.5 of about 69 active agent-hours (4%)".
- About 12M tokens for 57 Sol runs; Sol ended PARTIAL 35 times and DONE about 10 times.

**Era B: the walking skeleton (Nov 8 to 9).** The prompt told Sol to "ignore the factory". It had fixed
interfaces (`ARCHITECTURE.md` and WIT), small tickets that name the files they may touch, Sol in
parallel worktrees, and Opus reviewing and merging each one.
- 22 of 22 tickets DONE in about 1.5 days on about 2.35M tokens. It produced a runnable skeleton of
  13k lines: typed WASM mods, runtime, gateway, MCP toolsets, mod dependencies and events.

### Where it failed, by cost

1. **Process ate the work.** 96% of agent time went outside implementation. There were 77 council
   runs ("nine of 15 late councils were lineage bookkeeping"), 165 bookkeeping-only commits, a 351 KB
   NOTES.md and a process with its own 850-line budget. When the process is the product, nothing ships.
2. **Horizontal slices.** Milestones broke down along components, so each change finished a layer
   and no user story. Nothing showed that it worked until the end. The human had to force a re-plan
   on Nov 6.
3. **A queue stuck on the human.** Every ready bead waited on a human packet or a NO-CONSENSUS
   council. That gave about 35 idle hourly runs ("same signature 14+ times; no change started"). The
   inbox grew to 53 KB. The rule "never wait idle" was there, but nothing built around open questions.
4. **Sandbox blocks.** Sol could not open iroh's network socket, the bead lock or `ps`, so most
   PARTIALs said "needs a permitted runner". A custom Seatbelt profile fixed it, but only on day 3.
5. **Rework ping-pong.** One change needed 6 fix passes and a merge pass. The skeptic ran after
   VERIFIED and caused 3 more loops. Builds started while a council was open. Each fix was a cold
   Sol run (`--ephemeral`) that read the whole repository again.
6. **A global gate red for unrelated reasons.** Drafts without journey deltas blocked every change.
7. **Disk.** Each worktree had its own `target/`, from 11 GB up to 61 GB, and the disk filled up.

### What worked, and what watane keeps

| Colos | In watane |
|---|---|
| Era B: fixed interfaces, small tickets that list their files, Sol builds, Opus reviews | The whole core loop |
| `run.sh`: one entry point, old logs kept, a ledger of each run | `impl.sh`, with the same ledger |
| Sol at an effort set per task (medium by default, high for a boundary) | The same table |
| Acceptance tests with an independent oracle; never weaken a test | Implementer writes them first; validator checks them |
| The skeptic's checklist (new mechanism, weaker boundary, second copy of a concept, abstraction with one user) | In the validator's fit and simplicity checks |
| The "tripwire": stop when a fix needs a new mechanism | Implementer stops on costly-to-undo choices |
| One line per friction; act at three | `Friction` in `PLAN.md` |
| The adversary (found about 8 security defects) | Not yet. Add one run per step that touches a boundary (host sandbox, consent, signatures) |
| Mutation tests (they killed real mutants) | Not yet. Add `cargo mutants --in-diff` to the validator if weak tests slip through |
| Seatbelt profile for Sol | Not needed while watane has no iroh; Codex's own sandbox plus network and the cargo home |

Dropped: OpenSpec, beads, the lineage checker, the gate, the council, the navigator, the tuner,
the drift levels, the Foreman lock, the STE rules for agents, and the separate analyst and
acceptance author. watane's specs already carry the lineage (each subsystem has its own
`VISION`/`FUNCTIONAL`/`TECHNICAL` and build plan), and the slice's user-sim run is the check that
the lineage tried to be.

## 2. The setup

```text
foreman (Claude, /foreman)      product owner and your deputy; holds factory/PLAN.md
  └─ per slice: 1 to 4 tasks, one per subsystem
       ├─ implementer  Codex, gpt-6.1-sol, effort per task     factory/impl.sh
       ├─ validator    Claude Opus 5.5                          .claude/agents/validator.md
       │     round 1: return findings -> Sol resumes the same session
       │     round 2+: fixes small things itself
  └─ user-sim (Claude Opus 5.5) acts as the story's person on the real programs
       failure -> red regression test -> fix task (green, refactor) -> validator -> user-sim again
```

| File | Purpose |
|---|---|
| `.claude/skills/foreman/SKILL.md` | The loop: cut a slice, brief, build, validate, accept, decide or escalate |
| `.claude/agents/validator.md` | Acceptance coverage, architectural fit, simplicity; fix or return |
| `.claude/agents/user-sim.md` | Accepts a slice as its user; turns each failure into a red test |
| `factory/impl.sh` | Runs Sol on a brief, or resumes its session with feedback |
| `factory/implementer.md` | Sol's standing instructions (test-first, fixed interface, report format) |
| `factory/PLAN.md` | Slices, open questions, deputy decisions, friction, digest |
| `factory/tasks/` | One brief per task |
| `factory/runs/` | Sol logs, session ids and `ledger.jsonl` |
| `.cargo/config.toml` | One shared `target/` for all subsystems |

Escalation rule (SKILL section 5). The foreman decides as your deputy. It escalates only when it is
not confident, when the answer is costly to revert across subsystems, or when the question blocks
everything and no stub keeps the options open. Even then it builds the open option and carries on.

## 3. Start

From `watane/`, in Claude Code: `/foreman`. It refines slice S1 in `PLAN.md`, writes the task briefs
and runs them.
