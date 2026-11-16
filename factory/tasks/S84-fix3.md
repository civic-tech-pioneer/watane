Subsystem: app
Effort: low
Depends on: none
Slice: S84 — The manager looks like the design (app)

## Goal
The update band in the Library details says what the update is, as the Main board draws it, with a
compact offer button at its right.

## Acceptance
- AC1 For a mod with an `offer` and no `notice` or `error`, the band text (app/ui/app.js, the details'
  notice, ~line 140, today `An update is available.`) reads `Version <v> asks to <words>.` where `<v>`
  is the offer's version and `<words>` the `short` words (fall back to `text`) of `offerGrants`, first
  letter lower case, joined `A`, `A and B`, `A, B and C`; e.g. `Version 0.4.0 asks to connect to
  openlibrary.org.` With no `offerGrants`: `Version <v> is available.` (app/TECHNICAL 5.3.4 item 3).
  Notices and errors keep their own text.
- AC2 The offer button keeps its `offerLabel` text but is sized to its label at the right end of the
  band (the board's `.notice` row: text flexes, button does not stretch); it wraps under the text only
  when the panel is too narrow (flex-wrap), never full width by default.
- AC3 look.rs (or manager_dom.mjs's look scenario, run from look.rs) asserts both texts: an offer with
  one grant (`Version 0.4.0 asks to connect to openlibrary.org.`) and one with none (`Version 0.4.0 is
  available.`), and that the button text is `offerLabel`'s.
- AC4 `cargo test -p watane-app` and `--features driver --test look -- --include-ignored`; clippy and
  fmt clean. Only those window tests.

## Spec
app/TECHNICAL.md 5.3.4 item 3 (as of this brief's commit), 9.5; spec/design/manager/Main.dc.html (the
update band).

## Interface
None.

## Files
app/ui/app.js, app/ui/words.js, app/ui/style.css, app/tests/look.rs, app/tests/support/manager_dom.mjs,
app/NOTES.md.

## Changed tests
Search app/tests for `An update is available`; an assertion of it changes its words to AC1's.

## Out of scope
Everything else.

## Running tests
Build in `$CARGO_TARGET_DIR` (main's target); never create a target in the worktree. Run only the
window tests named above. No test may grep spec text.
