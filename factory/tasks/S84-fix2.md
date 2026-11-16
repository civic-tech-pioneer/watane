Subsystem: app
Effort: low
Depends on: none
Slice: S84 — The manager looks like the design (app)

## Goal
Close the last gaps of the second side-by-side comparison against the boards
(`spec/design/manager/*.dc.html`; screenshots in `spec/design/manager/screens/`). CSS and markup only.

## Acceptance
- AC1 Details rhythm (Main board): the panel's blocks are 26 px apart, as the board's aside (a flex
  column with gap 26): summary → notice → actions (or the confirm panel) → start row → the labelled
  sections. Simplest: `section.mod-details` a flex column with gap 26 (keep `hidden` working:
  `[hidden]{display:none}` must still win).
- AC2 `#updates-count`: the board's pill: inline-flex, centred, min-width and height 18 px, radius 9,
  11 px weight 600; hidden at 0 as before.
- AC3 Find's `Installed` link-button (`a.btn`) in `--ink`, not the accent of `a`.
- AC4 Grant text (`.grant-row` text) line-height 1.4.
- AC5 Settings: one `.btn.primary` per section (identity, site, backup, agents, own machines), the
  action that matters most there; every other button plain `.btn`. No tall empty band between Site and
  Agents: empty or hidden blocks take no space (e.g. `#own-link` when empty). Ids unchanged.
- AC6 `cargo test -p watane-app`, and `--features driver --test look -- --include-ignored`; clippy and
  fmt clean; static_style passes. No new test unless a behaviour changes (none does).

## Spec
app/TECHNICAL.md 5.3, 5.3.1, 5.3.2, 5.3.4, 5.3.5; the boards `Main.dc.html`, `Find.dc.html`.

## Interface
None.

## Files
app/ui/style.css, app/ui/index.html, app/ui/app.js, app/ui/machines.js, app/NOTES.md.

## Changed tests
None (searched look.rs and manager_dom.mjs for `primary` in Settings: re-check before you change a
button's class; a test that counts primaries changes its count only).

## Out of scope
Everything else.

## Running tests
Build in `$CARGO_TARGET_DIR` (main's target); never create a target in the worktree. Run only the
window tests named above, never the whole driver suite. No test may grep spec text.
