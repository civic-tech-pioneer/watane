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
| Sol at an effort set per task (medium by default; high only for an open security decision) | The same table |
| Acceptance tests with an independent oracle; never weaken a test | Implementer writes them first; validator checks them |
| The skeptic's checklist (new mechanism, weaker boundary, second copy of a concept, abstraction with one user) | In the validator's fit and simplicity checks |
| The "tripwire": stop when a fix needs a new mechanism | Implementer stops on costly-to-undo choices |
| One line per friction; act at three | `Friction` in `PLAN.md` |
| The adversary (found about 8 security defects) | Not yet. Add one run per step that touches a boundary (host sandbox, consent, signatures) |
| Mutation tests (they killed real mutants) | Weekly on `pkg` (`.github/workflows/mutants.yml`, about 20-30 min). Add `cargo mutants --in-diff` (about 1 min) to the validator if weak tests slip through |
| Seatbelt profile for Sol | Not needed while watane has no iroh; Codex's own sandbox plus network and the cargo home |

Dropped: OpenSpec, beads, the lineage checker, the gate, the council, the navigator, the tuner,
the drift levels, the Foreman lock, the STE rules for agents, and the separate analyst and
acceptance author. watane's specs already carry the lineage (each subsystem has its own
`VISION`/`FUNCTIONAL`/`TECHNICAL` and build plan), and the slice's user-sim run is the check that
the lineage tried to be.

## 2. The setup

```text
product owner (Claude, /product-owner, on main)   your deputy; owns factory/PLAN.md
  │  sketches one slice ahead; hands it out when no slice in flight touches the same subsystems
  └─ foreman (Claude, /foreman <slice>, own session, worktree on slice/<id>)
       ├─ per task (1 to 4, one per subsystem):
       │    implementer  Codex, gpt-6.1-sol, effort per task     factory/impl.sh
       │    validator    Claude Opus 5.5                          .claude/agents/validator.md
       │       round 1: return findings -> Sol resumes the same session
       │       round 2+: fixes small things itself
       ├─ user-sim (Claude Opus 5.5) acts as the story's person on the real programs
       │    failure -> red regression test -> fix task (green, refactor) -> validator -> user-sim again
       └─ DELIVERED -> product owner merges --ff-only, removes the worktree, hands out the next slice
```

| File | Purpose |
|---|---|
| `.claude/skills/product-owner/SKILL.md` | The loop: sketch, hand out, answer questions, merge, escalate |
| `.claude/skills/foreman/SKILL.md` | One slice end to end: brief, build, validate, user-sim, deliver |
| `.claude/agents/validator.md` | Acceptance coverage, architectural fit, simplicity; fix or return |
| `.claude/agents/user-sim.md` | Accepts a slice as its user; turns each failure into a red test |
| `factory/impl.sh` | Runs Sol on a brief, or resumes its session with feedback |
| `factory/implementer.md` | Sol's standing instructions (test-first, fixed interface, report format) |
| `.claude/agents/implementer.md` | Sonnet in Sol's place, for cloud foremen (no Codex there); same instructions |
| `factory/cloud-setup.sh` | SessionStart hook: wasm target and vendor folder in a cloud container; no-op on the Mac |
| `factory/watch-cloud.sh` | Waits for a cloud foreman's pushed `DELIVERED`/`BLOCKED` commit |
| `factory/PLAN.md` | Slices, open questions, deputy decisions, friction, digest |
| `factory/tasks/` | One brief per task, committed on the slice branch |
| `factory/runs/` | Sol logs, session ids and `ledger.jsonl`, in the main checkout (not in git) |

Questions go up in two hops. The foreman decides what stays inside its slice and is cheap to
undo; the rest goes to the product owner. The product owner escalates to you only when it is not
confident, when the answer is costly to revert across subsystems, or when the question blocks
everything and no stub keeps the options open. Even then the slice builds the open option and
carries on.

Sol runs in Codex's own sandbox with network on. Codex refuses writes inside `.git` unless each git
folder is named, so `impl.sh` names the worktree's git folder and the shared one. A resumed Codex
session gets the same settings. Both were tested from a worktree on 2026-11-10.

## 3. Start

From `watane/` on `main`, in Claude Code: `/product-owner`. It refines slice S1 in `PLAN.md` and
hands it to a foreman session.

## Test pyramid

Many cheap tests, few expensive ones. Each behaviour is proven at the lowest level that can show
it; a higher level only proves that the parts work together.

| Level | What | Cost | Runs in |
|---|---|---|---|
| Unit | a function in the crate (`#[cfg(test)]`): a rule, a parser, a classifier, an error text | ms | default `cargo test` |
| Integration | a command or route on prebuilt fixtures (host: `tests/fixtures`; dev: one warm template cache per test binary, shared by its tests; a copy at another path builds cold) | < 1 s each | default `cargo test` |
| Slow | a cold build, a full pipeline on the real `watane`, one per thing only it proves (DEV-S-10 to 13, the preview life cycle) | 10 s to minutes | `#[ignore = "slow: ..."]` |
| Journey | J1 to J5, one test each | minutes | `#[ignore = "slow: journey ..."]` |
| Front | an app GUI check that needs real focus or stacking (`App::front`); waits for the human to be idle | seconds each, plus the wait | `#[ignore = "front: takes the screen"]`, once per slice |
| User-sim | the slice's story on the real programs, by hand | an agent run | once per slice |

Rules:
- A subsystem's default `cargo test` stays under 2 minutes on a quiet machine. A test that needs
  more is `slow`, and its reason names what no cheaper test can prove.
- An integration test never builds a mod from scratch unless the build is what it tests.
- App GUI tests run hidden (`App::start`) or in the background (`App::background`): no window on
  screen, no Dock icon, no focus taken. Only `front` tests take the screen.
- Who runs what: Sol runs the default tests. The validator runs the default tests and the slow
  tests of its task's files (by name), not the whole slow set, and never `front` tests; for the
  app that means `--features driver --test <file>` for each touched test file, never the whole
  driver suite. The
  foreman runs everything once, `front` tests included, before it delivers. The product owner
  reruns a suite only for what changed after that run (a rebase on a newer `main`, a flake), plus
  the tests the cloud could not run. The user-sim runs no suite; it uses the programs.
- A failure the user-sim finds becomes a test at the lowest level that shows it.

## Timing tests

Tests that assert a spec'd time limit (a call stopped after 200 ms and before 2 s) fail under
the load of parallel slices, so they carry `#[ignore = "timing: run on a quiescent machine, ..."]`.
Sol and validators skip them. The product owner runs them on `main` after each merge, one at a
time; a failure counts only if it repeats:
`cargo test -p watane-host --test runtime -- --ignored --test-threads=1` and
`cargo test -p watane-host --test preview -- --ignored --test-threads=1 ac3_shutdown`.
A new test of a time limit gets the same attribute. A wait that only guards against a hang is
not such a test: give it a wide bound (30 s or more) and leave it in the normal run.

## Tests only the validator runs

Sol's sandbox cannot start a nested `sandbox-exec`, so DEV-S-13 (the build with network access
denied) runs only in the validator. That is expected; it is not friction.
