# Plan

The product owner's state; only the product owner edits it, on `main`. A new session continues
from here. Newest digest line last.

## Slices

Status: `[ ]` sketched, `[~]` in flight (with its branch), `[x]` delivered.

Step 1 (Make, J1). A first cut; the product owner refines each sketch before it hands it out.

- [x] **S1 Hello page.** Delivered 2026-11-10 (main `bacecdc`). Sol runs: pkg 1, host 1, dev 2.
- [~] **S2 The page calls its logic.** (branch `slice/s2`) A coding agent runs `watane-dev new
  hello` and `serve --json`; Alice opens the page, clicks its button, and sees the count go up.
  - Outcomes (user-sim, on the real programs):
    1. `check --json` on a new mod is `ok` and reports the export `mods:hello/api@0.1.0`, the event
       interface `mods:hello/changes@0.1.0` and the storage grant.
    2. `serve --json` builds `logic.wasm` offline, from the allowlist that ships with dev, and
       exits 0. The page's script loads `/watane.js` and calls `api.add`; a POST to `/call` on the
       page origin, as `watane.js` sends it, returns 1, then 2; `api.get` returns 2.
    3. With `logic/build.rs` added, or `rand` in `[dependencies]`, `serve --json` exits 1 with
       `mod.logic_file` or `mod.dependency` naming the item, and cargo did not run.
    4. A compile error in `logic/src/lib.rs` gives exit 1 and `build.compile` with that file and
       its line.
    5. An export with an endless loop fails its `/call` within the time limit; the next call
       to another export still answers.
  - Touches: pkg (stage 4, `Mod::check_component`), host (stage 1 runtime core: engine, limits,
    epoch ticker, state store, `watane:host/state` and `caller`, the JSON encoding, top-level call;
    from stage 3: `POST /call`, `/watane.js`; `HOST_WIT`; install accepts a logic part), dev
    (stage 2: allowlist, `scripts/vendor.sh`, toolchain, builder; the template becomes the counter
    of dev §4, without `tests/`).
  - Interfaces it creates: `HOST_WIT`, `check_component`, the `/call` body and the JSON encoding
    (host 6.9, 8.2), `watane.js`, the state store in a home, where dev finds the vendored allowlist.
  - May fake: `changes::counted` is accepted and delivered nowhere until events land. `serve`
    runs check and build only; unit and contract tests join in S4.
  - Leaves out: events, links between mods, unit and contract tests, toolsets, the outside-read
    check and reproducible bytes of the build (DEV-S-10, DEV-S-12; they matter from step 3), home
    mode.
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

## Friction

- S1: each foreman turn's closing text reached the product owner as a progress update. Fixed by
  the human's rule (1eed839): foremen report only when blocked or delivered.

## Digest

- S1: a coding agent runs `watane-dev new hello`, `check` and `serve --json`, opens
  `http://hello.localhost:<port>/` and sees the page; an edit shows after `serve` again;
  `serve --stop` ends the preview.
