Subsystem: dev
Effort: low
Depends on: none
Slice: S12b — The agent installs a mod into Alice's home (second user-sim rejection fix).

## Goal
The `next` that `install` gives when no app runs works from any folder: it names the mod folder.

## Acceptance
- AC1 The red test `regress_s12b_no_host_next_keeps_dir` (`dev/tests/install.rs`) passes, unchanged.
  On `env.host_not_running`, `next` is always
  `watane-dev install --json --dir <canonical mod folder> --home <home>` (plus ` --wait <n>` only
  when passed), each path shell-quoted by the existing quoting rule, also when the agent gave no
  `--dir`. Update the literal `assert_eq!` lines of the existing next tests
  (`regress_s12b_no_host_next_repeats_install`, `ac2_no_host_next_preserves_explicit_wait_and_given_home`,
  `ac2_no_host_next_quotes_home_for_the_shell`) to the new form; add one to the red test.
  TECHNICAL 2.4 **install**'s `next` rule says so.
- AC2 Nothing else changes. Default `cargo test -p watane-dev`, clippy
  `--all-targets --all-features -D warnings`, `cargo fmt --check` pass.

## Spec
dev `TECHNICAL.md` 2.4 **install**.

## Interface
none outside dev.

## Files
`dev/src/cli.rs`, `dev/tests/install.rs`, `dev/TECHNICAL.md`.

## Out of scope
`--json` in `next` after a text run (harmless; every `next` of dev uses `--json`).
