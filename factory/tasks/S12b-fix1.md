Subsystem: dev
Effort: low
Depends on: none
Slice: S12b — The agent installs a mod into Alice's home (user-sim rejection fix).

## Goal
Two `install` answers point the agent the right way. When the person said no, the agent is told
not to retry without asking them. When no app runs, `next` is the same install command, to run
once the person has started the app (doctor knows nothing about that home).

## Acceptance
- AC1 The red test `api::tests::regress_s12b_declined_fix_does_not_ask_to_correct_and_retry`
  (`dev/src/api.rs`) passes, unchanged. `host.declined` gets the fix exactly
  `The person declined in the app. Ask them before you run install again`; every other `host.*`
  code keeps its fix. TECHNICAL 2.2's `host.declined` row gets that fix text. Extend the test with
  an `assert_eq!` on the new literal.
- AC2 The red test `regress_s12b_no_host_next_repeats_install` (`dev/tests/install.rs`) passes,
  unchanged. On `env.host_not_running`, `install`'s `next` is
  `watane-dev install --json --home <canonical or given home>` (add ` --wait <n>` only when the
  agent passed `--wait`); other `env.*` errors of install (toolchain, allowlist from the test
  pipeline) keep `watane-dev doctor --json`. Extend the test with an `assert_eq!` on the literal.
  TECHNICAL 2.4 **install** states the `next` rule.
- AC3 Nothing else changes. Default `cargo test -p watane-dev`, clippy
  `--all-targets --all-features -D warnings`, `cargo fmt --check` pass.

## Spec
dev `TECHNICAL.md` 2.2, 2.4 **install**, 9.6.

## Interface
none outside dev.

## Files
`dev/src/{api,cli}.rs`, `dev/tests/install.rs`, `dev/TECHNICAL.md`.

## Out of scope
fake-watane's `--help` text, a never-answer mode, decline message wording (friction notes).
