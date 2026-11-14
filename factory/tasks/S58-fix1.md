Subsystem: app
Effort: low
Depends on: S58-1
Slice: S58 — The app is complete and says what happened

## Goal

The install consent's per-mod release line reads name first, like every other
S58-1 AC2 value. Today app/ui/consent.js:73 shows
`<key>/<slug>@<version> by <party> · signature verified`.

## Acceptance

AC1 The red test `regress_s58_install_consent_release_line_names_first`
(app/tests/gui_shell.rs, mode `regress_s58` in app/tests/support/consent_dom.mjs)
passes: the line shows `<slug> <version> by <party label> · signature verified — <reference>`
(readable part first, the full reference last after ` — `). Nothing else changes;
other consent_dom modes still pass.

## Spec

app/TECHNICAL.md consent text rule written in S58-1 (name before key); update it
if it does not already cover this line.

## Interface

none (page only; the request already carries `release` and `publisher`; derive
slug and version from the reference or from fields already present).

## Files

app/ui/consent.js, app/tests/support/consent_dom.mjs only if an existing mode
asserts the old string, app/TECHNICAL.md, app/NOTES.md.

## Changed tests

Search app/tests/support/consent_dom.mjs and app/tests/*.rs for the old
`by ... · signature verified` string and update to the new fact.

## Running tests

`cargo test -p watane-app --features driver --test gui_shell regress_s58` and
`node tests/support/consent_dom.mjs <mode>` in app/. Never run the four
`#[ignore = "front: takes the screen"]` tests; never `--include-ignored`.

## Out of scope

Everything else in S58.
