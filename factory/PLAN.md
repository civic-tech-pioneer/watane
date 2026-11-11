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
- [~] **S8 Same mod, same id.** (branch `slice/s8`; foreman: see `Foremen`) Bob copies Alice's mod folder to another path and runs
  `watane-dev test` with his own cache; both builds give the same `logic.wasm` bytes, and the
  preview host installs the mod under the same canonical package id that `pack` gives. A mod
  that sneaks in a file from outside its folder is refused.
  - Outcomes (user-sim, on the real programs):
    1. One mod folder at two paths with two caches: `test` in each gives equal `logic.wasm`
       bytes, holding neither the mod path, the cache path nor the home path (DEV-S-12).
    2. `include_bytes!` of a file outside the mod: `test --json` exits 1 with
       `build.reads_outside` naming the file, no `logic.wasm`, and the file's bytes in no file
       of the mod folder; same for a unit-test build (DEV-S-10).
    3. `env!("SECRET")` with `SECRET` set in the shell gives `build.compile` (DEV-S-11).
    4. The preview host's installed package id for a mod equals the id `pack` gives for the
       same folder; the host's temporary digest is gone (both `ponytail`s in `catalog.rs`).
  - Touches: dev stage 2 (`builder.rs`, `unit.rs` build step, `dev/tests/builder.rs`),
    host stage 5 (`catalog.rs` install via `pkg::pack`/`unpack`).
  - Interfaces it changes: host install now depends on pkg `pack`/`unpack` (pkg -> host).
  - Leaves out: `O_NOFOLLOW` and the other S7 pkg follow-ups.
- [~] **S6 J1: Alice gets her reading list.** (branch `slice/s6`; foreman: see `Foremen`)
  Alice asks her agent for a reading list. The agent runs `watane-dev doctor`, then `new`, writes
  logic, page and contract tests, gets a failing contract test, fixes it, runs `serve`; Alice adds
  a book on the page, asks for one change, sees it in the preview with her book still there; the
  agent reads the list through the preview's MCP toolset.
  - Outcomes (user-sim, on the real programs):
    1. `doctor --json` reports the toolchain, the `wasm32-wasip2` target, the allowlist, the
       program `watane` and the home host, each with a command for each gap (DEV-F-30); with
       `WATANE_BIN` missing it names the gap. A missing allowlist vendor folder is a gap whose
       command is `scripts/vendor.sh` (S5 friction).
    2. J1 as a journey test in `dev/tests/journey.rs` on the real `watane`, the reading-list
       mod written into a temp folder by the test: failing contract, fix, serve, a page change
       kept with data (DEV-S-22), an MCP `tools/call` that reads the list.
    3. DEV-S-23 (`serve --stop`) and DEV-S-24 (`env.watane_missing`) hold on the real program.
    4. A `serve` refused for failing tests says that a running preview still serves the
       previous build (S5 user-sim).
  - Touches: dev stage 5 (`cli.rs`, new `doctor.rs`, `preview.rs`, `dev/tests/journey.rs`,
    `dev/tests/serve.rs`), dev template `AGENTS.md` (doctor first). Not `builder.rs`, `unit.rs`,
    `dev/tests/unit_tests.rs`, `dev/tests/builder.rs` or anything in host (S8).
  - Leaves out: `fake-watane` (needed only for the home-host scenarios S-25 to 31, step 2).
- [ ] **S9 Hostile pages and the program, checked.** Host stages 3 and 6: every scenario of host
  FUNCTIONAL for pages, `/call`, `/events` and the program has a passing test in
  `host/tests/hostile.rs` or `program.rs`; what fails gets fixed.
  - Outcomes: the foreman lists each such scenario with its test; the user-sim plays a hostile
    page (wrong `Origin`, another mod's host name, oversize body, path tricks) on the real
    `watane` and every one is refused as the spec says.
  - Touches: host `tests/hostile.rs`, `tests/program.rs`, and `server.rs`/`pages.rs`/`bin` only
    for fixes. Waits for S8 (it touches `tests/program.rs`, `tests/pages.rs`).
- [x] **S7 Pack a mod.** Delivered 2026-11-10 (main `a740d78`). Sol runs: pkg 1.
  Follow-ups for a later pkg slice: too many files reads as "count must be 1..=4096"; a hidden
  link `ui/.x` is refused as a mod rule, not `Symlink`; a NUL in a path shows raw in the message;
  a failed unpack cleanup hides the first error; no `O_NOFOLLOW` against a walk-then-read race.

## Stage coverage

One row per stage of each subsystem's build plan (its TECHNICAL "Build plan" table), with the
step that needs it. Status: `open`, `part` (what is missing), `done` (slice). A step is done only
when its journey passes and every stage of that step is `done`, or a deputy decision moves the
rest to a later step. Leftover `ponytail:` comments count as missing parts (`git grep ponytail:`).

| Subsystem | Stage | Step | Status |
|---|---|---|---|
| pkg | 1 names, 2 meta/deps, 3 world/folder/grant | 1 | done (S1) |
| pkg | 4 component | 1 | done (S2) |
| pkg | 5 package (pack, unpack) | 1 | done (S7); host still uses its temporary digest (2 `ponytail` in `catalog.rs`) |
| pkg | 6 add_provider | 2 | open |
| pkg | 7 release | 3 | open |
| host | 1 runtime core | 1 | done (S2) |
| host | 2 links and events | 1, 2 | part (S3: page bus, outbox): links, subscribers, hops in step 2; `ponytail` in `bus.rs` |
| host | 3 server and pages | 1 | part (S2 `/call`, S3 `/events`, S5 `/mcp`): `tests/hostile.rs` page rows (S9) |
| host | 4 tools (toolsets, tokens, MCP) | 1 | part (S5): `remove_toolset`, `toolsets`, `tokens` with the app (step 2) |
| host | 5 catalog and home | 1, 2 | part: preview folder install; `Harness::call` (S4); temp digest (S8); `deliver` and plan, grants, update, remove, home mode in step 2 |
| host | 6 program | 1 | part (S1, S5 ready `mcp`/`token`): S1 to S3 tests not all checked (S9) |
| dev | 0 cli, outcome | 1 | done (S1) |
| dev | 1 template, new, check | 1 | done (S4) |
| dev | 2 allowlist, toolchain, builder | 1 | part (S2): DEV-S-10 outside reads (`ponytail` in `builder.rs`), DEV-S-12 reproducible bytes (S8) |
| dev | 3 unit | 1 | done (S4) |
| dev | 4 contract | 1 | part (S4): fakes and `deliver` steps (DEV-S-18, 19) in step 2 |
| dev | 5 preview, api, doctor, fake-watane | 1 | part: `serve` with MCP (S1, S2, S5); doctor and J1 (S6); `fake-watane` moved to step 2 (deputy) |
| dev | 6 deps, add, install | 2 | open |
| dev | 7 publish | 3 | open |
| app | A shell | 2 | open |
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
- S8: `44bfe2ab` (sessionId `44bfe2ab-9c0d-472b-9ebe-4495e4af90d8`), worktree `.claude/worktrees/s8`.
- S6: `4a8d4ea4` (sessionId `4a8d4ea4-67ac-411a-bf4d-9ce82018000d`), worktree `.claude/worktrees/s6`.
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
