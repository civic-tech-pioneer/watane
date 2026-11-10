# Plan

The product owner's state; only the product owner edits it, on `main`. A new session continues
from here. Newest digest line last.

## Slices

Status: `[ ]` sketched, `[~]` in flight (with its branch), `[x]` delivered.

Step 1 (Make, J1). A first cut; the product owner refines each sketch before it hands it out.

- [x] **S1 Hello page.** Delivered 2026-11-10 (main `bacecdc`). Sol runs: pkg 1, host 1, dev 2.
- [x] **S2 The page calls its logic.** Delivered 2026-11-10 (main `7eb55ad`). Sol runs: pkg 1, host 2, dev 2.
- [ ] **S3 An edit keeps the data.** The agent changes `lib.rs` and the page; `serve` again
  installs the new revision in the running preview; the count is still there (DEV-S-22); a page
  sees the `counted` event. Parts: host (update keeps state by identity, page event bus,
  `/events`), dev (reuse). Re-sketch after S2.
- [ ] **S4 The agent fixes a failing test.** `watane-dev test` runs unit and contract tests; a broken
  export gives a result the agent can act on (`errors`, `next`), and the fix makes it pass.
  Parts: host (`Harness`), dev (unit, contract, fakes).
- [ ] **S5 The agent reads through a toolset.** The preview prints a toolset token; an MCP client lists
  the mod's exports and calls one. Parts: host (toolsets, MCP).
- [ ] **S6 J1.** The reading-list story of J1 as a journey test in `dev/tests/`, including a change
  that shows in the running preview.

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
