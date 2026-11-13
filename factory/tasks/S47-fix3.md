Subsystem: dev
Effort: low
Depends on: none
Slice: S47 — J5 passes again (dev fix)

## Goal

Bob runs `new my-ratings --from <link>` on a package with `logic.wasm` and no
`logic/Cargo.toml`. S47 keeps that `logic.wasm`, but it implements the origin's
WIT package (`mods:ratings`), and `new` renames `wit/world.wit` to
`mods:my-ratings`, so `test` fails with `test.contract` and Bob cannot rebuild
it. This cannot work without source, and refusing the rename would break J5,
which derives under a new slug and then writes its own logic. Foreman decision:
`new --from` keeps `logic.wasm` and, only when the slug changes and the package
has no `logic/Cargo.toml`, adds a warning that tells Bob before `test` does.

## Acceptance

AC1 New slug, prebuilt package: exit 0, `logic.wasm` kept, `warnings` holds one
    `new.prebuilt_renamed` with message
    `logic.wasm was built for <origin slug>, not <new slug>: it will fail the contract tests until the mod has logic source`
    and fix
    `Write the logic source under logic/ (logic/Cargo.toml and logic/src) for <new slug>, or run new --from again with the slug <origin slug>`.
AC2 Same slug (explicit or default), prebuilt package: no warning.
AC3 Package with `logic/Cargo.toml`: no warning (`logic.wasm` dropped, as now).
AC4 The user-sim red test `build_on::regress_s47_renamed_prebuilt_mod_passes_test`
    (dev/tests/build_on.rs, commit 048f2db) asserted that `test` passes, which
    cannot hold (Goal). Rename it to `regress_s47_renamed_prebuilt_mod_warns`
    and make it assert AC1, keeping its setup. Cover AC2 with an assertion in an
    existing same-slug test (for example the S47-1 test
    `ac1_derive_keeps_only_logic_wasm_with_explicit_and_default_slug`).
AC5 dev's full suite stays green.

## Spec

dev/TECHNICAL.md: the warning-codes paragraph (near "Warning codes (step 5
first") and 10.2 (a step after the rename of step 7). Add the code to the Code
enum (section 13 listing near `NewForkUnavailable`) if dev's enum lists warnings
there.

## Interface

None outside dev.

## Files

dev/src/build_on.rs, dev/src/outcome.rs (or wherever `Code` lives),
dev/tests/build_on.rs, dev/TECHNICAL.md.

## Changed tests

`build_on::regress_s47_renamed_prebuilt_mod_passes_test` (AC4). No other test
asserts `new --from` warnings are empty (searched dev/tests for `warnings`);
if one does, add the case to it rather than weakening it.

## Out of scope

`test.contract`'s fix text; `test`/`serve` accepting `--home`; the app's
reading-list fixture's old `tests/books.json` form.
