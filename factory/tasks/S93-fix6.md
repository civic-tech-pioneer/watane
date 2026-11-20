Subsystem: dev
Effort: medium
Depends on: none
Slice: S93 — dev's last misleading lines (user-sim REJECTED: stale sibling with its own world).

## Goal
The red test `builder::regress_s93_generate_path_into_a_sibling_with_its_own_world_reads_outside`
(dev/tests/it/builder.rs, 5b932b4, `#[ignore]` slow) passes unedited. When a stale sibling's world
name differs, rustc's text names no sibling path ("World `other` not found in package
`mods:hello@0.1.0`"), so the rendered-text scan cannot see it. Detect the cause directly: when the
build fails, read the `wit_bindgen::generate!` `path:` literals in the copy's `logic/src/**/*.rs`
(the string literal after `path:` inside `generate!(...)`/`generate!({...})`), resolve each
against the copy's `logic/` (wit-bindgen's base, CARGO_MANIFEST_DIR), normalize, and report each
one that leaves the copy as `build.reads_outside` (one problem, the wit fix), in place of the
compile errors — the same output the existing scan gives. Keep `sibling_paths` for paths rustc
names; drop it only if the new check makes it redundant for every existing test (then say so).
Pick the simplest code; a regex-free scan of the literal is fine.

## Acceptance
- AC1 the red test passes unedited (`cargo test --test it -- --ignored builder::` from dev/).
- AC2 every existing `s93_*`, `adv_s93_*` builder test passes unedited.
- AC3 unit test: a `generate!({ world: "x", path: "../../other/wit" })` source with copy
  `/c/build/me` gives `/c/build/other/wit`; `path: "../wit"` gives nothing.

## Spec
dev/TECHNICAL 6.4 step 6: one sentence for the generate! path check.

## Interface
None.

## Files
dev/src/builder.rs, dev/TECHNICAL.md 6.4 only.

## Changed tests
None.

## Out of scope
cli/api texts (S93-fix5), pkg.
