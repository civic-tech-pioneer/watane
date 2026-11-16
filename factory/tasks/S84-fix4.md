Subsystem: app
Effort: low
Depends on: none
Slice: S84 — The manager looks like the design (app)

## Goal
The update band's button in the Library details is short, so it sits beside the band text as on the
Main board; the band text already names what the offer asks for.

## Acceptance
- AC1 In the details band only, the offer button reads `Install <v>…` when the offer's `offerGrants`
  is not empty, else `Install <v>` (app/TECHNICAL 5.3.4 item 3). It still calls `release_install` with
  the offer. The Updates card's offer button (`Review…`, or `offerLabel` on a withdrawal card) and every
  other use of `offerLabel` are unchanged.
- AC2 Tests: the `update-band` scenario (tests/support/manager_dom.mjs, run from look.rs) asserts
  `Install 0.4.0…` with one grant and `Install 0.4.0` with none, in place of offerLabel's text; the
  window AC2 layout check in look.rs (`windows::ac3_ac4_ac6_ac8_library_details_confirm_install_and_badges`)
  now also asserts that, at the default window size with a one-grant offer, the button sits on the
  text's row at the band's right (it uses a literal label today: update it). Any other assertion of the
  band button's text changes its words only.
- AC3 `cargo test -p watane-app` and `--features driver --test look -- --include-ignored`; clippy and
  fmt clean. Only those window tests.

## Spec
app/TECHNICAL.md 5.3.4 item 3 (as of this brief's commit).

## Interface
None.

## Files
app/ui/app.js, app/ui/words.js, app/tests/look.rs, app/tests/support/manager_dom.mjs, app/NOTES.md.

## Changed tests
The `update-band` scenario's offerLabel button assertion and the look.rs window check's `Install
2.0.0` literal (search for `Install ` in look.rs and manager_dom.mjs).

## Out of scope
Everything else.

## Running tests
Build in `$CARGO_TARGET_DIR` (main's target); never create a target in the worktree. Run only the
window tests named above. No test may grep spec text.
