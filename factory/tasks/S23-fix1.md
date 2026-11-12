Subsystem: dev
Effort: low
Depends on: none
Slice: S23 — The maker's agent stumbles less (user-sim rejection 1).

## Goal
`watane-dev add . --dir <missing folder>` reports `usage.not_a_mod` naming the folder (as
`check --dir <missing>` does), not `internal: No such file or directory`. And the declined-install
fix names a command that works as written.

## Acceptance
- AC1 The red test `deps::tests::regress_s23_add_missing_dir_is_not_a_mod` (dev/src/deps.rs,
  commit f40187d) passes; do not change the test. Exit class follows: usage (2).
- AC2 The `host.declined` fix says "then run install again with --wait <seconds>" (today
  "--wait" alone, which fails as `usage.args` when typed literally). Update the three strings
  in dev/src/api.rs tests/code and TECHNICAL 2.2's row; nothing else changes.
- AC3 Default tests, clippy, fmt pass. Run the default tests only (not `--ignored`).

## Spec
dev TECHNICAL 2.2, 9.5.

## Interface
None.

## Files
dev/src/deps.rs, dev/src/api.rs, dev/tests/install.rs (only if it pins the fix text), dev/TECHNICAL.md (2.2).

## Out of scope
Everything else in the user-sim's "felt wrong" list. Do not edit dev/AGENTS.md; give your spec-gap line (if any) in your final message.
