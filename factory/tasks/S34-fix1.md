Subsystem: host
Effort: low
Depends on: S34-1
Slice: S34 — Eve's host binds a mod that fits (host stage 9)

## Goal
Removing a mod never fails because of the links it leaves behind, and a bind always takes effect
in the install the participant confirms. Today, in a preview with three `shelf` mods and `recs`
bound to one, `remove` of the bound shelf fails with `ambiguous_provider` (the relink after remove
in `Catalog::relink`). Also, `plan_with` accepts a bind whose `to` is neither installed nor in the
plan, which later moves `recs` silently when `to` is installed, and accepts two given binds for one
consumer and provider.

## Acceptance
- AC1 The red test `host/tests/binds.rs::regress_s34_preview_remove_of_bound_shelf_with_two_shelves_left`
  passes unchanged: remove succeeds, the mod is gone, `recs` has no binds (its link to shelf is
  unbound, `provider: None`).
- AC2 `plan_with` with a bind whose `to` is neither installed nor a source of the plan:
  `bad_request` naming the bind.
- AC3 `plan_with` with two given binds for the same consumer and `provider`: `bad_request`.
- AC4 Every other test passes unchanged except ones that relied on a pending (not yet installed)
  `to` — list any you change. `cargo test -p watane-host -- --include-ignored --skip example`,
  clippy `--workspace --all-targets -D warnings`, `cargo fmt --check` pass.

## Spec
host TECHNICAL 4.5 rules 1 and 4 (already edited by the foreman), 4.3 step 5, 4.4 `remove`.

## Interface
none changes.

## Files
`host/src/catalog.rs`, `host/src/lib.rs` (only if needed), `host/tests/binds.rs`.

## Changed tests
Searched `host/tests/binds.rs`: none known to give a pending `to`; check while building.

## Out of scope
A clearer `ambiguous_provider` message on plan; the app's pick request (S37).
