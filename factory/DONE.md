# Done

Sketches of merged slices, moved out of PLAN.md (deputy skill, section 6). Newest last.

- [~] **S119 (upkeep) S111's polish in host and dev, S118's, and pkg's reserved Windows names.** (in flight, cloud,
  builder codex, reviewer claude; rule 3: last two S118 claude, S117 codex; the upkeep lane, sketched 2026-11-25 by
  deputy-3 from Leftovers; Codex task https://chatgpt.com/local/6b067827-da14-83ed-ab43-79bfbda7704a, dispatched 05:20 UTC; DELIVERED 05:59 UTC as PR #35, final head 7264e38 (+640/-102, all four CI jobs green; item 4 was already on main from S112, a red test added); r1: Claude review, routine `trig_01QxsQGcSfzYy1Hjj5buRF2r`, fires 06:42 UTC.) Outcomes, each item's check: (1) S111 r1 P1.1: connect and TLS handshake each get their own
  `idle-s` in `host/src/agent/https_stream.rs`, with a test each; P1.2 to P1.5, P1.11, P1.14 (host tests, the
  reset-before-head message, one sentence each in host 19.4 for the accepted R2 and R5); (2) S111 r1 P1.6 to P1.10,
  P1.12, P1.13 in `scripted-responses` and dev TECHNICAL; (3) S118 r2 P2.1 and P2.2; (4) S110 R2: pkg's `grammar`
  refuses Windows reserved device names on every platform, red test first. Touches: `host/`, `dev/`, `pkg/`. No boundary
  moves (item 4 narrows what a package may name; adversary not run). Protects: *polish*, item 4 *guard*.

- [~] **S120 Design: the factory as mods (VISION step 6).** (in flight, cloud, builder codex, reviewer claude; rule 3:
  last two S119 codex, S118 claude; the main line, sketched 2026-11-25 by deputy-4; Codex task https://chatgpt.com/local/6b068ece-9e90-83ed-8b9e-480c43b7b077, dispatched 07:32 UTC; DELIVERED 07:35 UTC as PR #39 at c737b8f (+269, spec only); r1: Claude review, routine `trig_011JchRYiPCWpuvBDTaeAbJp`, fires 07:38 UTC; r1 (07:50): 4/6, changes (4), all text fixes that must name a gap and leave it open (F1.1 who writes `access/` for a mod identity, F1.2 the shell provider is not there yet, F1.3 JF1 waits on E and F, F1.4 W29: one repository per mod); deputy grade 4/6; answer r1 to the same Codex task, 07:53 UTC; r1 answered at 5e5fd89 (all 10 fixed); r2: Claude review, routine `trig_014XvUijaZ5BWfoogTczqGcZ`, fires 07:57 UTC; r2 (08:00): 0/1, approve (P2.1, this story's wording, fixed by the deputy); waits on CI; spec-check not run before the sketch: the slice writes a new draft and the review checks it against the chain.) Spec documents only, no code. The
  story: a person steers coding agents from mods in her own host: the factory's planning role keeps the plan and sketches the next
  slice, its foreman role has an implementer build it and another vendor's agent review it, she approves each merge, and the
  factory proposes changes to its own mods, which she installs like any other mod (VISION step 6, "changes itself").
  Outcomes, each checkable by reading: (1) a new subsystem design (VISION, FUNCTIONAL with journeys, TECHNICAL) under
  `spec/design/factory/` as a draft, built only on the host and agent-layer interfaces as written (host 19, agentic-harness
  design TECHNICAL 2.1 and 9); (2) the map from today's factory (`factory/*.sh`, `PLAN.md`, the deputy, foreman and
  review-lead roles, routines and Codex tasks) to mods, toolsets and consents, saying what moves in and what stays out;
  (3) every Door C or H point listed for the human with options, consequences and a recommendation: where the mods live (a
  new repository is a new project, Door H), N5 (the implementer's shell: Codex app-server or the Claude shell grant), how
  the factory changes itself without widening its own grants, and the V7 statements it needs; (4) the order of work after
  it: the first build slice is the smallest journey that runs one real slice through mods, and which agentic-harness stage
  (E, F) each later part waits on. Touches: `spec/design/factory/` (new), one line in `spec/VISION.md` step 6 and
  `spec/TECHNICAL.md`'s order of work pointing at it; never a subsystem's code. Constraints: no change to a VISION goal or
  boundary (that is the human's); open points stay open in the draft, not decided. Boundaries: the factory mods' grants and
  consents are a W39 matter: the design names them; no adversary (no code). Size: a design slice. Protects: *core*.

- [~] **S121 (upkeep) polish in dist, log and pkg (S105 r5/r6, S108 r2, S109 r2, S119 P1.4).** (in flight, cloud, builder
  claude, reviewer codex; rule 3: last two S120 codex, S119 codex; the upkeep lane, sketched 2026-11-25 by deputy-4 from
  Leftovers; routine `trig_01UpZcrvSd1d7y3Mtwi5LMtD`, fires 07:32 UTC; DELIVERED ~07:50 UTC as PR #40 at 2ab4bf3 (+166/-2); r1: Codex review https://chatgpt.com/local/6b0693d8-2064-83ed-afd5-e27b8632b05c, dispatched 07:55 UTC; r1 (07:58): 2/1, changes (2) (P5.4 names the first base, not the redirecting one; P6.3's test has no listed base); both stand, deputy grade 2/1; posted on the PR; answer r1: Claude routine `trig_01AhQF721nBk4yGAWUJgUNgr`, fires 08:19 UTC; r1 answered at 233f731 (both fixed, P1.4 too); r2: Codex, same task, dispatched 08:31 UTC; r2 (08:31): 0/0, approve; ubuntu's app fixtures flake rerun once (Leftovers, app).) Outcomes, each item's check (re-check each line against main first): (1) S105 P5.3, P5.4, P6.1 to P6.3 in
  `dist/` (a test each for P6.1 to P6.3; P5.3 a fix or one line in dist TECHNICAL 4.4); (2) S109 P2.1 to P2.3 in `log/`
  and log TECHNICAL; (3) S108 P2.1, P2.2 in `pkg/`; (4) S119 P1.4 in `pkg/tests/it/package.rs`. Leaves out S108's spec
  questions and S109's crash-before-rename items. Touches: `dist/`, `log/`, `pkg/` and their TECHNICAL; never `host/`,
  `dev/`, `spec/`. No boundary moves (tests and messages only; adversary not run). Protects: *polish*.

- [~] **S123 (upkeep) Alice's install, upgrade and uninstall say the true thing (install/ polish from S89, S50b-fix,
  S50b-fix2, S50c, S94).** (in flight, cloud, builder codex, reviewer claude; rule 3: last two S122 codex, S121 claude;
  the upkeep lane, sketched 2026-11-25 by deputy-4 from Leftovers, install; Codex task https://chatgpt.com/local/6b06a324-a2e4-83eb-9de4-9f96d259204d, dispatched 08:59 UTC; DELIVERED 09:17 UTC as PR #41 at 46f8c1b (+492/-136; receipts gain a record of folders install.sh created, old receipts leave them alone); r1: Claude review, routine `trig_01DEjihs5BriDzzvy1LQxiRG`, fires 09:19 UTC; r1 (09:26): 1/6, changes (1) (F1.1 shellcheck cannot parse install.sh, ubuntu red); receipts checked: old ones still read, no folder removed that install.sh did not make; P1.1, P1.3, P1.6 taken, P1.2, P1.4, P1.5 to Leftovers; answer r1 to the same Codex task, 09:28 UTC; r1 answered at 776b61d; r2: Claude review, routine `trig_0199wSgiT94m4CPYGbPZtt6a`, fires 09:31 UTC; r2 (09:41): 0/1, approve (P2.1 to Leftovers); ubuntu red on host's mcp::ac7 `Busy` flake, outside the slice: rerun once.) Outcomes, each item's check (re-check each
  line against main first; drop one already fixed): (1) every line install.sh prints after a Ctrl-C, a refusal or a
  skipped entry is true and names `~`-relative paths: the post-undo "still works", "uninstalled" with skipped entries,
  the install-it-later line naming `WATANE_RUSTUP_URL`'s target, a Ctrl-C at each question saying whether watane is
  installed, no `^C` glued to the hint; (2) uninstall and interrupted installs leave no empty folders they made
  (`~/.local/bin`, `share/watane-dev`, `~/Applications`); (3) `--status | head -1` exits quietly (no `printf` broken-pipe
  text); (4) rustup's own output stays outside watane's frame and a rustup failure names its step; (5) S94's
  `check_foreign` `pid_gone` unit case. A test per item where install's suites can run it (Linux and macOS CI).
  Leaves out: the copied-home receipts (a spec call), the VM consent click, Windows. Touches: `install/` and its
  TECHNICAL only. No boundary moves (messages and cleanup of folders install.sh made; adversary not run). Protects:
  *polish*.

- [~] **S122 Alice gives a checked provider its judge key (host A5, app 19.4; agentic-harness stage E, watane half).**
  (in flight, cloud, builder codex, reviewer claude; rule 3: last two S121 claude, S120 codex; the main line, sketched
  2026-11-25 by deputy-4: step 6's first build waits on stages E and F (S120), and E's watane half needs no real judge. Codex task https://chatgpt.com/local/6b06994d-5934-83eb-9d1f-da9025713051, dispatched 08:15 UTC; PR #42 opened ~09:28 UTC as a draft; BLOCKED 09:52 at 83b71eb: the builder held delivery on cloud reds that also fail on main (app look::windows window height, host stage A load); GitHub CI at d235a2d was red on all three OSes on dev `scripted_host::ac2_case_19_every_refused_form_reaches_no_server`, a test of this slice's area: the answer waits for CI on 83b71eb; answered 10:09 UTC; DELIVERED at cadba83 (+1814/-82, CI green on all four); r1: Claude review routine `trig_01DkHPfSzNr3as2C8c6Aapde`, fires 11:20 UTC, adversary on.)
  The story: in the manager's page for a checked provider, Alice enters a judge key and the judge's origin; only that
  provider's requests to that origin carry it, and removing it ends its live streams. Outcomes, on the real programs
  with dev's scripted judge origin (dev 18.3): (1) Alice sets and removes a judge key for an installed provider in the app
  (`judge_set`, `judge_remove`, app 19.4, 19.5); the key file is named by the provider's identity, 0600, written
  atomically; (2) a request with `account: "judge"` from that provider reaches the scripted judge with the key, and from
  the unchecked provider, another publisher's mod or a preview it ends `no-account` with no request counted (host 19.4,
  S59); (3) `judge_origin` parses once (case and default port folded; a path or query refused `bad_request`), no
  redirect is followed; (4) removing the key ends a live judge stream `gone` (HOST-F-72); at startup a key file of no
  installed mod is deleted (app 19.4). Interface: `watane:host@0.1.0` H3 accounts as host 19 states it (no WIT change
  expected; one goes to the deputy first); the judge's wire shape stays the checked provider's (dev 17). Touches:
  `host/src/agent/`, `app/` (accounts, the judge page), `dev/` only if the scripted judge needs it; never `dist/`, `log/`,
  `pkg/` (S121). Boundary: W39, "a judge key goes only to its judge" (host 18, app 2072): adversary on. Leaves out: the
  checked provider itself and V7 (agentic-harness, needs a real judge account). Protects: *core*.

- [~] **S124 (upkeep) a failed fetch blames the true cause (net, node, dist), and net's budget flake.**
  (in flight, cloud, builder claude, reviewer codex; rule 3: last two S122 codex, S123 codex; the upkeep lane, sketched
  2026-11-25 by deputy-5 from Leftovers outside S122's host/, app/, dev/; spec-check not run: error text and tests only; routine `trig_01S1fjM2MXnYjYVirKm928Le`, fires 11:21 UTC.; DELIVERED ~12:48 UTC as PR #43 at b08f7ea (+723/-124, dist, net, node); r1: Codex review https://chatgpt.com/local/6b06d9d5-a524-83eb-8120-942a8b00d07b, dispatched 12:53 UTC; r1 (12:59): 3/2, changes (3), validator FAIL, adversary HELD, user-sim REJECTED; Windows CI red on the slice's own dist test (F1.ci); rulings: F1.1, F1.2 stand, F1.3 regraded polish (a shutdown in a narrow window); answer r1: Claude routine `trig_01C1eDEYf3zyHCToLdDiPCRN`, fires 13:31 UTC.)
  Outcomes, each item's check, on the real programs with a test per item: (1) (guard) [S100 CI] net's tests that share
  the connection budget (`budget::s9...`, and each like it) no longer flake on timing: 20 runs green per OS in CI, as
  S106 fixed `pair::s4`; (2) [S87 adversary, S87 user-sim] a local I/O or holder error during a download reads as local
  (no "unreachable <full machine>", no temp path), and a failed install whose real cause is the full machine's bad copy
  says so instead of "channel ... is not followed" (net 5.9 gains the sentence); (3) [S92] with every full machine off,
  a light fetch says no full machine is reachable, not "is not followed"; a holder refusing a channel, a bad copy from a
  machine that answered, and `DistError::Io`'s top line each name the true party; a local unpack failure is not
  `BadPackage`; the full-disk text names the temp path once. Touches: `net/`, `node/`, `dist/` (error text and their
  tests); never `host/`, `app/`, `dev/` (S122). App words that quote these errors are left for the next app upkeep.
  Leaves out the S92 mutant gaps and node's two chain helpers. Protects: *polish* (2, 3), *guard* (1). Remove the
  items from Leftovers when it merges; drop the stale [S111 CI] reach line (S115 fixed it).

- [~] **S125 (upkeep, guard) host's and app's flaky tests, and S122's judge polish.**
  (in flight, cloud, builder codex, reviewer claude; rule 3: last two S124 claude, S122 codex; the upkeep lane, sketched
  2026-11-25 by deputy-5 from Leftovers `### host` and `### app`; spec-check not run: tests, one app 19.4 rule; Codex task https://chatgpt.com/local/6b06ed1f-f930-83ed-af6e-ed7b0fdab06b, dispatched 14:19 UTC; DELIVERED ~15:25 UTC as PR #44 at 19e79d5 (+606/-196, host, app); r1: Claude review routine `trig_018fU3rRBVmPYN61MLLEJ4Ui`, fires 15:31 UTC, adversary on the judge parser and the home lock.)
  Outcomes, each item's check: (1) (guard) the `Busy("/tmp/.tmp…")` reopen after `drop(host)` (host
  `mcp::ac7_toolset_grammar_and_persistence`, app `fixtures::ac5_committed_fixtures_call_subscribe_and_keep_state`): find
  the one cause (the lock's release on drop) and fix it in the host or the test rig, with a test that reopens at once;
  (2) (guard) host `streams::ac10_hand_off...` (`Some(2)` vs `Some(3)`) and `adv_s96_timer_cancelled_while_the_firing_waits_still_fires`
  on Windows: find each race and fix it; the consent `ac5` and `https_stream::ac5` lines first check whether S118's rig fix
  already covers them (then drop the lines); each guard: 20 runs green per OS in one CI run; (3) [S122 R1.1] app 19.4:
  the judge section stays on the manager page while a key is kept, even after `claude-code` is withdrawn, so Alice can
  remove it (app TECHNICAL and FUNCTIONAL say so; a test); (4) [S122 r1] P1.1 to P1.8 (Leftovers `### app`), each where
  cheap. Touches: `host/` (tests, the lock on drop, `agent/accounts.rs`), `app/` (judge page, tests); never `dist/`,
  `net/`, `node/`, `log/`, `pkg/`. Protects: *guard* (1, 2), *core* (3), *polish* (4). Remove the items from Leftovers
  when it merges.

- [~] **S126 (upkeep, guard) host's last flakes and its S125/S119/S118 polish.** (in flight, cloud, builder codex,
  reviewer claude; rule 3: last two S125 codex, S124 claude; the upkeep lane, sketched 2026-11-25 by deputy-6 from
  Leftovers `### host`; Codex task https://chatgpt.com/local/6b07276a-d24c-83ed-bada-23e64aa57056, 2026-11-25 ~17:25 UTC; DELIVERED as PR #45 (+506/-163); r1: Claude review routine `trig_01QxAbH72mt7AWxKpHQdzJYJ`, fires 19:08 UTC (the builder ran no user-sim or adversary: the review does).) Outcomes, each item's check: (1, guard) the three flakes of S125 r1 (Windows
  `adv_s96_timer_set_across_remove_and_reinstall_carries_the_removed_origin` adv_s96.rs:731, `https_stream::ac7_a_consumer_that_defers...`
  streams.rs:71, macOS `runtime::ac11_s11_restart_preserves_links_and_provider_state` runtime.rs:534): each race found and
  fixed, no wall-clock assertion left on a loaded runner, 20 runs green per test on its OS in CI; (2, guard) S96 Q47:
  `remove` waits for a busy mod's call lock with a deadline and says so; S96 Q48: a startup sweep of timer rows left by a
  crash between `remove`'s catalog save and its timer rewrite (red test first); (3) R1.4 ruling (deputy, below): host 19.4
  names judge origins with an unspecified, loopback or private literal under "Not protected" (the participant types the
  origin and the provider needs a matching consented Http grant); no code change; (4) R1.2, R1.3 in host TECHNICAL 3.1;
  (5) polish: S125 P1.1 to P1.10, S119 P1.1 and P1.5, S118 r2. Touches: host, app (fixtures.rs, words.rs, ui/app.js,
  TECHNICAL 19.4). Constraints: no format or interface change; S113's kept-storage line is already done by S118 (drop it
  from Leftovers). Protects: *guard* (1, 2), *polish* (rest). Size: an upkeep slice. spec-check: skipped (docs-only
  wording items; no new contract).

After step 8, the plan ends and VISION's own step 6 begins: the factory as a set of mods in the
host. It exists today as `factory/*.sh`, markdown briefs and sessions the product owner starts by
hand — outside the substrate, steering coding agents and changing itself, which is what VISION
describes except for where it runs. Moving it inside makes the thing that built watane the first
serious thing built on watane, and is the sharpest test of the premise: if the substrate cannot host
its own factory, the premise is weaker than it looks. Not sketched; it needs its own design slice.


- [~] **S129 A maker previews a checked provider against dev's scripted judge (host 19.9, dev 18.4; agentic-harness stage E's
  "done when").** (in flight, cloud, builder codex, reviewer claude; rule 3: last two S128 codex, S127 claude, but S127
  was Claude; the main line, sketched 2026-11-25 by deputy-6 from S127's question (agentic-harness PR #56, option (a)).
  Codex task https://chatgpt.com/local/6b073053-19c8-83ed-bff6-6bf7de4f307d, 19:02 UTC; DELIVERED ~19:40 UTC as PR #46 (+503/-86); r1: Claude review routine `trig_01EkdTKe86qd8kaf1wmkhMj1`, 19:43 UTC. Size: small, a reason: it closes stage E's outcome 1 on a real host.) Story: a maker runs `watane-dev serve` on a
  checked provider with the scripted Responses program; the preview's judge account answers from the program's judge
  origin, so a safe call runs and an unsafe one denies, with no key of the maker's and no real judge. Outcomes: (1) in a
  development preview with `--scripted-openai`, an `account: "judge"` request of an installed mod goes to the program's
  judge origin (`https://judge.test`) with a fixture key (the script's `judge_key`, else a fixed default), after every
  check of 19.4 except the account lookup, exactly as the OpenAI binding of 19.9 does; (2) the provider's page and the
  app show that preview judge as entered, so `judge_origin` and the `[http]` grant match `https://judge.test` (a test
  build of the provider lists it); (3) real homes and release builds refuse it, as 19.9's OpenAI binding; (4) dev's
  `serve` passes nothing new when a script has no plays (`judge_key` and `plays` are already script fields, dev 18.3).
  Touches: host (the 19.9 preview binding, 19.4's lookup), dev (18.3/18.4 docs, a serve test). Not: S126's files
  (adv_s96, streams/runtime tests, `remove`, timer sweep). Boundary: W39 "a judge key goes only to its judge" (adversary).
  No test calls an external provider. Protects: *core*.

- [x] **S130 (upkeep) install.sh's leftovers: what Alice sees around a Ctrl-C, `--status` and uninstall.** (in flight,
  cloud, builder codex, reviewer claude; rule 3: last two S129 codex, S127 claude; the upkeep lane, sketched 2026-11-25 by
  deputy-6 from Leftovers `### install`; Codex task https://chatgpt.com/local/6b073806-b674-83ed-b83e-d3e470c2605f, ~19:35 UTC; DELIVERED ~19:42 UTC as PR #47 (+367/-83); r1: Claude review routine `trig_01FEHfiVhzUVxTQfXgBV4xpg`, 19:44 UTC; r1: 3/10 findings, `changes (3)` (adversary BREACHED, user-sim REJECTED 16/17); answer r1 sent to the Codex task 2026-11-26 06:30 UTC; answered at f0bb068; r2: Claude review routine `trig_01LDyvGEoLtwUXXaK4MqRbdA`, 06:40 UTC; r2: 2/7 `changes (2)` (adversary HELD, user-sim 11/12; F2.1 a recovery regression on a linked `~/.local`, F2.2 its guards unpinned, narrowed to the reviewer's four cases; R2.1 accepted); answer r2 to the Codex task ~07:10 UTC; answered at 351fbb3; r3: Claude review routine `trig_01XceBDWgyNLak3gyGSoYv4B`, 07:25 UTC (cut point: r3 rules the minimal version if it is not approve).) Outcomes, each item's check (re-check each line against main first; drop the r3: Claude 1/4 `changes (1)` (validator RETURN on a missing test pin, adversary HELD, user-sim ACCEPTED 13/13); **cut** by Deputy Rock 2026-11-26 ~08:10: F3.1 one test case, P3.1 one 8.5 clause (covered by R2.1), P3.2 to P3.4 to Upkeep; answer in the same Codex task, ~08:05 UTC; answered at 7c54c0f (test + 8.5 clause only); r4: Claude review routine `trig_01JC6XxHTrmmPgTB8izGU6rk`, 08:08 UTC, scoped to the ruling.
  ones already true and say so): (1) S123 r2 P2.1, r1 P1.2, P1.4, P1.5; (2) S94: a `pid_gone` unit case, `install-cache-*`
  folders pruned; (3) S89, S50b-fix, S50b-fix2, S50c user-sim lines that still hold: a hint after every Ctrl-C (also at
  each question and in the first 0.1 s where a trap can exist), `--status` cleans leftovers and survives a closed pipe,
  paths printed as `~`, rustup's own output outside the frame, uninstall says "partly uninstalled" when it skipped
  entries; (4) `clean_mac::ac1_ac2_fresh_home_real_release` passes in a fresh worktree without `target/release`, or skips
  with a reason. Touches: `install/` only. Mac checks: the existing macOS CI job (`clean_mac`). Constraints: no change to
  the receipt or journal format beyond what S123 made; Windows stays report-only. Protects: *polish* (guard: none). Size:
  an upkeep slice.

- [~] **S133 Design: a mod learns whose authority a timer firing carries (host 19.6, agentic-harness TECHNICAL 2.1).** (in flight,
  cloud, builder codex, reviewer claude; rule 3: last two S132 codex, S131 claude; the fourth lane, from Unsettled
  (deputy-6, agentic-harness S128), sketched 2026-11-26 by Deputy Rock; Codex task https://chatgpt.com/local/6b07e871-2184-83ed-aaa7-1aaf7883e04d, 08:06 UTC; BLOCKED 08:07 in the task (no spec-change skill in its workspace), answered by Deputy Paper 08:33 UTC with the plugin's public raw URL, or AGENTS.md's chain rule; DELIVERED as PR #48 at 966099c (+43/-1, host/TECHNICAL.md, spec/TECHNICAL.md); r1: Claude review routine `trig_019mDW1p3MCoLcnqcdtqEf1M`, fires 08:35 UTC; r1: 2/13 `changes (2)` (F1.1 the WIT belongs in 19.1, F1.2 the fake's result; adversary HELD), answer r1 sent ~08:40 UTC into the Codex task; answered at fab7d4c (F1.1, F1.2 fixed, polish 7 fixed, 6 deferred with reasons); r2: Claude routine `trig_01PUvpAqrm78X15Nbk1JLSzT`, fires 08:46 UTC; the agentic-harness consumer half goes out after r1, so it builds on a reviewed shape; no spec-check: the slice writes the clause that is
  missing on both sides. Size: a design slice. It ends two known limits in agentic-harness: F6.1 and case 22's
  idempotent half.)
  Why: the host resolves a timer's origin client and keeps it to itself. A timer callback's caller is `host`, so the
  kernel cannot tell whose authority a firing carries, or whether that owner is still enabled. A restart pass has the
  same gap. The kernel then acts under its own origin, which is the F6.1 known limit.
  Outcomes (the review checks them in the documents): (1) host TECHNICAL 19.6 (and 2.1 where the WIT changes) gives a
  mod a read-only way to learn the resolved origin of its own timer firing, with its exact shape: a field, a function
  or a callback argument. It holds what a removal or disable rewrote (S62), and what a mod sees when the origin is gone;
  (2) an owner-scoped restart callback, or the reason not to have one; (3) the PR body holds the contract text that
  agentic-harness's `design/TECHNICAL.md` 2.1 and 15 rows will copy verbatim (its consumer half, sketched after S132
  merges: S132 edits that file); (4) spec/TECHNICAL 6: no authority widens; a mod learns only the origin of its own timers.
  Touches: `host/TECHNICAL.md` 19.6 and 2.1, `spec/TECHNICAL.md` 6 if a row changes, watane only. Not agentic-harness
  (S132 edits its `design/TECHNICAL.md`). No code.
  Constraints: design only, through the `spec-change` skill. List each Door C choice (the WIT shape, the callback) in
  the PR body with its options, for the human's veto. A new host export or callback is a public interface: prefer the
  narrowest (read-only, own timers only).

- [~] **S135 (upkeep) install.sh's last S130 and S123 polish.** (in flight, cloud; Codex task https://chatgpt.com/local/6b07eebe-eed0-83eb-b34d-113057e32064, ~08:40 UTC; PR #49 at 780f4d1 (+501/-37); r1: Claude routine `trig_01LZEhZuvsfGGpRWBzxGsMim`, fires 08:46 UTC; sketched 2026-11-26 by Deputy Paper, the watane upkeep
  lane; builder codex, reviewer claude by rule 3 (last two S134 claude, S133 codex); no spec-check: install's own
  3.x wording at most. Exempt from size: upkeep.)
  Outcomes, each item's check (re-check each against main first; drop the ones already true and say so):
  (1) S130 P3.2: `undo_swap` reverts through the receipt's records only for watane's own files, a test on the
  relink-revert edge; (2) P3.3: each unpinned guard (`install.sh` :238, :402, :506, :513, :807, :811, :1457 at
  c8eb2a1f) has a test that fails without it; (3) P3.4: after a crash the stale-lock message comes first and
  `--status` names the lock; kill point D lists its files at their real paths; a successful recovery says so; a
  pre-commit kill then `--uninstall` leaves no empty `share/watane*/` folders; (4) S123 P2.1 (the relinked-folder guards
  tested through `keep_created_roots`), P1.2 (receipts record a `~/.local/bin` or `~/.local/share` watane made, and
  uninstall leaves one the person made), P1.4, P1.5.
  Touches: `install/` (install.sh, its tests, `install/TECHNICAL.md` and FUNCTIONAL wording). Not host, app, dev.
  Protects: *polish* (Alice's words after a Ctrl-C) and *guard* (P3.3). The older From-S94/S89/S50 lines stay in Upkeep.

- [~] **S136 A mod's timer firing tells it whose authority it carries (host 19.6 build, S133's design), with host's and dev's judge polish.**
  (sketched 2026-11-26 by Deputy Paper, the main line: the provider half of agentic-harness F6.1; builder codex, reviewer
  claude by rule 3 (last two S135 codex, S134 claude); in flight, cloud, handed out ~09:20 UTC after S133 merged; Codex task https://chatgpt.com/local/6b07f9b7-3ff0-83ed-a6d1-e8b790c7606c; DELIVERED as PR #50 at f04f409 (+499/-184); r1: Claude routine `trig_01HWVVe4oneZKnnZD7xPZDdq`, fires 09:53 UTC.
  Size: the read is small, so it carries host's polish to make one slice (reason: the provider half unblocks
  agentic-harness's consumer half).)
  Why: S133 specified `timers.firing-origin() -> option<string>` (host 19.1, 19.6); the kernel needs it to act under the
  owner's authority on a firing instead of its own (agentic-harness F6.1, case 22).
  Outcomes (user-sim on the real host and `watane-dev serve`): (1) a mod's own top-level `fired` reads `some(<origin
  identity>)`, every nested call and every other entry point reads `none`; (2) after the origin client is removed or
  disabled, the firing reads the rewritten origin (S62, S66) and gives no previous origin, name or generation; (3) a
  cancelled or replaced timer never enters `fired`; (4) the Harness returns `none` and a `Fakes` entry overrides it;
  (5) S129 P1.1 to P1.8 and S126 r1 P1.1, P1.3, P1.4 (re-check each against main first).
  Touches: `host/src/` (timers, the WIT in `wit/host.wit`), `host/tests/`, `dev/` (judge polish, Fakes), host/dev TECHNICAL
  only where the build finds a gap. Not install/ (S135).
  Constraints: the shape is S133's, merged: do not change it; a gap goes back as BLOCKED. Adversary: a mod that tries
  to read another mod's origin, or a stale one after a removal (spec/TECHNICAL.md 6).

- [~] **S137 (upkeep, guard first) dist's local-address guards and install's S135/S123 polish.**
  (sketched 2026-11-26 by Deputy Scissors, the upkeep lane; builder claude by rule 3 (last two S136 codex, S135 codex),
  reviewer codex; in flight, cloud, routine `trig_01D2jzqZBQFVRMEmK7P1o3Ek` fires 10:01 UTC; DELIVERED as PR #51 at 6e2ab67 (+1065/-54; dist items (1), (2) already on main, dropped); r1: Codex review https://chatgpt.com/local/6b08113f-1a4c-83eb-9486-f3d94dd8207b, 11:00 UTC; r1: 2/1 (changes; CI green), answer brief as PR comment 11:21 UTC, ruled narrow; the build session did not wake on it; answer routine `trig_01YH4RzNA7aoQz8ePvTwzZFQ` 11:42 UTC; r1 answers 2ccb2b8 (3 fixed); r2: Codex follow-up 11:55 UTC; exempt from the size rule (upkeep). No spec-check run: items only, any spec gap goes through spec-change.) r2: Codex review on 2ccb2b8, posted 12:14 UTC by Deputy Rock: 1/1 (changes; F2.1 a regression in the r1 answer, ruled narrow); r2 answer routine `trig_014os5h91fnwn7DzF9tsca9t` fires 12:16 UTC; r2 answers at d5d2dcf (12:27 UTC, check PASS); r3: Codex review, follow-up in the same task, 12:29 UTC.
  Outcomes (each item's check, re-checked against main first; an item already fixed is dropped with a line):
  (1) *guard* dist's local ranges (5.9) also refuse 100.64/10, 198.18/15, 0/8, fec0::/10 and the NAT64 and 6to4 forms of
  a local IPv4 address (S105 adversary, deputy decision 2026-11-23), and `localhost.` with one trailing dot is local
  (S104 review); (2) dist S105 P5.3, P5.4, P6.1 to P6.3 (Upkeep list above); (3) install S135 r1 P1.1 to P1.14 of
  PR #49, P1.1 by making user-sim's red test `regress_s135_changed_during_install_to_the_home_has_no_bare_tilde` pass,
  P1.2 with tests that kill mutants 9 and 10; (4) install S123 r2 P2.1 and r1 P1.2, P1.4, P1.5.
  Touches: `dist/src/`, `dist/tests/`, `install/`, dist and install TECHNICAL only where a rule changes. Not host/ or
  dev/ (S136).
  Constraints: no new public interface; a guard never weakens. Adversary on (1): Eve's listed base or redirect aimed
  at each new range (spec/TECHNICAL.md 6).

- [~] **S141 (upkeep, guard first) host's S136/S126/S111 polish, the ac9 flake, and install's S137 rough spots.**
  (sketched 2026-11-26 by Deputy Rock, the upkeep lane; builder codex by rule 3 (last two S140 claude, S138 codex),
  reviewer claude; in flight, cloud, Codex task https://chatgpt.com/local/6b082b7f-3a70-83ed-872a-f8da2c5e3050, 12:55 UTC; DELIVERED as PR #52 at 31d9a25 (13:50 UTC, +376/-61; S129 items already done by S136; S136 P1.3 (WIT comments) and P1.7 deferred; user-sim and adversary not run); r1: Claude routine `trig_01248AmqwLvZQ4ZGrbzwhrTo`, 13:52 UTC; r1: 2/3 (changes: B1.1 user-sim and adversary not run, B1.2 the tightened backpressure bound flakes 2 in 18; ruled narrow), answer as Codex follow-up in the build task 14:08 UTC; r1 answers at 4d561f3 (14:16 UTC; adversary HELD, user-sim ACCEPTED, 128-event bound, P1.1 disagreed); r2: Claude routine `trig_01Fraz988Ln1GxbyzRMbtkxD`, 14:18 UTC; at merge: S136 P1.3 and P1.7 back to `### host` and `### dev` (deferred: WIT change, new interface), drop `### S129 polish`, S126 r1 P1.4, S111 P1.1; friction: review-lead.md missing, third sighting (proposed as a factory fix); spec-check not run: no spec rule changes beyond a doc line where an item names one. Upkeep is exempt from the size rule.)
  Outcomes (each item's check, re-checked against main first; an item already fixed is dropped with a line):
  (1) *guard* the ubuntu flake `claude::ac9_say_refuses_slash_commands_strangers_and_unknown_streams`
  (host/tests/it/claude.rs:587, left 0, right 1, S133 CI): the race found and fixed, or the assertion waits on the event it
  means, with the reason in the test; (2) S136 r1 P1.1 to P1.7 (PR #50; P1.1 finishes S126 r1 P1.1) in `### host`;
  (3) S111 r1 host P1.1 (connect and TLS handshake each get their own `idle-s`) and P1.2 to P1.5, P1.11, P1.14 where cheap;
  (4) install's S137 user-sim rough spots (`### install`); (5) the `### S129 polish` list is checked against main and
  dropped where S136 already did it.
  Touches: `host/src/`, `host/tests/`, `dev/src/preview/` (S136 P1.4, P1.7), `app/` only for S136 P1.6's shared text,
  `install/`, and each subsystem's TECHNICAL only for a doc line an item names.
  Constraints: no new public interface, no WIT change; a guard never weakens; a flake is never fixed by a retry alone.
  Adversary: install's boundary for (4) (spec/TECHNICAL.md 6).

- [~] **S146 Design: one combined factory mod, and the planning/foreman split (VISION step 6, factory TECHNICAL 5).** in flight (cloud, builder claude, routine `trig_013YXruSe31389Jz8oq5XEsq` 16:59 UTC; DELIVERED as PR #54 at 88451300 (17:06 UTC); r1 Codex task https://chatgpt.com/local/6b086754-2a08-83eb-93bf-2f0fb4a1d0e2 17:08 UTC; r1: 1/1, answer routine `trig_01ReH7M9AZg7vWHJD91g7Rmt` 21:40 UTC (ruling: state harness 9.7's delegation conditions as written, no new mechanism; P1.1 too); answers 726baf3f 21:47; r2 Codex task https://chatgpt.com/local/6b08a848-30e0-83eb-8ffc-61a3a8699063 21:50 UTC; reviewer codex; Deputy Paper, 2026-11-26; rule 3: S144, S145 were codex; Deputy Rock's correction: this slice needs no repository). Why: the human's Door H answer (Human answers, 2026-11-26: one combined factory mod in one new repository, W29 kept, "for now"; the foreman's isolation from agentic-harness profile narrowing at stage F, not a second mod) must enter the spec before the build slice after it, which needs the repository. Outcomes: (1) `spec/design/factory/TECHNICAL.md` 5's first row records the human's answer as decided, with its date and "for now", and §1/§3/§6 and the subsystem's VISION/FUNCTIONAL agree with one combined mod (one identity, one consent scope, separate pages); (2) the "Planning and foreman split" Door C row is settled to match: what one identity and one consent scope mean for the planning and foreman pages, how the foreman's narrower authority comes from a child profile at harness stage F (AGT-J5), and what stays open until F; (3) each other §5 row that the answer changes says so, and no other row is settled; (4) a reader of `spec/design/factory/` can name the repository's role (the mod's source, W29) without a slug or publisher being chosen. Touches: `spec/design/factory/` only (and a line in `spec/VISION.md`'s step 6 or `spec/TECHNICAL.md` only if they name two mods). Constraints: a design slice: no code; spec-change skill for each edit (it runs spec-check on the diff); no new repository, slug or publisher identity; the remaining Door C rows stay open; a design slice is exempt from the size rule. Door C: the settled split is listed first in the deputy's report for the human's veto.

- [~] **S143 (upkeep) dev's, pkg's and log's leftovers.** in flight (cloud, builder claude, routine `trig_018MVZ2Lv3UJxBAjmPpWdzqQ` 14:41 UTC; DELIVERED as PR #53 at 72f68c82 (15:33 UTC; pkg and log items already on main, dev only); r1 Codex task https://chatgpt.com/local/6b0851b7-f61c-83eb-8244-e11126313013 15:36 UTC; r1: 1/3 findings (16:56 local, user-sim REJECTED on the provider recovery `next`), posted to the PR; answer routine `trig_01TQfeJK1DLijx1czZTcqsbt` 16:01 UTC (rulings: blocking stands, all three polish cheap); F1.ci Windows unit test posted 16:05; answers 4365f69f 16:25 (F1.ci not mentioned); r2 Codex task https://chatgpt.com/local/6b085dcb-7894-83ed-8e52-7280ed1ba003 16:27 UTC; r2: 1/0 (F1.ci Windows test), answer routine `trig_01HzFcVy41PYf38iWD76Gs5S` 21:40 UTC (ruling: gate the serve half to Unix or assert the Windows refusal); answers a7fe8afd; r3 Codex task https://chatgpt.com/local/6b08aa5f-8fe4-83ed-95d1-1ea54c70c068 21:59 UTC; r3: approve 0/0 (posted), waiting on macOS and Windows CI; reviewer codex; Deputy Paper, 2026-11-26; rule 3: S141, S142 were codex). Outcomes, each its item's check: dev's `### dev` items S119 r1 (P1.2, P1.3, P1.6, R1.1), S114 r3 (three), S111 r1 P1.6 to P1.13, S39 (a killed run's `<id>.<pid>.tmp` is swept), S86's prose and rerun lines; pkg's S110 R2 (Windows device names refused on every platform) and S119 P1.4; log's S109 P2.1 to P2.3. Re-check each line against main first (lesson of S49) and drop what is already done. Touches: `dev/`, `pkg/`, `log/` and their TECHNICAL; not `host/`, `app/`, `dist/`, `install/`. Constraints: no WIT change, no new public interface (S136 P1.7 stays); a refusal message may change wording. spec-check skipped: upkeep, specs change by a line at most. Exempt from the size rule (upkeep).

- [~] **S147 (upkeep, guard first) host's leftovers and the `s100_fix12` flake.** in flight (cloud, builder codex, task https://chatgpt.com/local/6b08a704-1ddc-83eb-a0f0-010f4381606b 21:45 UTC; reviewer claude); DELIVERED as PR #55 at 3470813e (22:29 UTC, CI green); r1 Claude routine `trig_01UGxYuGxG8ufxPS4LocnR6z` 05:59 UTC (dispatch late: the Mac's network stalled for hours); sketched by Deputy Scissors, 2026-11-26; builder codex (rule 3: of the last two, S146 was claude and S145 codex). Outcomes, each its item's check: (guard) `s100_fix12_ac1_the_next_queued_line_is_not_started_after_the_deadline` passes 20 runs in a row on macOS CI, its cause named in the commit; `### host` items S126 r1 P1.1 to P1.3 (P1.5 is app/: leave), S111 r1 P1.1 to P1.5, P1.11, P1.14, S91's over-declared Content-Length answers 413 at once, S12a's probe-to-bind window and abrupt-disconnect test, S45's 413 drain, S44's 404 code. Re-check each line against main first (lesson of S49) and drop what is done. Touches: `host/` and its TECHNICAL, `.github/workflows/ci.yml` for P1.3 only; not `dev/`, `pkg/`, `log/` (S143), not `spec/design/` (S146), not `app/`. Constraints: no WIT change (S136 P1.3 stays in Upkeep), no new public interface; an error code may change only where host 4.4 already names it. spec-check skipped: upkeep. Exempt from the size rule (upkeep). Protects: *guard* (the flake), *polish* (the rest).

- [~] **S150 Design: JF1 can be built from the documents (step 6, the parts that do not wait).** Sketched 2026-11-27 by
  Deputy Paper (evidence #56: JF1 waits on harness F's factory profiles, not on the repository). In flight (cloud, builder codex, reviewer claude;
  task https://chatgpt.com/local/6b092596-e3e8-83ed-9478-10121134504d, 06:41 UTC; DELIVERED as PR #57 at 88fff76a 06:44 UTC; r1 Claude routine `trig_01JMa2B8UNi42cVXbdxoL5Go` 06:46 UTC; r1: 2/3, changes, both blocking stand, answer r1 into the Codex task; answers 27680a5f; r2 Claude routine `trig_01NgdtPThvsGwPRvdYebLemE` 06:57 UTC). A design slice in `spec/design/factory/` only: (1) the sketch snapshot Alice carries in JF1 (its fields: project,
  base, branch, scope, acceptance, decisions; how it binds to a plan revision) and the run record's states and evidence
  fields (TECHNICAL 1), so a build needs no further choice; settle the "Sketch handoff" row for JF1's manual snapshot
  only (Door C; the automatic link stays open); (2) the two pages' views and Alice's actions in JF1 as FUNCTIONAL
  outcomes a user-sim can check on the real app; (3) the build rows for JF1, sized for slices, each naming what it waits
  on (harness F's factory profiles; the repository, STEP6-REPO); (4) S146 r2 P2.1 (TECHNICAL 6 step 3 wording). Outcome:
  a reviewer can build JF1 from the documents once F lands; every open point is decided in them or listed for the
  human. Constraints: no change to the agent layer's contracts (host 19, harness 2 and 9); a Door C choice is listed
  first in the PR for the deputy. Touches: `spec/design/factory/`. Docs only. Size: a design slice (it settles a
  Door C row before code depends on it).

- [x] **S149 (upkeep, guard first) host, pkg, log and dist polish queued behind S147.** In flight (cloud, builder
  claude, reviewer codex; rule 3: S148 and S147 were Codex; routine `trig_01AAZGaXwoNLX8rH6CfuwUHt`, fires 06:40 UTC; DELIVERED as PR #58 at 3697902f (most items already fixed on main), CI green; r1 Codex review https://chatgpt.com/local/6b093686-b734-83eb-88aa-48203316104a 07:50 UTC; r1: 2/0 findings, 07:50 UTC (posted by Deputy Scissors; its verdict line read `another round`, taken as changes (2)); answer r1 routine `trig_01Cmj2k8JH4HuNNW89FpyqST` 08:01 UTC, rulings: F1.1 narrows docs/test name only, F1.2 helper checks its I/O); answers 80cf29a6 ~08:20; r2 Codex review https://chatgpt.com/local/6b093f58-a2d8-83ed-bc1a-8e949a2e5067 08:29 UTC; r2: 0/0 approve, but macOS red: F2.ci (the F1.2 helper fails on a reset during the write; plus an unrelated app support_selftest flake); answer r2 routine `trig_0117t8L18c1Awo5DqQ4xsuwR` 08:46 UTC; answers e9b8b78f 09:01; r3 Codex review https://chatgpt.com/local/6b094722-246c-83eb-b5ee-958ce19de048 09:03 UTC, a short check of F2.ci; r3: 1/0 (a reset with zero bytes passes without a 413); **cut at r3 by Deputy Scissors** (same helper three rounds): split into (a) an oversize body alone always gets 413 + close, (b) the pipelined check proves only no second response; answer r3 routine `trig_01FVxd68TV6LaZgXkmZDqc7Z` 09:17 UTC; answers 22f321f9 09:35; r4 worth it (Deputy Rock): the trend is 2, 0, 1, and r4 only checks the cut's two tests, no new scope); r4 Codex review https://chatgpt.com/local/6b094f28-355c-83ed-94e7-dd8e59ecf062 09:39 UTC). Sketched 2026-11-27 by Deputy Paper from Leftovers.
  Re-check each item on main first (S112 already closed the dist local ranges, the trailing dot and redirects to a
  local address); drop what is already fixed and say so in the PR.
  1. (guard, CI on main) `host::agent::claude::run::join_tests::s100_fix12_ac1_the_next_queued_line_is_not_started_after_the_deadline`
     (`host/src/agent/claude/run.rs:1498`) stops flaking on macOS: the cause named and fixed, not a wider timeout alone.
  2. host S147 r1 polish P1 to P3 (`### host`), S126 r1 P1.3 (dead S126 repeat steps in `.github/workflows/ci.yml`).
  3. pkg S119 r1 P1.4 and S108 P2.1, P2.2; log S109 P2.1 to P2.3; dist S105 P5.3, P5.4, P6.1 to P6.3 where still open.
  Each outcome is its item's check. Touches: `host/`, `pkg/`, `log/`, `dist/`, `.github/workflows/ci.yml`; not `app/`,
  not `spec/design/`. No boundary moves: no adversary unless a change alters what a check refuses. Protects: *guard*
  (item 1), *polish* (the rest). Size: an upkeep slice.
  DONE 2026-11-27: PR #58 merged (rebase) by Deputy Rock after r4 approve and green CI on 22f321f9.
- [x] **S155 (upkeep) dev and app polish from Leftovers.** In flight (cloud, builder codex by rule 3: of the last two
  handed out, S151 was Claude; reviewer claude; Codex task https://chatgpt.com/local/6b09542d-c6d8-83ed-baf2-213d64a8905e 10:02 UTC; DELIVERED as PR #59 at c8a3f200 ~10:44 UTC (+348/-78, most items already on main per the PR body); r1 Claude review routine `trig_01CYQQate4EhxENDExn8mmxd` 10:46 UTC; r1: 0/2 approve, adversary not run (no check refuses anything new), CI green, but no user-sim: user-sim routine `trig_015MG4kuWVwMdHZgzB7fxKuZ` 11:07 UTC before merge; user-sim REJECTED (1) ~11:22: U1.1 `new` under a regular file names no path (resolved_path returns internal); answer r1 to Codex in the same task 11:24 UTC, ruling: name the path, with the user-sim's regress_s155; answers dc96628d ~11:26 (U1.1 fixed, P1.1 deferred to Leftovers); r2 Claude review routine `trig_01Nrk1VZqojDCxAp1Bn2aNEC` 11:28 UTC, a short check plus user-sim; r2: 0/0 approve, user-sim ACCEPTED, CI macOS red F2.ci (test expected /private/var); answer r2 to Codex in the same task ~11:55, ruling: fix the test; answers 48fd5d6a (one test line); r3 Claude review routine `trig_01RdBzq7ejQAKp2vvJJ8kRdr` 11:53 UTC, a recheck of F2.ci only). Deputy Paper took over 2026-11-27 ~11:52 UTC. Sketched 2026-11-27 by Deputy Rock.
  Re-check each item on main first and drop what is already fixed; say so in the PR.
  1. dev (`### dev`): S119 r1 P1.2, P1.3; S114 r3 (the three `dev/src/preview/openai.rs` and `dev/TECHNICAL.md:2993`
     items); S111 r1 P1.6 to P1.10, P1.12, P1.13 (`scripted-responses`); the S86 prose and rerun items; S93.
  2. app (`### app`): S117 r1 (the three items), S116 r1 P1.1, P1.2, P1.4.
  3. pkg (`### pkg`): S119 r1 P1.4 and S110 R2 (Windows reserved device names refused on every OS) where still open.
  Each outcome is its item's check. Touches: `dev/`, `app/`, `pkg/`, `log/tests/` (S117's ACL oracle); never
  `host/src/agent/` or `dev/src/bin/fake-claude.rs` (uncommitted work in the main checkout), never `spec/design/`. No
  boundary moves: no adversary unless a change alters what a check refuses (S110 R2 does: then the Claude review runs it).
  Protects: *polish*. Size: an upkeep slice.
  DONE 2026-11-27: PR #59 merged (rebase) by Deputy Paper after r3 approve and green CI on 48fd5d6a.

- [x] **S158 (upkeep) install, log and dist from Leftovers.** In flight (cloud, routine `trig_01XPVb1dpgha7hKDx63sHEH3` 11:55 UTC; DELIVERED as PR #60 at ac434189 ~13:35 UTC (+1278/-167; user-sim ACCEPTED 3/3; S107 items and Q12 left in Leftovers, app/ and no `forget` API); r1 Codex review https://chatgpt.com/local/6b098684-e484-83ed-8d46-68ea29fba02f ~13:40 UTC, adversary on both boundaries). Sketched 2026-11-27 by Deputy Paper; builder claude by rule 3 (the last two handed out, S157 and S155, were Codex), reviewer codex.
  Re-check each item on main first and drop what is already fixed; say so in the PR. Guards first:
  1. dist (`### dist`): (guard) S112 r2: a redirect to a machine-local host name reaches a local port; check the resolved address at connect time (TECHNICAL DD-26 states the rule for literals; extend its sentence to resolved addresses in `dist/TECHNICAL.md` with the change). S107 P3.3 and the Updates card hint; S26 Q12 (log entries of unlisted channels) only if a `forget` fits log's existing API, else leave it.
  2. log (`### log`): S112 R1.1, the identity and store operations on the checked folder handle (`openat`/`renameat`/`unlinkat`, Unix and Windows).
  3. install (`### install`): S135 r1 P1.1 to P1.14 (P1.1 with the user-sim's `regress_s135_changed_during_install_to_the_home_has_no_bare_tilde`), S123 r1 P1.2, P1.4, P1.5 and r2 P2.1, and the S94 `pid_gone` unit case.
  Each outcome is its item's check. Touches: `dist/`, `log/`, `install/`; never `dev/`, `app/`, `pkg/` (S155), `host/src/agent/` or `demo/` (uncommitted work in the main checkout), `spec/design/`. Boundary: item 1 narrows what dist fetches and item 2 the log's file access (`spec/TECHNICAL.md` section 6), so the adversary runs. Protects: *guard* (1, 2), *polish* (3). Size: an upkeep slice.

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

## Stage coverage

One row per stage of each subsystem's build plan (its TECHNICAL "Build plan" table), with the
step that needs it. Status: `open`, `part` (what is missing), `done` (slice). A step is done only
when its journey passes and every stage of that step is `done`, or a deputy decision moves the
rest to a later step. Leftover `ponytail:` comments count as missing parts (`git grep ponytail:`).

| Subsystem | Stage | Step | Status |
|---|---|---|---|
| install | 1 script, 2 versions, 3 packer | 7 | done (S50b; macOS S50b-fix) |
| install | 4 workflow, 5 clean Mac (J7) | 7 | done (S50c; v0.1.0 checked in a clean VM) |
| net | N1 crate, codecs, `hello`, `heads`, `entries` | 8 | done (S60) |
| net | N2 device list, pairing, classes, allowances | 8 | done (S62) |
| net | N3 sync, gossip hints, blobs | 8 | done (S63) |
| net | N4 light machines, restore, reach | 8 | done (S67, S68) |
| log | L6 keyless (`open_keyless`, `identity()` an Option, `drop_identity`) | 8 | done (S61) |
| dist | S8 keyless, holder methods, `kept`, `Via::Kept`, `search_stored`, the lock rule | 8 | done (S61) |
| node | 1 holder | 8 | done (S64) |
| node | 2 machines (`told`, `need`, search, join, restore hints) | 8 | done (S70) |
| node | 3 program (`watane-node` join, run, status, relay, dht) | 8 | done (S72) |
| app | G own machines (J8) | 8 | done (S75) |
| pkg | 9 outlet functions, `World::collects` | 9 | done (S65) |
| host | 10 collect | 9 | done (S65) |
| dev | 12 collect fakes and answers | 9 | done (S65) |
| app | H extension points (J9) | 9 | done (S66) |
| pkg | 10 `main` in `mod.toml` | 10 | done (S74) |
| app | I main mod and manager | 10 | done (S74) |
| pkg | 11 `Grant::Manage` | 10 | done (S76) |
| host | 11 `watane:host/manage` | 10 | done (S76) |
| dev | 13 manage fake | 10 | done (S76) |
| app | J manage requests, J10 | 10 | done (S77) |
| pkg | 1 names, 2 meta/deps, 3 world/folder/grant | 1 | done (S1) |
| pkg | 4 component | 1 | done (S2) |
| pkg | 5 package (pack, unpack) | 1 | done (S7, S8: the host installs through it) |
| pkg | 6 add_provider | 2 | done (S11) |
| pkg | 7 release | 3 | done (S20) |
| host | 1 runtime core | 1 | done (S2) |
| host | 2 links and events | 1, 2 | done (S3, S11: links, subscribers, hops on the preview `install_folders` path) |
| host | 3 server and pages | 1 | done (S9); the subscription and revoke hostile rows in step 2 |
| host | 4 tools (toolsets, tokens, MCP) | 1 | done (S5, S58: the app lists toolsets, removes one and issues tokens) |
| host | 5 catalog and home | 1, 2 | done (S8, S11, S12a: plan, `Source::Folder`, grants, revoke, update, remove, home mode) |
| host | 6 program | 1 | done (S9: `--home` runs a participant home with no app) |
| host | 8 releases | 3 | done (S21b: `Source::Release`, S31, S32) |
| dev | 0 cli, outcome | 1 | done (S1) |
| dev | 1 template, new, check | 1 | done (S4) |
| dev | 2 allowlist, toolchain, builder | 1 | done (S8) |
| dev | 3 unit | 1 | done (S4) |
| dev | 4 contract | 1 | done (S4, S11: fakes and `deliver`) |
| dev | 5 preview, api, doctor, fake-watane | 1 | done (S6); `fake-watane` moved to step 2 (deputy) |
| dev | 6 deps, add, install | 2 | done (S11, S12b; `fake-watane` too) |
| dev | 7 publish | 3 | done (S21c) |
| dev | 8 search | 4 | done (S31; the app route S29) |
| app | A shell | 2 | done (S10) |
| app | B host | 2 | done (S12a) |
| app | F Linux | 2 | done (S18: arm64 Linux in Docker, `app/scripts/linux-test.sh`; front tests are macOS only) |
| app | C dist | 3 | done (S21b: publish, follow, `DistConfirm`) |
| app | D find | 4 | done (S25: APP-F-37, 39, 40; S29: APP-F-38, 43 to 46) |
| app | E build on | 5 | done (S37) |
| host | 9 binds | 5 | done (S34) |
| dist | S7 provider bases | 5 | done (S35) |
| dev | 9 build on | 5 | done (S36) |
| pkg | 8 repository | 6 | done (S41) |
| dev | 10 publish --push | 6 | done (S42) |
| dev | 11 new --from clone and fork | 6 | done (S43) |
| log | L1 to L5 | 3 | done (S19: L1 to L4; S21a, S21b: L5) |
| dist | S1 to S5 | 3 | done (S21a) |
| dist | S6 find | 4 | done (S28; r1 (13:38 UTC): 2/1, changes (2), Codex: F1.1 dist checks the truncated DNS result (16 kept), F1.2 SQLite may create channels.db outside the checked log dir; both guards, kept blocking by Deputy Scissors; posted on the PR; answer r1 Claude routine `trig_014eoWaLGdJUsJTSuPHfK9bT` 13:50 UTC; r1 answered at c3ffcbf5 (both fixed with regression tests, P1.1); r2 same Codex review task, dispatched ~14:25 UTC) |

## Leftovers

  DONE 2026-11-27: PR #60 merged (rebase) at 05478e35 by Deputy Scissors after r2 approve (0/0) and green CI on c3ffcbf5; accepted by the deputy: the builder's address-range risk (documentation networks, 192.0.0/24, Teredo, out of scope by dist TECHNICAL 5.9) and the log swap-and-swap-back race (two unusual events).
