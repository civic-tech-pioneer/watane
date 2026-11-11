# Plan

The product owner's state; only the product owner edits it, on `main`. A new session continues
from here. Newest digest line last.

## Slices

Status: `[ ]` sketched, `[~]` in flight (with its branch), `[x]` delivered.

Step 1 (Make, J1). A first cut; the product owner refines each sketch before it hands it out.

- [x] **S1 Hello page.** Delivered 2026-11-10 (main `bacecdc`). Sol runs: pkg 1, host 1, dev 2.
- [x] **S2 The page calls its logic.** Delivered 2026-11-10 (main `7eb55ad`). Sol runs: pkg 1, host 2, dev 2.
- [x] **S3 An edit keeps the data.** Delivered 2026-11-10 (main `c883171`). Sol runs: host 1, dev 1.
- [x] **S4 The agent fixes a failing test.** Delivered 2026-11-11 (main `91e8d0a`). Sol runs: S4-1 1, S4-2 1, S4-3 1.
- [x] **S5 The agent works the mod through MCP.** Delivered 2026-11-11 (main `196452e`). Sol runs: S5-1 1, S5-2 1.
- [x] **S8 Same mod, same id.** Delivered 2026-11-11 (main `d79e73e`). Sol runs: S8-1 1, S8-2 1.
- [x] **S6 J1: Alice gets her reading list.** Delivered 2026-11-11 (main `856b431`). Sol runs: S6-1 1, S6-2 1.

Step 1 done 2026-11-11: J1 passes (`dev/tests/journey.rs`); every step-1 stage is done or its rest
is in step 2.

- [x] **S7 Pack a mod.** Delivered 2026-11-10 (main `a740d78`). Sol runs: pkg 1.
  Its follow-ups are done in S15.

Step 2 (Run together, J2).

- [x] **S10 Alice opens the app.** Delivered 2026-11-11 (main `da54bb4`). Sol runs: S10-1 1, S10-2 1, S10-3 1, S10-4 1.
- [x] **S11 The recommendations mod calls the reading list.** Delivered 2026-11-11 (main `e6dc87e`). Sol runs: S11-1 1, S11-2 3, S11-3 1, S11-4 2, S11-fix1 1.
  Left for later (user-sim): `mod.dependency_cycle` and `mod.self_dependency` have `fix: null`;
  `check.providers[].path` is null for a local provider; `add --dir` resolves the provider path
  from `--dir`; a subscriber's `/events` also gets the provider's raw `event` messages. Host
  stage 5 (`Host::plan`, `Source`, confirmation) and S25 (revoke) are not built: S12.
- [x] **S13 Cheap tests.** Delivered 2026-11-11 (main `cf35f70`). Sol runs: S13-1 1.
  dev's default `cargo test` takes about 1.5 min (was about 9); the full run with slow tests about
  10 min (was about 30). `dev/TECHNICAL.md` 15.2 maps each DEV-S scenario to its test and level.
- [x] **S14 Quiet GUI tests.** Delivered 2026-11-11 (main `dca54b6`). Sol runs: S14-1 1.
- [x] **S15 Packages say what is wrong.** Delivered 2026-11-11 (main `3c441d7`). Sol runs: S15-1 2 (new, resume).
  Follow-ups: a link inside a hidden folder (`ui/.git/x`) is still a `[path]` rule in `Mod::read`;
  `O_NOFOLLOW` guards only the last path part; `folder.rs` opens with plain `File::open` after
  `symlink_metadata` (a narrow swap window; packed bytes stay safe).
- [x] **S16 J1 with Codex.** Delivered 2026-11-11 (main `34c6cc4`). Sol runs: none. Codex made the
  mod alone (379 s, 31 commands, 6 failed, about 9 source reads); MCP calls needed one approval line.
  Log: `factory/harness-codex.md`; raw runs in `factory/runs/s16-bob/` (untracked).
- [~] **S17 The simplest tool to start with.** (branch `slice/s17`; foreman: see `Foremen`) The human's call: one slice only for the maker's
  experience, for every harness. Input: S16's friction log and the user-sim notes in `Friction`
  (`next` null after `serve` and on env gaps, doctor's text mode prints raw JSON, `--help` shows no
  options, contract and WIT errors with `line: null`, `preview.previous` names the API URL, a
  missing vendor folder reads as a version mismatch, `serve --stop` drops data silently).
  - Outcomes: the top items fixed, each with a cheap test; the same story under Claude and Codex
    takes fewer commands and fewer errors than in S16, measured the same way.
  - From S16 (`factory/harness-codex.md`): the vendor fix must work from the agent's own folder
    (a sandboxed agent cannot write into the watane checkout); `--help` gives each command and
    option a line; the template `AGENTS.md` says how to add the preview's MCP endpoint and token
    to an agent's config (the token changes with each preview), that some harnesses need tool
    approval for it (Codex: `default_tools_approval_mode = "approve"`), and how to try the page
    without a browser; the missing vendor folder says "missing" (red test
    `regress_s16_missing_vendor_says_missing`, ignored until S17).
  - Measure both harnesses the same way: commands, failed commands, source reads, guesses.
  - Touches: dev (`cli.rs`, `outcome.rs`, `preview.rs`, the template `AGENTS.md`). After S11 and
    S13 (shared dev files).
- [x] **S12a J2: Alice runs her mods together in the app.** Delivered 2026-11-11 (main `37b2a9e`). Sol runs: S12a-1 1, S12a-2 1, S12a-fix1 2.
  Left for later (user-sim, validator): the app's toolset view names a tool
  `reading-list:books.list`, but MCP names it `reading-list_books_list` (APP-F-25; cheap app fix);
  with a provider disabled, the recommendations fixture page shows "not granted" and hides the
  host's message; stage C must put the lock on `Parties` (now a BTreeMap alias); pages get no
  grant-change message, so the app reloads the open window; a `/call` whose client disconnects
  keeps an `Arc<Host>` past shutdown; host `ac4_program_defaults_to_environment_home_and_port`
  probes port 47341 and then binds it, a race with a running watane.
- [~] **S18 Alice runs the app on Linux.** (branch `slice/s18`; foreman: see `Foremen`) App stage F:
  every app test passes on Linux under `xvfb-run`, in a Docker container on this Mac (arm64; the
  deputy decision says a Linux VM is enough; x86_64 under emulation only if arm64 hides a gap).
  - Outcomes: one script (`app/scripts/linux-test.sh`) builds the image and runs the app's
    default and driver tests in it, all green; a `*.localhost` gap in WebKitGTK, if any, is a
    finding in `app/NOTES.md`; the toolset view shows the host's MCP tool name (APP-F-25, from
    S12a's user-sim).
  - Touches: app only (Linux `cfg` code, the driver's Linux side, the script, a Dockerfile under
    `app/`). Not dev (S17), not host unless a test proves a host bug on Linux (then ask).
  - Leaves out: a Linux bundle, CI.
- [ ] **S12b The agent installs a mod into Alice's home.** `watane-dev install` (DEV-S-27 to 29)
  and `fake-watane`, against the home host S12a built. After S17 (shared dev files).
  Also the S11 user-sim notes: `fix` for `mod.dependency_cycle`/`self_dependency`,
  `check.providers[].path` for a local provider, `add --dir` path resolution, a subscriber's
  `/events` getting the provider's raw `event` messages.

## Stage coverage

One row per stage of each subsystem's build plan (its TECHNICAL "Build plan" table), with the
step that needs it. Status: `open`, `part` (what is missing), `done` (slice). A step is done only
when its journey passes and every stage of that step is `done`, or a deputy decision moves the
rest to a later step. Leftover `ponytail:` comments count as missing parts (`git grep ponytail:`).

| Subsystem | Stage | Step | Status |
|---|---|---|---|
| pkg | 1 names, 2 meta/deps, 3 world/folder/grant | 1 | done (S1) |
| pkg | 4 component | 1 | done (S2) |
| pkg | 5 package (pack, unpack) | 1 | done (S7, S8: the host installs through it) |
| pkg | 6 add_provider | 2 | done (S11) |
| pkg | 7 release | 3 | open |
| host | 1 runtime core | 1 | done (S2) |
| host | 2 links and events | 1, 2 | done (S3, S11: links, subscribers, hops on the preview `install_folders` path) |
| host | 3 server and pages | 1 | done (S9); the subscription and revoke hostile rows in step 2 |
| host | 4 tools (toolsets, tokens, MCP) | 1 | part (S5): `remove_toolset`, `toolsets`, `tokens` with the app (step 2) |
| host | 5 catalog and home | 1, 2 | done (S8, S11, S12a: plan, `Source::Folder`, grants, revoke, update, remove, home mode) |
| host | 6 program | 1 | done (S9: `--home` runs a participant home with no app) |
| dev | 0 cli, outcome | 1 | done (S1) |
| dev | 1 template, new, check | 1 | done (S4) |
| dev | 2 allowlist, toolchain, builder | 1 | done (S8) |
| dev | 3 unit | 1 | done (S4) |
| dev | 4 contract | 1 | done (S4, S11: fakes and `deliver`) |
| dev | 5 preview, api, doctor, fake-watane | 1 | done (S6); `fake-watane` moved to step 2 (deputy) |
| dev | 6 deps, add, install | 2 | part (S11: deps, add): `install` (DEV-S-27 to 29) |
| dev | 7 publish | 3 | open |
| app | A shell | 2 | done (S10) |
| app | B host | 2 | done (S12a) |
| app | F Linux | 2 | open: a Linux VM is enough (Docker on this Mac runs arm64 Linux; x86_64 under emulation) |
| app | C dist | 3 | open |
| app | D find | 4 | open |
| app | E build on | 5 | open |
| log | L1 to L5 | 3 | open |
| dist | S1 to S5 | 3 | open |
| dist | S6 find | 4 | open |

## Foremen

Background CLI sessions; transcripts in `~/.claude/projects/<worktree folder>/<sessionId>.jsonl`.

- S4: `91605db7` (sessionId `91605db7-cf74-46c5-b1a9-dfff584ce505`), worktree `.claude/worktrees/s4`; died 2026-11-10 23:32 mid S4-2 validation.
  Resumed 2026-11-11 as `2066b4f1` (sessionId `2066b4f1-2ca3-4a1f-9fa3-29c671cea48f`); died after
  the user-sim's ACCEPTED, in its final test run; the product owner ran the tests and merged (stopped, removed).
- S5: `91dc7a35` (sessionId `91dc7a35-91d7-4d4d-a6a1-4f843f3fd54a`), worktree `.claude/worktrees/s5` (stopped, removed).
- S8: `44bfe2ab` (sessionId `44bfe2ab-9c0d-472b-9ebe-4495e4af90d8`), worktree `.claude/worktrees/s8` (stopped, removed).
- S6: `4a8d4ea4` (sessionId `4a8d4ea4-67ac-411a-bf4d-9ce82018000d`), worktree `.claude/worktrees/s6` (stopped, removed).
- S9: `e793310c` (sessionId `e793310c-21e9-4c6d-bddd-6e1180c23fe4`), worktree `.claude/worktrees/s9` (stopped, removed).
- S10: `a41a03d5` (sessionId `a41a03d5-4976-4920-bd4a-029b3628011c`), worktree `.claude/worktrees/s10` (stopped, removed).
- S11: `23325b72` (sessionId `23325b72-bf6f-4a9b-a8aa-846a03ff376c`), worktree `.claude/worktrees/s11` (stopped, removed).
- S14: `bffc598d` (sessionId `bffc598d-0fb1-4534-afc1-55371b4892e1`), worktree `.claude/worktrees/s14` (stopped, removed).
- S15: `1fd281db` (sessionId `1fd281db-b26f-4f32-903f-2a9a91d248f9`), worktree `.claude/worktrees/s15` (stopped, removed).
- S16: `c69c6b56` (sessionId `c69c6b56-1fd2-47b8-baf1-9cf4e88e95b5`), worktree `.claude/worktrees/s16` (stopped, removed).
- S12a: `48eb6176` (sessionId `48eb6176-882b-4e90-9154-7f8df7a7077e`), worktree `.claude/worktrees/s12a` (stopped, removed).
- S13: `93875efd` (sessionId `93875efd-ef7d-4863-a9eb-7cad24dda18f`), worktree `.claude/worktrees/s13` (stopped, removed).
- S17: `2894d2f6` (sessionId `2894d2f6-5753-4f2c-8189-388e2022683d`), worktree `.claude/worktrees/s17`.
- S18: `2a36dab3` (sessionId `2a36dab3-8ee4-4149-a9b7-8446fa03e19c`), worktree `.claude/worktrees/s18`.
- S7: `871c5605` (sessionId `871c5605-a537-44d1-bd37-c2dc3068d11c`), worktree `.claude/worktrees/s7` (stopped, removed).

## Waiting on the human

- Process change (friction 3x: S7, S14, S12a): foremen try `git push`. Proposal: one line in
  `.claude/skills/foreman/SKILL.md` §5: "Never push; the product owner merges locally." Cost to
  undo: none. Also: delete the pushed `slice/s12a` on origin (merged into local main)?

- S16/S17: should the host mark some MCP tools read-only (`readOnlyHint`), so harnesses like Codex
  call them without an approval setting? Options: (a) no hint; the template `AGENTS.md` documents
  the approval line per harness; (b) a mod declares read-only functions (a new `mod.toml` or WIT
  convention) and the host passes the hint on. Recommendation: (a). The hint is the mod's own
  claim, so a mod could mark a write read-only to skip approval, and (b) adds a format. Cost to
  undo: (a) nothing; (b) a format change in pkg, host and dev. S17 builds (a) meanwhile.

## Deputy decisions

The human may veto each one.

- S1: one root Cargo workspace (resolver 3, one `Cargo.lock`, shared `target/`).
- S1: the host installs a folder by copy with a temporary package digest until pkg `pack` lands
  (host §15, Q12); only throwaway preview homes store it.
- S1: host recovery restores a committed `.old` mod before deleting leftovers (Q13); the home lock
  is held before the marker is created (Q14).
- S1: `run.json` means "listening"; only the ready line means "initial mods installed".
- S1: `serve --stop` finds the preview by the mod path's hash alone, so it works on a broken or
  deleted mod folder.
- S1: `deps.json` refuses duplicate keys; a folder-level violation names file `.` (pkg §15).
- S2: `Host::open(home, Limits)` until home mode; `Caller` is only `Page`; no depth or hop limits yet.
- S2: event imports are linked as no-ops until S3; the storage grant check inside state waits for
  `revoke`.
- S2: install copies `logic.wasm` with the folder, then `check_component` and compile; mods with
  calls or subscriptions to other mods are refused until links land.
- S2: the engine turns Wasm threads off and refuses shared memory (host Q15 to Q17).
- S2: `check_component` checks the component header before decoding (pkg §15 item 16).
- S2: `serve` runs check and build only; the outside-read check and reproducible bytes wait
  (`ponytail`).
- S3: events reach only the emitter's own pages (`changed` and the events); subscribers wait for
  links. The page bus lives in `Host` (broadcast, 1024). host adds `tokio-stream` 0.1 for SSE.
- S3: the outbox holds JSON, not WIT values with hops (host Q18; revisit with subscribers).
- S3: ending streams on disable or remove, and hop and delivery limits, wait (host §15 S3 cut).
- S3: pages reload only when the package id changes.
- S7: `pack` checks paths (links, special files, names) before `Mod::read`, so a link gives
  `PackageError::Symlink` and special files are never opened (pkg §6.4, §15.17).
- S4: `serve` runs the whole `test` pipeline first; failing tests start no preview and leave a
  running one alone.
- S4: unit tests keep typed WASI instances linked (wasmtime 48's `define_unknown_imports_as_traps`
  breaks resource identities); non-WASI imports trap; TCP, UDP and name lookup are off.
- S4: `Harness` has only `call`; `deliver` joins with links (step 2), `functions()` with S5.
- S4: the outside-read check on unit-test builds stays deferred with DEV-S-10 (S8).
- S5: `Host` gains only `functions`, `set_toolset`, `issue_token`, `end_token`, `Served::mcp_url`;
  `remove_toolset`, `toolsets`, `tokens` wait for the app. `set_toolset("preview")` is `bad_name`
  in every home. `/mcp/<toolset>` is on the API host names with a 1 MiB body limit.
- S5: `Host::functions` is the listing; `Harness::functions` waits for `test.uncovered`.
  wit-parser is pinned `=0.254.2` (the version in `Cargo.lock`).
- S5: `serve`'s `next` is `watane-dev test --json` only when the test pipeline fails.
- S6: `fake-watane` moves to step 2; only the home-host scenarios (S-25 to 31) need it.
- S8: dev scans and clears both `<crate>.d` and `<crate>-*.d`, and touches the copied crate root
  so cargo writes fresh dep-info; only the mod crate rebuilds.
- S8: the host unpacks into a new `staging/<hex>/mod/`; the link refusal still runs before pack.
- S9: the program runs participant homes (`--home`, `Mode { Home, Preview }`); until the app's
  Confirm lands, a home acts as "no app": installs that need confirmation are 403 `refused`.
  No-arg `watane` opens the default home on port 47341 (host 15 "S9 cut", Q22).
- S9: repeated `Host` headers are 421; a missing page file is 404 on any method; the connection
  closes after a body-size refusal (host 8.1 to 8.3, Q21).
- S6: doctor's `next` is `watane-dev new <slug> --json` with no gaps, else `watane-dev doctor --json`;
  a missing vendor folder's fix is `<abs dev>/scripts/vendor.sh` in doctor, `test` and `serve`.
- S6: a refused `serve` warns `preview.previous` when a preview of that folder still answers.
- S10: `mod_disable` moved into stage A (fake Core) so app S8 is testable now.
- S10: `Info.plist` has an ATS exception for `localhost` and its subdomains (insecure HTTP only, no
  arbitrary loads); without it macOS loads no mod page. host Q10 verified for WKWebView (`::1`
  works); WebKitGTK still open (stage F).
- S10: the consent window's `Destroyed` handler denies and reopens off the main thread.
- S14: the foreman edited `factory/README.md` ("Front" row in the test pyramid), `validator.md`
  (never runs front tests) and `user-sim.md` (starts the app hidden) itself.
- S14: a driver op `screen` reports the mode and the real AppKit activation policy (app 10).
  Hidden WKWebView loads pages and runs timers on time, so only a self-test uses `background`.
  The front tests' idle wait panics after 10 minutes; it never skips.
- S15: `PackageError::TooManyFiles` (count 0 stays `Format`); `Path`/`Symlink` print the path
  with `escape_debug`; pkg adds `libc` 0.2 (unix) for `O_NOFOLLOW|O_NONBLOCK`, no `unsafe`.
- S16: Codex ran in its `workspace-write` sandbox with network (as Sol), with its own
  `CODEX_HOME`, so the human's `~/.codex` was untouched.
- S11: host fixture `list` renamed `shelf` (`list` is a WIT keyword, refused as a slug).
- S11: host links, missing providers, cycles and order are built on the preview `install_folders`
  path only; `Host::plan`, `Source` and confirmation stay with app stage B (S12).
- S11: no global call lock: per-mod call locks plus per-emitter ordered delivery.
- S11: `Harness::deliver(function, args)` takes `<short iface>.<fn>` of a subscription export,
  caller `mod:<provider slug>`. `changed` goes only to mods whose state bytes changed.
- S11: serve `next`: success gives the served mod's own page (the last entry, providers come
  first), null when it has no page; an `env.*` refusal gives `watane-dev doctor --json`; a
  provider's failure gives `watane-dev test --json --dir <provider>`.
- S11: pkg `add_provider` refuses a symlinked destination; an I/O failure may leave a partial copy.
- S13: dev tests share one warm cache folder per test binary (a copy at a new path builds cold:
  the path is in the rustflags). A command test may be removed once a unit test proves its rule;
  5 were, each named in TECHNICAL 15.2's replacement table. DEV-S-27 to 31 are marked planned.

## Friction

- S12a: giving back a withdrawn grant asks no consent (app TECHNICAL 7); app FUNCTIONAL S9 changed to match.
- S12a: the app fixtures ship a committed `logic.wasm` (`tests/fixtures/build.sh`); no app test builds a mod.
- S12a: the app CLI in stage B: `--home` defaults to `default_home()`, `--port` to `WATANE_PORT`, then `DEFAULT_PORT`.
- S12a: `Source` has only `Folder`; `Release` comes with stage C. `install_folders` stays for previews and dev (host Q24).
- S12a: host shutdown waits for every server-side holder of the host, at most 5 s (host 12.2 unchanged).
- S1: each foreman turn's closing text reached the product owner as a progress update. Fixed by
  the human's rule (1eed839): foremen report only when blocked or delivered.
- S2: a brief left out the subsystem's error-code module (`dev/src/outcome.rs`); Sol stopped
  halfway. Briefs should always allow it.
- S2: Sol's sandbox refuses nested `sandbox-exec`, so DEV-S-13 runs only in the validator.
- S2, S3: host program tests with 2 s and 10 s limits fail now and then under heavy load (2x).
- S3: the brief did not say S2's `/events`-404 tests would change; Sol left them red.
- S3: nested `sandbox-exec` still runs only in the validator (2x).
- S4: the foreman's process died mid-validation (the battery ran out); `watch.sh` was not
  running, so nobody woke. Not process friction.
- S7: the foreman tried `git push`; the repository has no remote.
- S7: there is no command that only builds `logic.wasm`; and Homebrew's `cargo` comes before
  rustup's on this machine's PATH, without `wasm32-wasip2`.
- S4: the foreman died after acceptance, in its last test run (a forced OS update). Not process
  friction.
- S4: contract `test.contract` errors carry `line: null`; `emits` mismatches show the full event
  name in `got` and the short one in `expected`; one template bug fails a unit and a contract test.
- S5: Sol's sandbox cannot run dev's ignored tests; a task that needs another's host ends PARTIAL
  and the validator runs the real tests.
- S5: a fresh worktree has no `dev/allowlist/vendor`; 4 builder tests fail until
  `scripts/vendor.sh` runs (into S6 `doctor`).
- S5: dev's `ac10` unit-limit test (7 s) fails under parallel load; it gets the timing ignore (S8).
- S5: a refused `serve` leaves the old preview up without saying so (into S6); a trap's text is a
  long wasm backtrace with the panic message at the end; dev keeps the preview token in
  `<cache>/previews/*/ready.json` (0600, dev Q6), which reads awkwardly beside host 10.2.
- S2, S3, S8: nested `sandbox-exec` (DEV-S-13) runs only in the validator (3x). The human
  accepted it (factory/README.md); no longer friction.
- S8: S8-1's Sol run took 77 min, much of it full-suite reruns under S5's load.
- S8: dev `ac10` (timing-ignored now) failed twice on `main` under S6's load: 8.4 s against 7 s,
  the time includes rebuilding the changed source. Rerun when quiet; if it fails then, the timer
  should start after the build.
- S8 user-sim: when only the unit-test build reads outside, `result.build` still names the removed
  `logic.wasm`; `build.reads_outside` has `line: null`; `serve --help`/`new --help` show no options.
- S9: two parallel tasks both took spec gap id Q21; the foreman should hand out Q ids.
- S9: Sol stopped PARTIAL on an old S1-cut test; a brief that changes a cut should name the tests
  it supersedes.
- S9 user-sim: a huge declared `Content-Length` with no body waits instead of 413 at once;
  duplicate keys in a `/call` body are accepted (last wins); `mode_mismatch` does not name the
  asked mode; `GET /api/mods` is 404 (S1 cut); HTTP-layer 400s lack the 8.3 headers.
- S6: the full dev run with `--include-ignored` takes about 12 min, over one 10-min shell limit.
- S6: parallel tasks all append to dev/AGENTS.md "Spec gaps", so each rebase conflicts there
  (trivially).
- S6 user-sim: `preview.previous` names the API URL, not the page; a missing vendor folder reads
  as "differs from this watane-dev"; `mod.invalid` WIT errors have `line: null`; doctor's text mode
  prints raw JSON; a raw `/call` without `args` is `bad_request`; headless Chrome hung on the
  page's open `/events`, so the form was not driven.
- S10: Sol's sandbox has no GUI session; app window tests run only in the validator, which found
  the real bugs (ATS, consent reopen). App briefs must say so.
- S10: window tests get focus only while the terminal is in front (macOS 27); unattended runs can
  time out on app S3 to S5.
- S10: Tauri embeds `Info.plist` without a rebuild trigger; a `cargo clean -p watane-app` dropped
  2.5 GiB of the shared `target/`.
- S7, S14: the foreman tried `git push`; there is no remote (2x).
- S14: app `support_selftest` "port refused after drop" checks fail now and then under parallel
  load (pre-existing; a fix task for a later app slice).
- S14: "user-sim runs no suite" did not fit a slice whose story is the suite; the foreman made a
  one-off exception. Minor: `App::start`/`background`/`front` lack doc comments;
  `ac3_consent_takes_focus` passes a misleading `("WATANE_DRIVER_SCREEN","hidden")` to `App::front`.
- S9, S15: a brief kept "existing tests stay green" while it changed what an old test asserts;
  Sol stopped or needed a resume (2x).
- S15: watane-dev has no `pack` command, so pack is reached only through the host or pkg tests.
- S16: codex-cli 0.163 has no `--full-auto` (use `-s workspace-write`); `codex exec` needs
  `--skip-git-repo-check` outside a git repo; S6 kept no Claude command log to compare with.
- S11: a spec fixture name (`list`) was never checked against the slug grammar; S11-2's first run
  stopped BLOCKED on it.
- S11: Sol twice added an unlisted product change to make its tests easy (a host global call
  lock; a dev "built-only" `test` that would let `serve` install an unbuilt `logic.wasm`).
  Validators caught both.
- S11: `--include-ignored` now also runs main's intentionally red S16 test, so a foreman's final
  run needs a `--skip`.
- S11: dev's full slow run took about 30 min (serve.rs 11, unit_tests.rs 10): S13's input.
- S13: a fresh worktree has no `dev/allowlist/vendor`; tests fail with `env.allowlist` until
  `scripts/vendor.sh` runs. Foremen should run it first.
- S13: the 2-min default budget rests on two binaries (contract, unit_tests) that each pay one
  cold build of about 35 s; little headroom as tests are added.
- S13: some `slow:` reasons say why a test is slow but not what it needs.
- S12a: Sol cannot run window tests, so app tasks end PARTIAL and the validator finds the GUI
  failures (S10, S12a: 2x).
- S12a: the foreman pushed `slice/s12a` to origin (once the remote existed); now told: slice
  branches stay local. Earlier push attempts: S7, S14 (3x in all; process change to propose).
- S12a: a test flaked 1 in 16 runs after the rebase: a server task kept `Arc<Host>` after
  shutdown. Fixed with a bounded wait (S12a-fix1).

## Digest

- S1: a coding agent runs `watane-dev new hello`, `check` and `serve --json`, opens
  `http://hello.localhost:<port>/` and sees the page; an edit shows after `serve` again;
  `serve --stop` ends the preview.
- S2: the template is a counter; Alice clicks Add on the page and the count goes up, stored by the
  host; a runaway export is stopped and the mod keeps answering.
- S3: the agent edits logic and page while Alice has it open; `serve` reloads her pages with the
  new revision and the same count; every open page updates by itself.
- S7: a mod folder packs into one canonical package with a stable id and unpacks to the same
  folder; hostile packages are refused.
- S4: the agent runs `watane-dev test`, gets unit and contract failures with file, step, expected
  and got, fixes the code and passes; unit tests run sandboxed with a time limit; `serve` refuses
  a mod whose tests fail.
- S5: the agent points an MCP client at `serve --json`'s `mcp` URL with its token, lists and
  calls the mod's functions, and Alice's open page updates by itself.
- S8: a mod folder builds to the same `logic.wasm` from any path and cache, and the preview host
  installs it under the same id `pack` gives; a mod that reads a file outside its folder is refused.
- S9: every page, `/call`, `/events`, `/api` and program scenario of step 1 has a test on the
  real `watane`; hostile pages are refused; `watane --home D` runs a participant home.
- S6: Alice's agent runs `doctor`, builds her reading list with `new`, `test`, a contract fix and
  `serve`; her book survives a page change; the agent reads the list over MCP (J1 passes).
- S10: Alice starts `watane-app --home <dir>` and sees an empty launcher with her two mark words;
  mod windows are sealed (109 hostile attacks, 0 allowed); every request goes through the consent view.
- S14: `cargo test --features driver` in `app/` shows no window, no Dock icon and takes no focus;
  the 4 front tests run with `--include-ignored` once the human is idle 30 s.
- S15: a bad file in a mod gives an error that names the real cause (too many files, a link,
  escaped control characters, the unpack error before the cleanup error); reads never follow a
  swapped-in link.
- S16: a Codex agent makes and previews a mod with no human help; `factory/harness-codex.md`
  lists what made it guess or read source.
- S11: in `recommendations` the agent adds `reading-list`, writes a call and a subscription, tests
  them with a fake and a `deliver` step, and `serve` installs the provider first; a book added in
  one page reaches the other through the call and the event.
- S13: a watane maker runs `cargo test` in dev and gets an answer in about 1.5 min, not 9.
- S12a: Alice installs both mods in the app after consent, uses them together, restarts, withdraws
  and gives back a grant, disables or removes a mod, and gives an agent an MCP token (J2 passes).
