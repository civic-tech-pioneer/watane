Subsystem: install
Effort: medium
Depends on: s50b-3
Slice: S50b — the install script and its archive (install stages 1 to 3)

## Goal
The two red tests of the adversary pass: an uninstall never removes a folder outside the home, and
the toolchain detection never makes rustup install or run anything before a typed yes.

## Acceptance
- AC1 `install/tests/adv_s50b.rs` `adv_s50b_uninstall_rmdirs_outside_the_home_through_a_link`
  passes: the closing `rmdir` loop of `do_uninstall` removes a folder only when it resolves inside
  the resolved home (8.5 rule 3 / 3.4.1), else leaves it.
- AC2 `adv_s50b_toolchain_installed_without_a_yes_from_a_project_folder` passes: detection (3.7)
  runs from a neutral folder (`cd "$HOME"` in a subshell, or `/`) with `RUSTUP_AUTO_INSTALL=0`
  exported to rustup, and so do the rustup commands of offer B. A new test shows the
  `[toolchain] path = "<dir>"` case: a `rust-toolchain.toml` in the starting folder that names a
  path whose `bin/rustc` records being run; it is never run (works with the fake rustup if the real
  one is absent — the fake need not honour the file; then assert the script's cwd at detection is
  not the starting folder, by having the fake rustup record `pwd`).
- AC3 Nothing else changes; every other test still passes.

## Spec
install/TECHNICAL 3.7 (add: detection and offer B run from `$HOME` with `RUSTUP_AUTO_INSTALL=0`,
and why), 8.4, 8.5. Record in install/NOTES.md.

## Interface
none.

## Files
`install/install.sh`, `install/tests/adv_s50b.rs`, `install/tests/script.rs`,
`install/tests/fakes/*`, `install/tests/support/*`, `install/TECHNICAL.md`, `install/NOTES.md`.

## Changed tests
none (the two red tests are made green, unchanged; adjust them only if a test itself is wrong, and say so).

## Out of scope
Whether `watane-dev doctor` should detect the same way (dev 6.1): a later dev slice. s50b-fix2.
