Subsystem: install
Effort: low
Depends on: s50b-fix3
Slice: S50b — the install script and its archive (install stages 1 to 3)

## Goal
User-sim's red test passes: when the reader of the script's output goes away (`curl … | sh | head`),
the script still cleans up (3.8) and, inside the swap, undoes it as for HUP/INT/TERM.

## Acceptance
- AC1 `install/tests/regress_s50b.rs` `regress_s50b_closed_stdout_leaves_the_temporary_folder`
  passes unchanged: `PIPE` is trapped with the same handler as `HUP`/`INT`/`TERM` (exit 141), and
  writes from the handler cannot themselves die of SIGPIPE (redirect its own messages to stderr,
  or ignore further PIPE inside it).
- AC2 A reinstall of the same version does not print `If watane is open, quit it and open it
  again.` (3.6 step 5 says it is for an upgrade); a test asserts it is absent on a reinstall and
  present on an upgrade.
- AC3 Every other test passes.

## Spec
install/TECHNICAL 3.8 and 3.9's exit-code table (add 141, `PIPE`), 3.6 step 5. Record in install/NOTES.md.

## Interface
none.

## Files
`install/install.sh`, `install/tests/*`, `install/TECHNICAL.md`, `install/NOTES.md`.

## Changed tests
`inst_s13_same_version_again` if it asserts the quit line (it should then assert its absence).

## Out of scope
dev's fix texts (Q17).
