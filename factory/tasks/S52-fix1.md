Subsystem: app
Effort: low
Depends on: S52-1
Slice: S52 — The front tests can be run

## Goal

A skipped front test is visible without `--nocapture`. Today the skip line is
`eprintln!`, which libtest captures and hides when the test passes, so
`cargo test --features driver -- --ignored` shows a skipped front test as a plain
`ok`, the same as one that really ran (user-sim finding).

## Acceptance

AC1 The skip line in app/tests/support/driver.rs is written straight to the
process's stderr (`writeln!(std::io::stderr(), ...)`, which libtest's capture does
not intercept), not with `eprintln!`. With the screen locked,
`cargo test --features driver --test gui_shell -- --ignored --exact ac3_consent_takes_focus`
(no `--nocapture`) shows `front test skipped: the screen is locked (CGSSessionScreenIsLocked)`.
Run that only if `ioreg -n Root -d1 | grep -c '"CGSSessionScreenIsLocked"=Yes'` prints 1
right before; never run a front test on an unlocked screen.
AC2 `ac2_front_skips_locked_and_rejects_unknown_ioreg` still passes (adapt it only if
it relied on capture; keep what it asserts).

## Spec

app/TECHNICAL.md section 11, the locked-screen sentence: say the line shows without `--nocapture`.

## Interface

None.

## Files

app/tests/support/driver.rs, app/tests/support_selftest.rs, app/TECHNICAL.md.

## Changed tests

none (searched app/tests for the skip line: only support_selftest ac2 reads it).

## Out of scope

Anything else.
