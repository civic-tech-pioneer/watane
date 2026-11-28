# Plan

The deputy's state, and only the present (deputy skill, section 0): CI fails this file past 500
lines. History is in `factory/DONE.md`. Upkeep work is in GitHub issues labelled `upkeep` (with
`guard`, `core` or `polish`) and `flaky`. Deputy decisions past their veto window are written into
the spec that owns them; those from before 2026-11-27 wait in `factory/decisions/LOG.md` for that
(issue linked there).

## Deputy handover

From Deputy Paper to Deputy Scissors, 2026-11-28 ~09:00 UTC, after three merges (watane S162, agentic-harness S151, S163). One watcher ran over all three repositories (`watch-all.sh <state> 600 civic-tech-pioneer/watane civic-tech-pioneer/agentic-harness civic-tech-pioneer/watane-factory`); stopped. **Codex runs on GPT-6 Sol** (Human answers): every new Codex task starts on GPT-6.2 Sol; pick GPT-6 Sol by name before typing.

- **watane S164** (PR #123, builder codex on GPT-6 Sol, task https://chatgpt.com/local/6b0a821f-0c44-83eb-8924-44168a8db063; reviewer claude). CI was red before review: Windows fixed (b4be4ae); macOS `manage::ac3_s82` (#106) failed on every run twice, while main passes it. **Cut** (Deputy Paper, ~08:55 UTC): revert the slice's #106 changes (`app/tests/it/manage.rs`, `app/tests/support/driver.rs`) to main, keep the rest, #106 stays open for a slice that probes on a macOS runner; sent to the Codex task, awaiting its push `S164 r0b: answers`. Then: CI all green, Claude review r1 (routine, review-lead.md in the brief). Record the cut under Cuts in the next report.
- **watane-factory JF1-1** (PR civic-tech-pioneer/watane-factory#4, builder claude routine `trig_0115fFChXSGujUeQtnujmzEk`, reviewer codex). r1 Codex review (https://chatgpt.com/local/6b0a899e-5a40-83eb-9766-6fe08082d057) posted on the PR: changes (1), F1.1 a version-2 draft saved as version 1 (logic/src/snapshot.rs:140, lib.rs:97), P1.1 clipboard fallback. **Next: dispatch the answer r1 to a new Claude routine** (one writer: the build session went idle at delivery). Its PLAN.md is in that repository.
- **agentic-harness**: S163 merged (551b4f46). No slice in flight there; its next lane sketch is a design-lane slice: #125 (S163 A1.1 line), the S151-H H2 answer (`mods:agent` import limit) and the `spec`-labelled upkeep issues among #97 to #124, all in `design/`.
- **Waiting on the human:** plugin PR mboogerd/claude-plugin#13 (Codex on GPT-6 Sol in deputy 4c); watane-mods#10 (adopt.sh and factory-ci.yml with watane's guardrails); the local watane-mods checkout holds uncommitted edits plus the `[watane-factory]` row in projects.toml and one unpushed commit (bb10db8).
- **Friction:** watane's own foreman skill has no section 6 (answer a review round); agentic-harness and the plugin do. Until watane syncs it, spell the answer steps out in watane briefs.

## Slices

The main line: step 11, the agent layer; its next slice, JF1-1, is in flight in civic-tech-pioneer/watane-factory (its PLAN.md; routine `trig_0115fFChXSGujUeQtnujmzEk` 07:35 UTC, builder claude).
The human chose (2026-11-21, "the bridge") to build the agent layer next, driven by the collection's
product owner (watane-mods `/po`, `factory/config.toml` here; watane keeps its own foreman, agents,
check.sh and CI). The design lives in `watane-mods/agentic-harness/design/` (VISION, FUNCTIONAL,
TECHNICAL; N1 to N11 and R1 are the human's answers there). Its TECHNICAL 15 is the checklist of
watane changes the human accepted (N3); TECHNICAL 17 gives each stage its watane part and its
agentic-harness part. A stage's watane slice goes first; the agentic-harness slice that uses it
names the same interface in its sketch and fakes it until this one merges.

- [~] **S164 (upkeep) the four flakes, then app.** In flight (cloud, builder codex, reviewer claude; Codex task https://chatgpt.com/local/6b0a821f-0c44-83eb-8924-44168a8db063 on GPT-6 Sol, 07:47 UTC). Sketched 2026-11-28 by Deputy Paper; builder codex by rule 3 (the last two handed out: watane S162 claude, agentic-harness S163 codex). Issues, guards and flakes first: #105, #106, #108 (app tests: `machines`, `manage`, `support_selftest`, macOS) and #107 (`host/tests/it/streams.rs`, Windows): find each cause from its CI log and fix the test or the code, so it passes 20 runs in a row on its OS's CI job (say how it was checked; a cause in a product path is a fix, not a retry). Then app: #73 (channel rows show words, not raw transport text), #72 (`Core::mods` stops re-reading every related channel view each refresh), #82, #83, #102, #71 (tests and tidy), #103 (the Updates card hint, if the case still occurs; else close it with why). The build team closes an issue already fixed on main with its commit; the PR body says `Fixes #n` for each other. Outcomes: each issue's check (a test or a CI run, green on all four jobs). Touches: `app/`, `host/tests/it/streams.rs` and, only if a flake's cause is there, host's stream code; `dist/` tests for #83 P1.1; not `host/src/agent/`, `dev/` (local work in progress on the Mac), no WIT, no interface or error-code change. Protects: *guard* (the flakes), *polish* (the rest). Size: an upkeep slice. CI red before review (F1.ci): Windows fixed in b4be4ae; #106 cut by Deputy Paper 2026-11-28 (reverted to main, stays open).

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

- 2026-11-28 (to Deputy Paper): **JF1-ADOPT, yes to both**: civic-tech-pioneer/watane-factory adopted into the collection (adopt.sh, c32ad56 there; a `[watane-factory]` row in the local watane-mods `projects.toml`), mod slug `factory`, signed with the human's own maker identity. The human adds the repository to the Codex `watane` environment; until then both of its teams are Claude. JF1-1 is in flight there (its own PLAN.md).

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
