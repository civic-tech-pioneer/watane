# Plan

The deputy's state, and only the present (deputy skill, section 0): CI fails this file past 500
lines. History is in `factory/DONE.md`. Upkeep work is in GitHub issues labelled `upkeep` (with
`guard`, `core` or `polish`) and `flaky`. Deputy decisions past their veto window are written into
the spec that owns them; those from before 2026-11-27 wait in `factory/decisions/LOG.md` for that
(issue linked there).

## Slices

The main line: step 11, the agent layer; its next slice, JF1-1, waits on the human (JF1-ADOPT).
The human chose (2026-11-21, "the bridge") to build the agent layer next, driven by the collection's
product owner (watane-mods `/po`, `factory/config.toml` here; watane keeps its own foreman, agents,
check.sh and CI). The design lives in `watane-mods/agentic-harness/design/` (VISION, FUNCTIONAL,
TECHNICAL; N1 to N11 and R1 are the human's answers there). Its TECHNICAL 15 is the checklist of
watane changes the human accepted (N3); TECHNICAL 17 gives each stage its watane part and its
agentic-harness part. A stage's watane slice goes first; the agentic-harness slice that uses it
names the same interface in its sketch and fakes it until this one merges.

## Suite time

Wall time of each subsystem's tests on `main` (`factory/README.md`, "Time and value"): the
default `cargo test` (budget 2 min) and `--include-ignored` (budget 10 min). From the main check
after each merge; under load, say so. A row over budget, or up more than 20% in one slice, makes
that subsystem's next slice a test-cost slice.

| Subsystem | Default | With ignored | Measured |
|---|---|---|---|
| app | 9 s; 95 s with `--features driver` | 79 s (11 slow, no front/real/vm; S90) | main `393ae6c` (MAIN PASS), integration binary, load ~10 to 17 |
| dev | 22 s | 283 s (46 slow, real `watane`) | main `393ae6c`; slow set at `e04b480` (S93), load ~10 |
| dist | 28 s | not measured | main `393ae6c` (MAIN PASS), integration binary, load ~10 to 17 |
| host | 11 s | not measured | main `393ae6c` (MAIN PASS), integration binary, load ~10 to 17 |
| install | 125 s (S94; cloud 8 s) | 483 s | main `b5e54ab`, Mac, integration binary, load ~6 to 11 |
| log | 7 s | not measured | main `393ae6c` (MAIN PASS), integration binary, load ~10 to 17 |
| net | 24 s | not measured | main `393ae6c` (MAIN PASS), integration binary, load ~10 to 17 |
| node | 16 s | not measured | main `393ae6c` (MAIN PASS), integration binary, load ~10 to 17 |
| pkg | 8 s | not measured | main `393ae6c` (MAIN PASS), integration binary, load ~10 to 17 |

## Waiting on the human

- **JF1-ADOPT (2026-11-27, Deputy Scissors): watane-factory exists (STEP6-REPO answered); JF1-1 (`spec/design/factory/TECHNICAL.md` 6) is the main line's next slice and needs from you, in one go:** (1) the publisher who signs the factory mod and its slug (TECHNICAL 1: "chosen when the repository is made"; rec: slug `factory`, your own maker identity as for the other watane-mods); (2) adopt it into the collection: `watane-mods/factory/bin/adopt.sh <folder> civic-tech-pioneer/watane-factory` (writes config.toml and PLAN.md, adds the ruleset that holds main to `gate`, like the other mod repos) and a `[watane-factory]` row in `projects.toml` (a new project, Door H); (3) add the repository to the Codex `watane` environment (an account setting; until then JF1-1 builds with Claude, which rule 3 picks anyway). JF1-1 is handed out to Claude as soon as (2) has run; (1) can follow, the build keeps the slug and publisher in one place.

- (2026-11-25, deputy-5, S122 merged: agentic-harness stage E's watane half is done) **A real judge account for V7.** (2026-11-27, Deputy Paper: stale for stage E, which merged as agentic-harness S127 on 2026-11-26 with Jev as its judge, PR civic-tech-pioneer/agentic-harness#56; N5 decided (a) by the human 2026-11-27, agentic-harness PLAN fc09fb3.) Stage E's checked provider (agentic-harness) needs a judge service the provider can call with a key Alice enters; nothing in the factory can open one (an account is the human's). Which judge (Jev or another), and may the factory use a key of yours in its cloud tests? Blocks agentic-harness stage E, so step 6's JF1.

- **V8 writer busy share (S96).** One durable write per batch costs 24.5% writer busy at stage A's load
  (budget 10%, which S96 applies to the no-write run). Options: (a) keep "durable first" (19.6) and
  accept the write run's share as N4's price (rec: a kernel batch rarely writes); (b) group commits
  across batches (a host change, weaker per-batch durability). Undo: (a) none; (b) a host write path.

- (2026-11-21, from S95; none blocks stage A's A1) Six security questions in host/TECHNICAL 18, each
  with its options there: Q38 the consent key names no provider (`via`), blocks A3 (answered (b) by the human, 2026-11-24: Human answers); Q39 the Claude
  login token and loopback under the shell grant (probe V28; agentic-harness S1's V27 found a
  loopback proxy breach), blocks the shell grant in A2; Q40 sessions read each other's scratch and
  run folders, blocks A2's scratch work; Q41 symlinks past the deny rules (V29), blocks A2; Q42 login
  files the closed deny list misses under the shell grant; Q43 the content shape of `say` and a
  lookalike `/` (V30), blocks A2. Also Q37: requests ride the call chain, but writes commit per
  invoke (host Q3).

- (2026-11-20, from S93; not blocking) The build boundary: wit-bindgen, run by rustc during a
  mod build, opens an absolute `generate!`/include path the mod names (e.g. a macOS autofs
  `/net/<host>/...`) before dev's dep-info check rejects it. dev's own check no longer touches it.
  May a mod's build reach such a path at all? Options: (a) refuse absolute or outside paths in
  `generate!`/include by scanning the source before the build (recommended: cheap, no build
  starts; undo: drop the scan); (b) run the build with no read access outside the mod and the
  cache (sandbox-exec on macOS; strong, platform-specific); (c) accept it as a known limit (the
  maker's own machine, their own mod). PO leans (a).

- S16/S17: should the host mark some MCP tools read-only (`readOnlyHint`), so harnesses like Codex
  call them without an approval setting? Options: (a) no hint; the template `AGENTS.md` documents
  the approval line per harness; (b) a mod declares read-only functions (a new `mod.toml` or WIT
  convention) and the host passes the hint on. Recommendation: (a). The hint is the mod's own
  claim, so a mod could mark a write read-only to skip approval, and (b) adds a format. Cost to
  undo: (a) nothing; (b) a format change in pkg, host and dev. S17 built (a).

## Human answers

- 2026-11-28: **Codex runs on GPT-6 Sol, not GPT-6.2 Sol** ("this is the way forward"). OpenAI's Daybreak troubleshooting page: GPT-6.2 Sol and Astra stay on standard safeguards under Daybreak Blue; GPT-6 Sol and Luna get its reduced refusals. GPT-6.2 Sol withheld S162 r2 ("This content can't be shown"); the same brief on GPT-6 Sol came through (approve). Pick it in the composer's model picker before each dispatch. The factory's rule: mboogerd/claude-plugin, deputy 4c.

- 2026-11-27 (to Deputy Paper): STEP6-REPO, option (a), "feel free to do it in my stead": Deputy Paper made https://github.com/civic-tech-pioneer/watane-factory (public, main, README only) at 08:00 UTC. Publisher and mod identity not yet named. Evidence #56 closed.

- 2026-11-26: S120's Door H (where the factory mods live, `spec/design/factory/TECHNICAL.md` 5, first row): **one combined factory mod in one new repository**, W29 kept, "for now". Isolation of the foreman's authority comes from agentic-harness profile narrowing at stage F, not a second mod. The next factory design or build slice writes it into that row with spec-change and settles the "Planning and foreman split" Door C row to match.

- 2026-11-26: the Codex `watane` environment's secrets will not be rotated; never ask again. mboogerd/claude-plugin#6 (10-minute Codex timer) merged by the human.

- **2026-11-25, the human (to deputy-5): no test run in a cloud (CI or a cloud team) calls an external provider** (OpenAI, Jev or another judge, GitHub's API, any real service). Unit and integration tests use fakes: dev's scripted Responses server and scripted judge, fake `claude`, fake-watane. A check against a real provider is a deputy probe on the Mac (V7 on Jev). Checked 2026-11-25: no workflow in watane or agentic-harness passes a provider secret to a test.

- 2026-11-24 Codex Daybreak (authorized security work) is set up ("Done with daybreak"): security rounds go to Codex again; a round it withholds after this goes to the human.

- 2026-11-24 host 18 **Q38: (b)**, the human ("b is fine for q38"). A workspace or account consent is kept per (origin client, `via` = the provider's identity, folder or account); a new provider asks again. S103 built (b); its next answer round writes host 18 Q38 as decided by the human, 2026-11-24, and the 19.7 rows to match. agentic-harness TECHNICAL 8 already cites 19.7.

## Deputy decisions

(none in the veto window)

## Friction

- (2026-11-28, S162 r2 on GPT-6 Sol) the Codex `watane` environment had no Cargo in that task, so the review's gate ran only its structural checks; r1 on GPT-6.2 Sol ran both gates. CI covered the build and tests. Watch the next Codex review; if Cargo is missing again, the environment's setup needs it.
