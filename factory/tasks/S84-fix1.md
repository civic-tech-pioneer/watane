Subsystem: app
Effort: medium
Depends on: none
Slice: S84 — The manager looks like the design (app)

## Goal
Close the gaps a side-by-side comparison of every view against the human's boards found
(`spec/design/manager/*.dc.html`; screenshots of both sides, light and dark, in
`spec/design/manager/screens/`, `<view>-app-*.png` beside `<view>-design-*.png`). Mostly CSS and
markup in `app/ui/`; a few word fixes. Read the board's CSS for each value; never copy board code.

## Acceptance
Each line: what the board has → what the app must do. Selectors are the likely place.
- AC1 Tiles (`button.tile`): content top-aligned (not centred: the tile inherits `.btn`'s
  `justify-content:center`), text weight 400 with the by-line 13 px weight 400; badge `right:16px`.
  Simplest: tiles stop carrying `.btn`, or `.tile` overrides it.
- AC2 Library spacing: `.library-list` padding 28 / 24 / 40; heading margin-bottom 18 and
  `align-items: baseline`.
- AC3 Line height: normal line-height for controls, tiles, header and rows; 1.5 only for running
  paragraphs (the header 64 px as on the board).
- AC4 Icons: the board's inline SVG icons (search, plus, link, refresh, check, warning, globe,
  download arrow, and the first-start cards' link / search / folder; the four-squares title glyph)
  replace the text glyphs ⌕ ＋ ↗ ↻ ✓ ⚠ ◎ ↓ ▱, at the board's sizes and stroke, `aria-hidden`, coloured by
  `currentColor`. Inline `<svg>` in index.html or created with `createElementNS` in JS; no `style=`
  attribute (the CSP), sizes by class. The refresh icon is the one that turns while checking.
- AC5 Installed note: placed first in the Library column, above the `Library` heading; one line:
  `<name> is installed.` (600) then the muted `It adds to …` inline (a span, not a block); the 18 px
  SVG check in `--ok`, stroke 2.4. Several new mods: `A and B are installed.`, `A, B and C are
  installed.`, with no `It adds to` line (TECHNICAL 5.3.3, changed in 0b348a9).
- AC6 Confirm panel: 1 px `--line` border, radius 14, on `--bg`; flex column, gap 12; title 15 px 600;
  buttons right-aligned; `#dependency-warning` with an 18 px SVG triangle beside a column of lines
  (gap 4), radius 10, padding 10 × 12; the radio labels flex, gap 8, centred, rows 8 px apart. The
  start row stays visible while the panel is open: the panel replaces only the action buttons
  (5.3.4 item 5, changed in 0b348a9).
- AC7 Details: grant glyph in `--muted`, 14 px; grant row gap 12; connection chips in a flex-wrap
  with gap 6, top-aligned with their label (no 6 px top margin), rows 12 px apart; notice band
  padding 12 × 14.
- AC8 Monograms: letter 12 px at 28 px, 11 px at 24 px (and the board's size for each other monogram
  size).
- AC9 First start: h1 30 px; the line under it 16 px, line-height 1.5; the picture's margin 12 then
  gap 10; card headings 16 px weight 600; the link field on `--bg`; buttons and fields 40 px, radius
  10 in this view (5.3.3, changed in 0b348a9).
- AC10 Drop target: the second line 14 px `--muted`.
- AC11 Find: a hit whose mod is installed in another version shows `Installed` (the link to Library)
  beside its `You have <version>` tag, not `Install…` (5.3.5 item 4, changed in 0b348a9). Listing
  words name parties by name only, as APP-F-43 writes them: `listed by Alice through Ivy`, own index
  `listed by you` (fingerprints stay only in the publisher line; the fingerprint stands in for a
  missing name).
- AC12 Updates: a withdrawal card has no `Not now` (5.3.6: only the new-grant card has it); its
  muted words are one 13 px line; the version beside a name 15 px weight 400 `--muted` in the card
  head and 14 px in the done rows; the disabled `Checking…` button at opacity .7 (not .5). With no
  followed channel `#checked-at` reads `Checking…` while it runs, not `Reading 0 channels…` (5.3.2,
  changed in 0b348a9).
- AC13 Settings: its header pushes the mark to the right end (as the manager header's spacer);
  its buttons use `.btn` (primary where a section has one action that matters most) and fields the
  36 px shape; no ragged `margin:4px` stacking. All Settings and machines ids unchanged.
- AC14 Tests: extend `tests/support/manager_dom.mjs` `look`/`find-updates` (or `tests/look.rs`) for
  the behaviour changes only: AC5's word forms (one, two, three names; no adds-to line for several),
  AC6's start row visible with the panel open, AC11's `Installed` for another version and the
  name-only listing words, AC12's withdrawal card without `Not now` and `Checking…` with no channel.
  Pure CSS values need no test (the static style test stays as it is unless a token changes).
- AC15 `cargo test -p watane-app` and `--features driver -- --include-ignored` (front four skipped)
  green; clippy and fmt clean; static_style and static_config pass.

## Spec
app/TECHNICAL.md 5.3.1 to 5.3.6 (as of 0b348a9); app/FUNCTIONAL.md APP-F-43; the boards
`Main.dc.html`, `Find.dc.html`, `Updates.dc.html`, `Installed.dc.html`, `Empty.dc.html`.

## Interface
None with another task.

## Files
app/ui/**, app/tests/look.rs, app/tests/support/manager_dom.mjs, app/tests/find.rs and
app/tests/j4.rs (only if a listing-words or Installed assertion there changes with AC11),
app/NOTES.md.

## Changed tests
Search app/tests for `listed by`, `Install…`, `Not now`, `Reading 0`, `It adds to` and
`installed-note` before changing them. Any test asserting the fingerprint in listing words, `Install…`
on a hit with another version installed, or `Not now` on a withdrawal card changes its words only, to
the facts above, with a NOTES line each. Never weaken an assertion.

## Out of scope
Rust. The consent view's layout and words. The refresh-on-focus of program installs (later slice).

## Running tests
Build in `$CARGO_TARGET_DIR` (main's target); never create a target in the worktree. Never run these
four (they take the screen): `--skip ac1_consent_stays_above_hostile_and_takes_focus --skip
ac3_consent_takes_focus --skip ac3_reopening_an_open_mod_focuses_it --skip
ac4_approve_delay_and_deny_focus`. If your sandbox cannot start window tests (HIServices XPC), say
which ACs are PARTIAL; the validator runs them. No test may grep spec text (a `.md` file).
