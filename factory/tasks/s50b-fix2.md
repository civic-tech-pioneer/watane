Subsystem: install
Effort: medium
Depends on: s50b-fix1
Slice: S50b — the install script and its archive (install stages 1 to 3)

## Goal
The adversary's five RISK findings on the Install boundary are closed by the foreman's decisions
below (each the stricter option, cheap to undo), each with a test that fails without the change.

## Acceptance
- AC1 Archive links (3.4 step 9): an archive with any symbolic-link or hard-link member is refused
  with exit 5 before anything is moved into place (check the `tar -tvzf` listing type character,
  or `[ -L ]`/find after extraction in the temp folder — portable between GNU and BSD tar).
  Test: an archive whose `bin/watane` is a link to a file outside the tree; exit 5, the outside file
  unchanged (mode included), nothing installed. Make sure `package.sh` never produces a link member
  (refuse a link among its inputs, test it).
- AC2 Journal (3.6 step 2): the journal, `receipt.new.<pid>` and any file the script creates with
  `>` are created with noclobber (`set -C` around the write) or after a check that no file or link
  exists at that name; a planted symlink `journal.<pid>` for the run's pid makes the run stop
  (exit 5) without writing through it. Test it: a fake `sh` pid is hard to predict, so plant links
  for a range, or test the helper by planting at the name the script reports; choose a reliable way.
- AC3 Home deletion (8.4 step 5): when the home (`~/.watane` or `WATANE_HOME`) resolves outside the
  resolved `$HOME`, the uninstall does not delete it even on a typed `delete`; it prints the
  resolved path and the `rm -rf '<path>'` command for the person to run, and the question is not
  asked. The question, when asked, names the resolved path. Test on the pty (slow).
- AC4 Toolchain question (3.7): when `WATANE_RUSTUP_URL` is set, the state-`none` question names
  that URL instead of `https://sh.rustup.rs`. Test.
- AC5 Removal rule (8.5): rule 2 refuses a `bin/` name ending in `.mine`; rule 3 requires the folder
  that holds the path to resolve inside the resolved install folder it is listed under
  (`.local/bin`, `.local/share/watane`, `.local/share/watane-dev`, `Applications`), not merely inside
  the home. Tests: a receipt line `~/.local/bin/watane.mine` is skipped; with
  `~/.local/share/watane/x -> ~` a receipt line `.../share/watane/x/Documents` does not remove
  `~/Documents`.
- AC6 Every earlier test passes.

## Spec
install/TECHNICAL 3.4 step 9, 3.6, 3.7, 4, 8.4, 8.5, 10; FUNCTIONAL where an oracle changes. Write
each decision into the TECHNICAL section that owns it and add IN-34.. rows to section 15. Record in
install/NOTES.md.

## Interface
none.

## Files
`install/install.sh`, `install/package.sh`, `install/tests/*`, `install/TECHNICAL.md`,
`install/FUNCTIONAL.md`, `install/NOTES.md`.

## Changed tests
`inst_s16_uninstall_home_deleted` and `inst_s28_*` if the question's text now carries the
resolved path: they assert the new text.

## Out of scope
Release signing (Q2).
