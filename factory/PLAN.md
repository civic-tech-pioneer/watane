# Plan

The product owner's state; only the product owner edits it, on `main`. A new session continues
from here. Newest digest line last.

## Slices

Status: `[ ]` sketched, `[~]` in flight (with its branch), `[x]` delivered.

Step 1 (Make, J1). A first cut; the product owner refines each sketch before it hands it out.

- [x] **S1 Hello page.** Delivered 2026-11-10 (main `bacecdc`). Sol runs: pkg 1, host 1, dev 2.
- [ ] **S2 The page calls its logic.** The template gets a logic part; a button on the page calls an
  export and shows the answer. Parts: pkg (world, component check), host (runtime core, `/call`),
  dev (builder).
- [ ] **S3 Data stays.** The template counts clicks in `watane:host/state`; after `serve --stop` and
  `serve` the count is still there. Parts: host (state store, grant from the world), dev (template).
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

## Friction

- S1: each foreman turn's closing text reached the product owner as a progress update. Fixed by
  the human's rule (1eed839): foremen report only when blocked or delivered.

## Digest

- S1: a coding agent runs `watane-dev new hello`, `check` and `serve --json`, opens
  `http://hello.localhost:<port>/` and sees the page; an edit shows after `serve` again;
  `serve --stop` ends the preview.
