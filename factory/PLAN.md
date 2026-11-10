# Plan

The product owner's state; only the product owner edits it, on `main`. A new session continues
from here. Newest digest line last.

## Slices

Status: `[ ]` sketched, `[~]` in flight (with its branch), `[x]` delivered.

Step 1 (Make, J1). A first cut; the product owner refines each sketch before it hands it out.

- [~] **S1 Hello page.** (branch `slice/s1`) A coding agent runs `watane-dev new hello`, then
  `watane-dev serve --json`, opens `result.pages[0].url` and sees the template page.
  - Outcomes (user-sim, on the real `watane-dev` and `watane`):
    1. `new hello --json` exits 0 with the envelope; `check --json` in the folder is `ok` and reports
       no grants, exports, imports or events. `new Hello --json` exits 2 with `usage.slug`.
    2. `serve --json` exits 0; a GET of `result.pages[0].url` (`http://hello.localhost:<port>/`)
       returns the bytes of `ui/index.html` with the page CSP; the page's script loads from a file.
    3. After an edit of `ui/index.html`, `serve --json` again gives `reused: true` and the new bytes.
    4. `serve --stop --json` exits 0; the port refuses connections; the preview home is gone.
    5. With `WATANE_BIN` naming a missing file, `serve --json` exits 3 with `env.watane_missing`.
  - Touches: pkg (stages 1 to 3: names, `mod.toml`, world, folder rules; `Mod::read` of a page-only
    mod), host (the program `watane --preview`, ready line, `run.json`, `GET /api/status`,
    `POST /api/install` of page-only folders, page origin `<local>.localhost` with the CSP, SIGTERM),
    dev (stage 0 envelope and exit codes, `new` and `check` with a page-only template, `serve`,
    `serve --stop`).
  - Interfaces it creates: pkg `Mod::read` and its accessors; host `Ready`, `RunInfo`, the program's
    arguments, `/api/status`, `/api/install`; dev's result envelope.
  - May fake: the template is page-only (mod.toml, `wit/world.wit` with an empty world, `ui/`,
    the agent guide); logic and tests join in S2 and S4. The ready line's `mcp` and `token` are
    `null` until S5. The host may install a folder without pack and unpack (pkg stage 5), as long
    as outcome 3 holds.
  - Leaves out: logic, runtime, state, `/call`, `watane.js`, toolsets, `doctor`, allowlist,
    builder, home mode and confirmation.
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

(none)

## Friction

(none)

## Digest

(none)
