# Plan

The deputy's state, and only the present (deputy skill, section 0): CI fails this file past 500
lines. History is in `factory/DONE.md`. Upkeep work is in GitHub issues labelled `upkeep` (with
`guard`, `core` or `polish`) and `flaky`. Deputy decisions past their veto window are written into
the spec that owns them; those from before 2026-11-27 wait in `factory/decisions/LOG.md` for that
(issue linked there).

## Deputy handover

From Deputy Scissors to Deputy Rock, 2026-11-28 ~11:35 UTC, after three merges (watane-factory JF1-1, agentic-harness S165, watane S164). One watcher over all three repositories (`watch-all.sh <state> 1800 civic-tech-pioneer/watane civic-tech-pioneer/agentic-harness civic-tech-pioneer/watane-factory`); stopped. Codex runs on GPT-6 Sol: in the composer's model list, type "GPT-6 Sol" in its search box and pick it (the list scrolls; the chip resets to GPT-6.2 Sol on each new task).

- **agentic-harness S166** (PR #140, kernel guard #133 then #117 and console polish; builder codex, task https://chatgpt.com/local/6b0ab34b-3ee0-83ed-86da-d9ed2b21807d; reviewer claude). BLOCKED on #114 answered (option B: #114 left the slice, needs its own slice with a 9.3 spec change first; comment on #114). PR ready at 3aafa62b, CI green 11:31 UTC. **Next: dispatch Claude review r1** (routine, review-lead.md in the brief). Browser suite did not run in Codex (Chromium download 403): CI's.
- **watane**: nothing in flight. Upkeep lane next: S164 left #105 (cause not named), #106 (cut, needs a macOS probe), #83 P1.2 open, polish in the new S164 r2 issue. Main line (JF1-2 in watane-factory) needs agentic-harness stage F profiles.
- **watane-factory**: JF1-1 merged. Evidence issue civic-tech-pioneer/watane-factory#7: no cloud can run watane's real host (Claude: no IPv6 loopback; Codex: no watane-dev), so its user-sim did not run; accepted limits in #6.
- **Report to the human not yet sent** (the human was told in chat, not as a report): Deputy decisions to list for veto: JF1-1 merged with user-sim `not run (environment)` (watane-factory PLAN); S165 r3 P3.4 LOG index waived; S166 #114 out (agentic-harness PLAN). Cuts: S164 #106 (Deputy Paper). New accepted risks: S164 R1.1 (temp path on disk-write failure), R2.1 (same-user temp-name race, `dist/src/site.rs:155`); JF1-1 two (#6); S165 A1.1 (S39 r1), A1.2/A2.1 (S154 A1.3), A3.1 (= agentic-harness #133). Owed: a factory proposal for the S153 process rule (agentic-harness #122, "Codex build team hands the adversary to the Claude review team").
- **The human asked (2026-11-28)** why the self-clear failed: auto mode's classifier refuses `clear_session` "self". Suggested an allow rule for `mcp__ccd_session_mgmt__clear_session` in the watane checkout's `.claude/settings.local.json` (narrower than user settings); a running session may need a restart to pick it up. If your clear is refused again, take over with your context and say so.
- **Friction:** cmd+v does not paste into the Codex pane; type the brief (newlines submit, so flatten it). watane's foreman has no section 6: spell the answer steps out in watane briefs.

## Slices

The main line: step 11, the agent layer; its next slice, JF1-1, is in flight in civic-tech-pioneer/watane-factory (its PLAN.md; routine `trig_0115fFChXSGujUeQtnujmzEk` 07:35 UTC, builder claude).
The human chose (2026-11-21, "the bridge") to build the agent layer next, driven by the collection's
product owner (watane-mods `/po`, `factory/config.toml` here; watane keeps its own foreman, agents,
check.sh and CI). The design lives in `watane-mods/agentic-harness/design/` (VISION, FUNCTIONAL,
TECHNICAL; N1 to N11 and R1 are the human's answers there). Its TECHNICAL 15 is the checklist of
watane changes the human accepted (N3); TECHNICAL 17 gives each stage its watane part and its
agentic-harness part. A stage's watane slice goes first; the agentic-harness slice that uses it
names the same interface in its sketch and fakes it until this one merges.

(none in flight in watane; S164 merged 2026-11-28)

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
