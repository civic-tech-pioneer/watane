# Plan

The product owner's state; only the product owner edits it, on `main`. A new session continues
from here. Newest digest line last.

## Slices

Status: `[ ]` sketched, `[~]` in flight (with its branch), `[x]` delivered.

Step 1 (Make, J1). A first cut; the product owner refines each sketch before it hands it out.

- [x] **S1 Hello page.** Delivered 2026-11-10 (main `bacecdc`). Sol runs: pkg 1, host 1, dev 2.
- [x] **S2 The page calls its logic.** Delivered 2026-11-10 (main `7eb55ad`). Sol runs: pkg 1, host 2, dev 2.
- [~] **S3 An edit keeps the data.** (branch `slice/s3`) The agent changes the counter's logic and
  page while Alice has it open; `serve` again shows the new revision with the same count, and the
  page updates by itself when the count changes.
  - Outcomes (user-sim, on the real programs):
    1. After two `api.add`, change `lib.rs` to add 10 and edit `ui/index.html`; `serve --json` gives
       `reused: true`, the page returns the new bytes, `api.get` returns 2, the next `api.add` 12.
    2. Two pages open on the mod (headless browser): an Add in one shows the new count in both,
       through the `counted` event; the template's `app.js` uses `watane.on`.
    3. An `/events` stream on the page origin delivers each `counted` event with its count, in
       order.
    4. An edit that breaks the build: `serve --json` exits 1 with `build.compile`, and the running
       preview still serves the old revision and its count.
  - Touches: host (update of an installed mod keeps state by identity, the page event bus and
    `GET /events`, `watane.on` in `watane.js`, outlet imports emit instead of no-op), dev (`serve`
    reuse with a logic part, template `app.js`).
  - Interfaces it creates: the `/events` wire format and `watane.on` (host 8.x, 9).
  - Leaves out: delivery to other mods and subscriptions (step 2), toolsets, home mode.
- [ ] **S4 The agent fixes a failing test.** `watane-dev test` runs unit and contract tests; a broken
  export gives a result the agent can act on (`errors`, `next`), and the fix makes it pass.
  Parts: host (`Harness`), dev (unit, contract, fakes).
  Also from S2's user-sim: `mod.dependency` and `mod.cargo_key` give `line: null` for
  `logic/Cargo.toml`; a `/call` without `Origin` gets a 403 that does not name the header.
- [ ] **S5 The agent reads through a toolset.** The preview prints a toolset token; an MCP client lists
  the mod's exports and calls one. Parts: host (toolsets, MCP).
- [ ] **S6 J1.** The reading-list story of J1 as a journey test in `dev/tests/`, including a change
  that shows in the running preview.

## Stage coverage

One row per stage of each subsystem's build plan (its TECHNICAL "Build plan" table), with the
step that needs it. Status: `open`, `part` (what is missing), `done` (slice). A step is done only
when its journey passes and every stage of that step is `done`, or a deputy decision moves the
rest to a later step. Leftover `ponytail:` comments count as missing parts (`git grep ponytail:`).

| Subsystem | Stage | Step | Status |
|---|---|---|---|
| pkg | 1 names, 2 meta/deps, 3 world/folder/grant | 1 | done (S1) |
| pkg | 4 component | 1 | done (S2) |
| pkg | 5 package (pack, unpack) | 1 | open; host's temporary digest waits on it |
| pkg | 6 add_provider | 2 | open |
| pkg | 7 release | 3 | open |
| host | 1 runtime core | 1 | done (S2) |
| host | 2 links and events | 1, 2 | part: page bus in S3; links, delivery, hops in step 2 |
| host | 3 server and pages | 1 | part: `/events` in S3; `tests/hostile.rs` page rows |
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

## Friction

- S1: each foreman turn's closing text reached the product owner as a progress update. Fixed by
  the human's rule (1eed839): foremen report only when blocked or delivered.
- S2: a brief left out the subsystem's error-code module (`dev/src/outcome.rs`); Sol stopped
  halfway. Briefs should always allow it.
- S2: Sol's sandbox refuses nested `sandbox-exec`, so DEV-S-13 runs only in the validator.
- S2: host program tests with 2 s and 10 s limits fail now and then under heavy load.

## Digest

- S1: a coding agent runs `watane-dev new hello`, `check` and `serve --json`, opens
  `http://hello.localhost:<port>/` and sees the page; an edit shows after `serve` again;
  `serve --stop` ends the preview.
- S2: the template is a counter; Alice clicks Add on the page and the count goes up, stored by the
  host; a runaway export is stopped and the mod keeps answering.
