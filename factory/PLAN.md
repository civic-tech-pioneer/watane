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
  Follow-ups for a later pkg slice: too many files reads as "count must be 1..=4096"; a hidden
  link `ui/.x` is refused as a mod rule, not `Symlink`; a NUL in a path shows raw in the message;
  a failed unpack cleanup hides the first error; no `O_NOFOLLOW` against a walk-then-read race.

Step 2 (Run together, J2).

- [x] **S10 Alice opens the app.** Delivered 2026-11-11 (main `da54bb4`). Sol runs: S10-1 1, S10-2 1, S10-3 1, S10-4 1.
- [~] **S11 The recommendations mod calls the reading list.** (branch `slice/s11`; foreman: see `Foremen`) In a preview: the agent runs
  `watane-dev add ../reading-list` in `recommendations`, writes a call and a subscription, tests
  with a fake and a `deliver` step, and `serve`s both; adding a book in one page shows in the
  other.
  - Outcomes: DEV-S-18, 19 (fakes, `deliver`), DEV-S-25, 26 (`add`, cycle); a `serve` of
    `recommendations` installs `reading-list` first; a call and an event cross the two mods on
    the real `watane`.
  - Touches: pkg stage 6 (`add_provider`), host stage 2 (links, subscribers, hops: `bus.rs`
    ponytail; `harness.rs` fakes ponytail), dev stage 6 (`deps`, `add`; `cli.rs`), dev contract
    fakes and `deliver`. Not `app/` (S10).
  - Also (S6 user-sim): after a successful `serve`, `next` says to give `pages[0].url` to the person;
    a `serve` refused for an env gap has `next` `watane-dev doctor --json`; `serve --stop` says
    that the preview's data is gone.
  - Leaves out: `install` into a home host (needs the app, stage B), grants UI.
- [ ] **S13 Cheap tests.** The test pyramid (`factory/README.md`) on the tests that exist. A
  maker of watane runs `cargo test` in dev and gets an answer in under 2 minutes, not 12.
  - Outcomes: dev's default `cargo test` under 2 min on a quiet machine (now: a cold mod build of
    about 30 s in nearly every integration test); the template is built once per test binary and
    each test copies that warm cache; rules now proven by a command test (check rules, error
    texts, `json_eq`, classifications) get unit tests, and the command test stays only where it
    proves wiring; cold builds, the preview life cycle and J1 are `slow` with a reason; host's
    default run is checked against the same budget. No outcome loses its proof: each DEV-S and
    HOST scenario keeps a test, listed by the foreman.
  - Touches: `dev/tests/*`, `dev/src/*` test modules only, host tests only if over budget.
    Waits for S11 (it adds dev tests).
- [~] **S14 Quiet GUI tests.** (branch `slice/s14`; foreman: see `Foremen`) Requested by the
  human: the app's driver tests open real windows that take focus and show a Dock icon, which
  interrupts the human's own work while the factory loops. A maker runs
  `cargo test --features driver` in `app/` and sees nothing happen on screen.
  - Design (agreed with the human): the driver reads `WATANE_DRIVER_SCREEN` = `hidden` |
    `background` | `front` once at startup. One helper in `views.rs` wraps the three
    `WebviewWindowBuilder`s (main, mod, consent): hidden -> `.visible(false)`, background ->
    `.focused(false)`, front -> unchanged. One helper replaces the direct `set_focus` /
    `request_user_attention` calls and does nothing unless front. Every mode except front sets
    `ActivationPolicy::Accessory` in setup. Without feature `driver` the helpers do nothing.
    Tests declare their mode, with no ifs in test bodies: `App::start` (hidden, the default),
    `App::background`, `App::front` in `tests/support/driver.rs`. Tests that need real focus or
    stacking (consent takes focus, the 800 ms approve delay, consent stays above; at first look
    `gui_isolation` ac1 and `gui_shell` ac4) use `App::front` and carry
    `#[ignore = "front: takes the screen"]`. `App::front` waits until the human has been idle
    about 30 s (`ioreg` HIDIdleTime), printing "waiting for you to step away", and fails loudly
    after a time limit; it never skips, since these are security checks.
  - Check first: whether hidden WKWebView windows still finish page loads, run driver JS and keep
    timers unthrottled; a test that breaks hidden uses `App::background`.
  - Outcomes: 1. a plain `cargo test --features driver` in `app/` shows no window, no Dock icon,
    never takes focus; 2. `-- --include-ignored` runs the front tests and they pass; 3. the release
    build still has no driver; 4. `factory/README.md` "Test pyramid" and the user-sim and validator
    prompts say front tests run once per slice (user-sim or foreman), not every validator round.
  - Touches: `app/src/{views,driver,lib}.rs`, `app/tests/support/driver.rs`, `app/tests/gui_*.rs`,
    `factory/README.md`, `.claude/agents/{user-sim,validator}.md`. Not dev, host, pkg (S11).
  - Rejected: moving windows to another Space (private APIs only).
- [ ] **S12 J2: Alice runs her mods together in the app.** App stage B on the real host. Alice
  starts the app, installs `reading-list` and `recommendations` from their folders, approves the
  plan, uses them together, restarts, withdraws a grant, and removes a mod.
  - Outcomes: app S2, S2b, S6 to S9 against the real host; J2 as `app/tests/j2.rs`; the host's
    plan, grants, revoke, update, remove and home mode with the app's Confirm (host stage 5);
    the step-2 hostile rows (subscription export via `/call`, storage after revoke).
  - Touches: app (`core.rs` on host, `HostConfirm`, `words.rs`, delete `fake.rs`), host stage 5
    (catalog plan, grants, remove), dev `install` (DEV-S-27 to 29, `fake-watane`). Waits for S10
    and S11; may be cut into an app part and a host part once those land.
  - Also (S10 user-sim): the main page polls on focus and every 2 s (app 7); "No mods yet" in an
    empty launcher; a disabled mod says so; opening an open mod while consent shows must not
    steal its focus; `ready` reports the real port.

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
| pkg | 6 add_provider | 2 | open |
| pkg | 7 release | 3 | open |
| host | 1 runtime core | 1 | done (S2) |
| host | 2 links and events | 1, 2 | part (S3: page bus, outbox): links, subscribers, hops in step 2; `ponytail` in `bus.rs` |
| host | 3 server and pages | 1 | done (S9); the subscription and revoke hostile rows in step 2 |
| host | 4 tools (toolsets, tokens, MCP) | 1 | part (S5): `remove_toolset`, `toolsets`, `tokens` with the app (step 2) |
| host | 5 catalog and home | 1, 2 | part: preview install via pack (S8); `Harness::call` (S4); fakes (`ponytail` in `harness.rs`), `deliver`, plan, grants, update, remove, home mode in step 2 |
| host | 6 program | 1 | done (S9: `--home` runs a participant home with no app) |
| dev | 0 cli, outcome | 1 | done (S1) |
| dev | 1 template, new, check | 1 | done (S4) |
| dev | 2 allowlist, toolchain, builder | 1 | done (S8) |
| dev | 3 unit | 1 | done (S4) |
| dev | 4 contract | 1 | part (S4): fakes and `deliver` steps (DEV-S-18, 19) in step 2 |
| dev | 5 preview, api, doctor, fake-watane | 1 | done (S6); `fake-watane` moved to step 2 (deputy) |
| dev | 6 deps, add, install | 2 | open |
| dev | 7 publish | 3 | open |
| app | A shell | 2 | done (S10) |
| app | B host | 2 | open |
| app | F Linux | 2 | open |
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
- S11: `23325b72` (sessionId `23325b72-bf6f-4a9b-a8aa-846a03ff376c`), worktree `.claude/worktrees/s11`.
- S7: `871c5605` (sessionId `871c5605-a537-44d1-bd37-c2dc3068d11c`), worktree `.claude/worktrees/s7` (stopped, removed).

## Waiting on the human

(none)

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

## Friction

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
