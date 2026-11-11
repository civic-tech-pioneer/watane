# Plan

The product owner's state; only the product owner edits it, on `main`. A new session continues
from here. Newest digest line last.

## Slices

Status: `[ ]` sketched, `[~]` in flight (with its branch), `[x]` delivered.

Step 1 (Make, J1). A first cut; the product owner refines each sketch before it hands it out.

- [x] **S1 Hello page.** Delivered 2026-11-10 (main `bacecdc`). Sol runs: pkg 1, host 1, dev 2.
- [x] **S2 The page calls its logic.** Delivered 2026-11-10 (main `7eb55ad`). Sol runs: pkg 1, host 2, dev 2.
- [x] **S3 An edit keeps the data.** Delivered 2026-11-10 (main `c883171`). Sol runs: host 1, dev 1.
- [~] **S4 The agent fixes a failing test.** (branch `slice/s4`; foreman: see `Foremen` below)
  The agent breaks the counter's `add`, runs `watane-dev test`, reads the result, fixes the code,
  and the tests pass.
  - Outcomes (user-sim, on the real programs):
    1. On a new mod, `test --json` exits 0; unit `passed` equals the `#[test]` functions in
       `logic/src/lib.rs`, contract `passed` equals the steps in `tests/*.json` (DEV-S-4).
    2. With `add` changed to add 2, `test --json` exits 1 with `test.contract`: file, step, call
       `api.add`, expected and got; `next` is the command to run again. The fix makes it pass.
    3. A unit test `fails_on_purpose` gives `test.unit` naming it; an endless unit test gives
       `test.unit_limit` within the limit plus 5 s; a unit test cannot read `/` or the
       environment, and writes no file (DEV-S-14 to 16).
    4. Without `tests/*.json`, `test` exits 1 with `mod.no_contract_tests`; `serve` runs `test`
       first and starts no preview for a mod whose tests fail.
    5. `mod.dependency` and `mod.cargo_key` give their line in `logic/Cargo.toml`; a `/call`
       without `Origin` gets a 403 whose message names the missing header (S2 user-sim).
  - Touches: host (stage 5 part: `Harness`, `Fakes`, `Outcome`, `EventRecord`; the 403 message),
    dev (stage 3 unit, stage 4 contract and `json_eq`, template `tests/counter.json`, check
    stage 6, line numbers for `logic/Cargo.toml`).
  - Interfaces it creates: `Harness` and its types (host → dev); the contract test file (dev 8.1).
  - Leaves out: fakes for other packages and `deliver` steps (DEV-S-18, the `deliver` part of
    DEV-S-19): they need links (step 2). `emits` steps are in.
- [ ] **S5 The agent reads through a toolset.** The preview prints a toolset token; an MCP client lists
  the mod's exports and calls one. Parts: host (toolsets, MCP).
- [ ] **S6 J1.** The reading-list story of J1 as a journey test in `dev/tests/`, including a change
  that shows in the running preview.
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
| host | 3 server and pages | 1 | part (S2 `/call`, S3 `/events`): `tests/hostile.rs` page rows |
| host | 4 tools (toolsets, tokens, MCP) | 1 | open (S5) |
| host | 5 catalog and home | 1, 2 | part: preview folder install only; `Harness` (S4); plan, grants, update, remove, home mode in step 2 |
| host | 6 program | 1 | part (S1): S1 to S3 tests not all checked |
| dev | 0 cli, outcome | 1 | done (S1) |
| dev | 1 template, new, check | 1 | part (S1, S2): template `tests/` and check stage 6 in S4 |
| dev | 2 allowlist, toolchain, builder | 1 | part (S2): DEV-S-10 outside reads, DEV-S-12 reproducible bytes |
| dev | 3 unit | 1 | open (S4) |
| dev | 4 contract | 1 | open (S4) |
| dev | 5 preview, api, doctor, fake-watane | 1 | part: `serve` only (S1, S2) |
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
  Resumed 2026-11-11 as `2066b4f1` (sessionId `2066b4f1-2ca3-4a1f-9fa3-29c671cea48f`).
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

## Friction

- S1: each foreman turn's closing text reached the product owner as a progress update. Fixed by
  the human's rule (1eed839): foremen report only when blocked or delivered.
- S2: a brief left out the subsystem's error-code module (`dev/src/outcome.rs`); Sol stopped
  halfway. Briefs should always allow it.
- S2: Sol's sandbox refuses nested `sandbox-exec`, so DEV-S-13 runs only in the validator.
- S2, S3: host program tests with 2 s and 10 s limits fail now and then under heavy load (2x).
- S3: the brief did not say S2's `/events`-404 tests would change; Sol left them red.
- S3: nested `sandbox-exec` still runs only in the validator (2x).
- S4: the foreman's process died silently mid-validation; `watch.sh` was not running, so nobody woke.
- S7: the foreman tried `git push`; the repository has no remote.
- S7: there is no command that only builds `logic.wasm`; and Homebrew's `cargo` comes before
  rustup's on this machine's PATH, without `wasm32-wasip2`.

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
