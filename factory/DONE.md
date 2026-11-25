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
