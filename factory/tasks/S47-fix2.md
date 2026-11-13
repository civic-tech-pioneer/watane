Subsystem: app
Effort: low
Depends on: none
Slice: S47 — J5 passes again (dev fix)

## Goal

J5 (app/tests/j5.rs `ac1_journey_j5`) passes again. S44 (2571d78) changed the
`indexes_list` listing `target` on purpose from the raw ReleaseRef to display
text (`"<slug> by <party label>"`, app/src/core.rs `index_row`; its unit test
near app/src/core.rs:1421 asserts the new form). J5 still expects the raw
ref `<carol key>/ratings` at app/tests/j5.rs:280 and fails there. Update J5's
assertion to the S44 form; the app's code does not change.

## Acceptance

AC1 J5's listing check asserts `target` is the display text for Carol's
    ratings (`"ratings by "` followed by Carol's party label, which contains
    "Carol"), kind `list`, note `used by stats`.
AC2 `WATANE_DRIVER_SCREEN=hidden cargo test --features driver --test j5 --
    --ignored ac1_journey_j5` passes on the Mac (the foreman runs it; run it
    yourself if your sandbox allows the app window, else say so).

## Spec

app TECHNICAL: the section that documents `indexes_list` (S44 wording); no
change unless it still says the target is a ReleaseRef.

## Interface

None.

## Files

app/tests/j5.rs; app's TECHNICAL only if it contradicts S44.

## Changed tests

app/tests/j5.rs `ac1_journey_j5`: the listing assertion (AC1).

## Out of scope

Any change to `indexes_list` output.
