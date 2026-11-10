# Plan

The foreman's state. A new session continues from here. Newest digest line last.

## Slices

Step 1 (Make, J1). A first cut; the foreman refines each slice before it starts.

- [ ] **S1 Hello page.** A coding agent runs `watane-dev new hello` and `watane-dev serve --json`,
  opens the URL, and sees the template page. Parts: pkg (slug, `Mod::read` for a page-only mod),
  host (program, serve one page on its own origin with the CSP, preview mode), dev (`new`, `check`,
  `serve`, `serve --stop`, the result envelope).
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
