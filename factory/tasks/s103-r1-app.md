Subsystem: app
Effort: low
Depends on: none
Slice: S103 — Alice gives a mod's agents a folder, and can take it back (review r1, app part)
## Goal
Answers Codex review r1 of PR #26 for the app: the toolset consent shows every function name
before Approve, never "and N more" (F1.4; app TECHNICAL 19.2 as the foreman updated it in
3f735c7); an agent request answered Deny with a choice or a withheld list is `WrongAnswer` and
leaves the question open (F1.5; S103-3 AC2).
## Acceptance
Red first: each test fails on the current code before its fix.
AC1. F1.4: `words::tests`: a toolset of 13 and of 40 functions gives a question holding every name,
     in toolset order, separated by `, `, with no `more`; the window test
     `gui_shell::s58_agent_consent_words_literal_text_and_no_stop` (or a new `s58_` one) shows a
     13-name toolset with every name present as text in the consent window, the view scrolls or
     wraps.
AC2. F1.5: `consent::tests`: for an Agent request `answer(false, Some("x".into()), vec![])` and
     `answer(false, None, <a non-empty withheld>)` each return `WrongAnswer` and the request is
     still the head of the queue; a plain Deny still succeeds; the same at the broker level.
AC3. clippy (`--features driver`) and nightly fmt clean; `factory/check.sh s103-r1-app 3f735c7`
     passes apart from the 5 `support_selftest` tests that need IPv6 `::1` (absent in this
     container; they fail on main too); run the `s58_`/`s60_` tests under `xvfb-run -a`.
## Spec
app TECHNICAL 19.2 (words, `<functions>`), 6.3, 17.4; FUNCTIONAL APP-F-82, S58.
## Interface
none.
## Files
`app/src/words.rs`, `app/src/consent.rs`, `app/ui/` (consent view styles if a long list needs
wrapping), `app/tests/it/gui_shell.rs`, `app/TECHNICAL.md` (only to state the built behaviour).
## Changed tests
`words::tests::ac1_agent_consent_words`: its `and <n> more` expectation becomes the full list.
## Out of scope
The host (s103-r1-host).
