Subsystem: app
Effort: low
Depends on: none
Slice: S29 — Carol finds Alice's mod in her app (J4); user-sim fix.

## Goal
The own index row in the find view says how many mods the own index lists. Today it counts only
the own published releases, so after Carol lists Alice's mod it still says "0 mods".

## Acceptance
- AC1 The red test `app/src/core.rs` `find_integration_tests::regress_s29_own_index_counts_listed_mods`
  passes: the own `IndexRow.mods` is the number of live `list` listings of the own channel
  (includes are not mods). Decision (foreman): own releases are not counted; they show as `own`
  hits in search.
- AC2 Nothing else changes: `cargo test` and `cargo test --features driver` (not front) pass in
  `app/`; clippy and fmt clean. If an existing test asserted the old own count, list it.

## Spec
app `TECHNICAL.md` 7 (`IndexRow.mods`), 9.6. Add one clause to 9.6 `indexes_list`: the own row's
`mods` counts its live `list` listings.

## Interface
None changes.

## Files
`app/src/core.rs` (or `app/src/find.rs` if `index_rows` lives there), `app/TECHNICAL.md`.

## Changed tests
Search `app/src` and `app/tests` for assertions on the own row's `mods` (e.g. `ac3_indexes_put_own_channel_first...`,
`tests/find.rs`); none is expected to assert the release count.

## Out of scope
Any wording change.
