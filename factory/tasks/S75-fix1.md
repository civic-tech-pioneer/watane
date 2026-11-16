Subsystem: app
Effort: low
Depends on: none
Slice: S75 — Alice pairs her laptop and her server from the app, and J8 passes (app stage G)

## Goal
On a home whose identity signed a statement (set a name, published), clicking Join no longer asks
"This home's identity <fp> has signed nothing. Joining your machines deletes it ..." (a false
statement). APP-F-54: the question is for a home whose identity signed nothing; a home that signed
is refused with the cause. Today `app/ui/machines.js:16-18` asks whenever `status.identity` is set.

## Acceptance
- AC1 The red test `app/tests/machines.rs::regress_s75_join_question_on_signed_home` (commit
  5572b21, from user-sim) passes; nothing else changes.
- AC2 A home that signed nothing still gets the question (S30, `ac3_s30_join_light_and_refuse_used_identity`
  stays green), and a home that signed still gets node's `IdentityInUse` cause with its key file
  unchanged.
- AC3 `cargo test -p watane-app --features driver --test machines -- --include-ignored` and the
  lib tests pass; clippy `--all-targets --features driver -D warnings`, fmt clean. Never run front
  tests (`--skip front:` does not skip them; skip by name: ac1_consent_stays_above_hostile_and_takes_focus,
  ac3_consent_takes_focus, ac3_reopening_an_open_mod_focuses_it, ac4_approve_delay_and_deny_focus).

## Spec
- app/FUNCTIONAL.md APP-F-54, S30. app/TECHNICAL.md 7 (`Status`), 9.8 (`machine_join`).
- The simplest fix: `Status.identity` gains `signed: bool` (the own channel has a head:
  `dist.own()?.head.is_some()`, node 5.5's own test), and the page asks the question only when
  `signed` is false; when true it calls `machine_join` directly and shows the refusal. Write the
  field into TECHNICAL 7's `Status` and the sentence in 9.8.

## Interface
- `Status.identity.signed: bool` (new, page-only consumer).

## Files
- app/src/core.rs (or wherever `Status` is built), app/ui/machines.js, app/TECHNICAL.md 7 and 9.8,
  app/tests/machines.rs only if the red test needs no change (it should not).

## Changed tests
- none (searched app/tests for `has signed nothing`: only S30's test and the red test).

## Out of scope
- The other user-sim notes (spent-code text, reconnect, last seen, `--help`).
