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
| The adversary (found about 8 security defects) | `.claude/agents/adversary.md`, once per slice that touches a section 6 boundary (foreman skill, section 3) |
| Mutation tests (they killed real mutants) | `cargo mutants --in-diff` on each task's diff, in the validator; weekly on all of `pkg` (`.github/workflows/mutants.yml`) |
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
       ├─ adversary (Claude Opus 5.5) attacks the slice if it touches a security boundary
       │    breach -> red test -> fix task, as for user-sim
       ├─ user-sim (Claude Opus 5.5) acts as the story's person on the real programs
       │    failure -> red regression test -> fix task (green, refactor) -> validator -> user-sim again
       └─ DELIVERED + PR with auto-merge -> CI on Linux, macOS, Windows -> green: GitHub merges
```

| File | Purpose |
|---|---|
| `.claude/skills/product-owner/SKILL.md` | The loop: sketch, hand out, answer questions, merge, escalate |
| `.claude/skills/foreman/SKILL.md` | One slice end to end: brief, build, validate, user-sim, deliver |
| `.claude/agents/validator.md` | Acceptance coverage, architectural fit, simplicity; fix or return |
| `.claude/agents/user-sim.md` | Accepts a slice as its user; turns each failure into a red test |
| `factory/impl.sh` | Runs Sol on a brief, or resumes its session with feedback; then `check.sh` |
| `factory/check.sh` | A task's tests outside Sol's sandbox: default, clippy, fmt, slow tests of touched files, `cargo deny` if `Cargo.lock` changed |
| `.github/workflows/ci.yml` | The slow sensor: the whole workspace on Linux, macOS and Windows for every PR to `main`: build, tests, clippy, fmt, `cargo deny` (`deny.toml`) |
| `factory/check-main.sh` | What CI cannot run, on the Mac after each merge |
| `.claude/agents/adversary.md` | Attacks a slice that touches a security boundary; each breach becomes a red test |
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
| Integration | a command or route on prebuilt fixtures (host: `tests/fixtures`; dev: one warm template cache, shared by every dev test and kept between runs (a new watane-dev rebuilds only what changed); a copy at another path builds cold) | < 1 s each | default `cargo test` |
| Slow | a cold build, a full pipeline on the real `watane`, one per thing only it proves (DEV-S-10 to 13, the preview life cycle) | 10 s to minutes | `#[ignore = "slow: ..."]` |
| Journey | J1 to J5, one test each | minutes | `#[ignore = "slow: journey ..."]` |
| Front | an app GUI check that needs real focus or stacking (`App::front`); waits for the human to be idle | seconds each, plus the wait | `#[ignore = "front: takes the screen"]`, once per slice |
| User-sim | the slice's story on the real programs, by hand | an agent run | once per slice |

Rules:
- A subsystem's default `cargo test` stays under 2 minutes on a quiet machine. A test that needs
  more is `slow`, and its reason names what no cheaper test can prove.
- An integration test never builds a mod from scratch unless the build is what it tests.
- A subsystem's integration tests are one binary: each file is a module of `tests/it/main.rs`
  (`tests/it/<file>.rs` and a `mod <file>;` line), never a new `tests/<file>.rs`. Every test
  binary links wasmtime and is scanned on its first run; 124 of them took 6.84 GB and 42 s to
  relink after a change to `pkg`. Run one file's tests with `cargo test --test it <file>::`. A
  test that changes or counts process-wide state (env, umask, cwd, open sockets) runs itself in a
  child process (`static_config`, `unpack_umask`, net and node `support::rerun_alone`); a lock
  that kept a file's tests apart lives in `tests/support` when files share it (app `GUI`); net's
  `support::slot` lets at most 6 tests run machines at once, as its deadline tests need; install's
  `support::RunPermit` lets at most 4 scripts run at once.
- App GUI tests run hidden (`App::start`) or in the background (`App::background`): no window on
  screen, no Dock icon, no focus taken. Only `front` tests take the screen.
- Who runs what: Sol runs the tests it works on, and the default tests once at the end.
  `factory/check.sh` (run by `impl.sh` after every Sol run, outside the sandbox) runs the default
  tests, clippy, fmt and the `slow:` tests of each touched test file (app: with `driver`); a
  failure goes back to Sol before any validator. The validator reads that log instead of
  rerunning it, and reruns `check.sh` only after its own fixes; never the whole slow set or
  driver suite, never `front` tests. The
  foreman runs everything once, `front` tests included, before it delivers. The product owner
  reruns a suite only for what changed after that run (a rebase on a newer `main`, a flake), plus
  the tests the cloud could not run. The user-sim runs no suite; it uses the programs.
- A failure the user-sim finds becomes a test at the lowest level that shows it.

## Time and value

Agents do not feel time pass, so test suites grew to over an hour and a cosmetic flake took a
day without anyone flinching. These checkpoints stand in for that feeling. They are not quotas:
they catch an excess early, while turning back is cheap.

- **Weigh what the work protects** before a fix slice, a new slow test or another round:
  - *guard*: authorization, consent, signatures, isolation, data loss (`spec/TECHNICAL.md` 6).
    Worth what it takes. A flake here is chased to its cause and never ignored on a hunch.
  - *core*: a journey step, or a feature a person relies on.
  - *polish*: wording, layout, default focus, convenience. One attempt: one diagnosis of a
    flake, one task for a fix. If that is not enough, it becomes an accepted limit in `PLAN.md`
    (a flaky test gets `#[ignore = "flaky: <cause>, accepted <date>"]`), not another slice.
- **Suite time is a budget.** Each delivery reports, per touched subsystem, the wall time of its
  default `cargo test` and of its `--include-ignored` run, before and after the slice. The product
  owner keeps them in `PLAN.md`'s `Suite time` table. A default run over 2 minutes, a slow set
  over 10, or either growing more than 20% in one slice: that subsystem's next slice moves tests
  down the pyramid or deletes duplicates before it adds features.
- **Elapsed time is a signal.** A task past 3 Sol runs, a flake past one diagnosis, or a slice
  in flight for more than a working day: stop and weigh again. Is what it protects still worth
  the time? Write the answer as a decision line. Continuing is fine; continuing without asking
  is not.
- **A new test that starts a process, a window or a VM** says what a unit or fixture test cannot
  show. That is in the pyramid's rules too; the validator holds to it.

## Timing tests

Tests that assert a spec'd time limit (a call stopped after 200 ms and before 2 s) fail under
the load of parallel slices, so they carry `#[ignore = "timing: run on a quiescent machine, ..."]`.
Sol and validators skip them. The product owner runs them on `main` after each merge, one at a
time; a failure counts only if it repeats:
`cargo test -p watane-host --test it -- --ignored --test-threads=1 runtime::` and
`cargo test -p watane-host --test it -- --ignored --test-threads=1 program::catalog::ac3_shutdown`;
`factory/check-main.sh` runs them.
A new test of a time limit gets the same attribute. A wait that only guards against a hang is
not such a test: give it a wide bound (30 s or more) and leave it in the normal run.

## Tests Sol's sandbox cannot run

Sol's sandbox cannot start a nested `sandbox-exec` or the GUI, so DEV-S-13 (the build with network
access denied) and the window tests run first in `factory/check.sh`, right after Sol. That is
expected; it is not friction.

## Two feedback loops

| Loop | Sensors | Where | Takes | Before |
|---|---|---|---|---|
| Fast | `factory/check.sh`, the validator, the adversary, the user-sim | the agent loop, on the foreman's machine | minutes | each task, then the slice's delivery |
| Slow | CI (`.github/workflows/ci.yml`): build, default tests and clippy on Linux, macOS and Windows; fmt and `cargo deny` once | GitHub, through a pull request | warm cache: Linux ~15 min, macOS ~30; cold ~45 | the merge to `main` |

A slice is checked on one OS while it is built: the Mac (local) or Linux (cloud). A Linux slice
broke `main` on macOS twice (S91: a Linux-only `mkfifoat`, an errno that differs), and only the
Mac check after the merge saw it. CI closes that gap without slowing the agents: the fast loop
stays as it is, and the slow one runs while the product owner and foremen do other work.

- On delivery the foreman, local or cloud, pushes `slice/<id>`, opens a PR to `main` and arms
  auto-merge (`gh pr merge --auto --rebase`; foreman skill, section 5). `main`'s ruleset
  ("main: CI green before merge") requires `lint`, `test (ubuntu-latest)` and `test (macos-26)`,
  so the PR merges itself when they pass. Nobody merges by hand. A red run is a failure on
  another OS: the local foreman fixes it on its branch; for a cloud slice the product owner
  starts a new routine (product-owner skill, section 5).
- The ruleset's admin bypass lets the product owner push its own commits (`PLAN.md`, a small
  fix) straight to `main`; it also forbids force-pushing or deleting `main`. GitHub deletes a
  merged PR's branch.
- After the merge, the Mac runs only what CI cannot: `factory/check-main.sh` (the timing tests,
  one at a time) and a cloud slice's `not run (cloud)` tests (`front`, DEV-S-13).
- A test red in CI but green in the slice's runs is rerun once before it counts (product-owner
  skill, section 5); one that repeats is a flake to weigh ("Time and value").
- CI runs the default tests only. `slow`, journey, `front`, `timing` and `real` tests stay
  `#[ignore]`d there, as in `check.sh`.
- App window (`driver`) tests: on macOS in CI, `macos-26` (runners have a GUI session; on
  `macos-15` mod windows at `*.localhost` never loaded). On Linux they need Xvfb and a window
  manager (`app/scripts/linux-test.sh`, run by hand): a later step for CI. Windows has none.

## Windows

CI builds on Windows but does not block a merge (`continue-on-error`) until the workspace
compiles there. First run (2026-11-20): `pkg`, `dist` and `install` compile; two things stop the
rest.
1. `log`'s identity key file opens with `O_NOFOLLOW`, `O_NONBLOCK` and mode `0600`. That is a
   security check (`spec/TECHNICAL.md` 6): Windows needs its own design (no-follow through
   `FILE_FLAG_OPEN_REPARSE_POINT`, an owner-only ACL), not a `cfg` that skips it. Everything that
   depends on `log` (`host`, `net`, `node`, `dev`, `app`) waits on it. This needs the human.
2. `app` needs `app/icons/icon.ico` for Tauri's Windows resource file.
After those come the next compile errors, then the tests. The shell installer (`install.sh`),
`sandbox-exec` (DEV-S-13) and the AppKit driver are macOS-only already. Windows becomes
blocking when its run is green.
