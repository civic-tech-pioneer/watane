Subsystem: install
Effort: medium
Depends on: none
Slice: S50c — a person installs watane on a clean Mac (install stages 4 and 5, J7)

## Goal
The two red tests of the S50c adversary pass: `install/tests/adv_s50c.rs`
`adv_s50c_origin_main_tag_passes_ancestor_check` and
`adv_s50c_link_after_check_lets_upgrade_delete_linked_folder`. The foreman changed the spec
for both (read `git diff 0036d84 -- install/TECHNICAL.md` after this brief's commit):
1. release.yml's tag check uses `refs/remotes/origin/main` (7.2 step 2); INST-S-29's rule in
   `workflow.rs` requires that full ref name.
2. install.sh: before the swap, the install folders are resolved again and compared with the
   step 4 values (3.6 step 3; on a difference: undo, exit 5, the new message); step 5 removes a
   `.old.<pid>` only when rule 4 holds against the new receipt.

## Acceptance
- AC1 Both red tests pass, unchanged (fix the test only if it asserts something the spec
  above contradicts; say so).
- AC2 A test: a link made in `~/.local/share/watane-dev` during staging (the `chmod_wait` fake,
  as the red test) makes the run exit 5 with first stderr line `watane install:
  ~/.local/share/watane-dev changed during the install: it now resolves to <home>/Documents`,
  the old version still installed, `~/Documents/vendor/marker` kept, no lock, no journal.
- AC3 `workflow.rs` fails on a copy whose check says `origin/main` instead of
  `refs/remotes/origin/main`.
- AC4 Every new status-dropping site goes through `check_signal` (3.8); status_audit.rs and
  check_signals.rs pass unchanged. All tests of watane-install pass, `-- --include-ignored`
  too; shellcheck, clippy, fmt clean.

## Spec
install/TECHNICAL 3.6 steps 3 and 5, 7.1, 7.2 step 2, 8.5 rule 4, 10 (the not-protected list),
3.8. install/FUNCTIONAL INST-S-29.

## Interface
none.

## Files
`install/install.sh`, `.github/workflows/release.yml`, `install/tests/*`, `install/TECHNICAL.md`,
`install/FUNCTIONAL.md`, `install/NOTES.md`.

## Changed tests
`install/tests/workflow.rs`: the ancestor-check rule now requires `refs/remotes/origin/main`.
Searched install/tests for `origin/main` and for tests that change a folder during a run:
only `adv_s50c.rs` (the red tests) and `workflow.rs`.

## Out of scope
Closing the instant between a check and its `mv`/`rm` (TECHNICAL 10, not protected).
