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
