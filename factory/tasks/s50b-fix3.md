Subsystem: install
Effort: medium
Depends on: s50b-fix2
Slice: S50b — the install script and its archive (install stages 1 to 3)

## Goal
The adversary's second-pass red test passes: an install folder that resolves to the home itself
(a link `~/.local/share/watane-dev -> ~`, `~/.local/bin -> ~`, `~/Applications -> ~`) gives the
removal rule no reach into the home's top level.

Foreman's decision for 8.5 rule 3: the install folder a path is listed under must resolve
**strictly below** the resolved home (not equal to it), and the folder holding the path must
resolve inside that resolved install folder. Install folders that are links to elsewhere inside
the home (a dotfiles setup) keep working. The same "strictly below" holds for the install-folder
check of 3.4.1 at install time (an install into a folder that resolves to the home itself is
exit 2), and for the closing `rmdir` loop of the uninstall.

## Acceptance
- AC1 `install/tests/adv_s50b.rs` `adv_s50b_uninstall_through_install_folder_linked_to_the_home`
  passes unchanged.
- AC2 Tests for `~/.local/bin -> ~` (a receipt line `~/.local/bin/watane-x` does not remove
  `~/watane-x`) and `~/Applications -> ~` (does not remove `~/watane.app`) on uninstall.
- AC3 An install with `~/.local/bin -> ~` (or any of the three folders resolving to the home) is
  refused with exit 2 before any download (extend INST-S-31's cases).
- AC4 An install folder that is a link to another folder strictly inside the home still installs,
  upgrades and uninstalls (existing tests, or one new one if none covers it).
- AC5 Every other test passes.

## Spec
install/TECHNICAL 3.4.1, 8.4, 8.5 rule 3, section 15 (an IN row); install/FUNCTIONAL INST-S-31 if
its oracle gains the case. Record in install/NOTES.md.

## Interface
none.

## Files
`install/install.sh`, `install/tests/*`, `install/TECHNICAL.md`, `install/FUNCTIONAL.md`, `install/NOTES.md`.

## Changed tests
`inst_s31_an_install_folder_outside_the_home`: gains the "resolves to the home" cases.

## Out of scope
Refusing every link among the install folders (would break dotfiles setups).
