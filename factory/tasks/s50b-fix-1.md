Subsystem: install
Effort: low
Depends on: none
Slice: S50b-fix — install's tests pass on macOS

## Goal
`cargo test -p watane-install -- --include-ignored` is green on a Mac, three runs in a row, also
when the Mac is loaded (load average 15 to 20 from parallel jobs), and `shellcheck -s sh
install.sh package.sh` reports nothing. The cloud built S50b on Linux, where `codesign` is absent
and `script(1)` is util-linux; on the Mac two things differ.

## Acceptance
- AC1 `tests/package.rs` `ac1_the_archive_has_the_tree_of_4_2` passes on macOS and on Linux.
  Decision (foreman): when `codesign` runs, the tree of 4.2 also holds what `codesign --deep
  --sign -` writes into the bundle: `Applications/watane.app/Contents/_CodeSignature/` with
  `CodeDirectory`, `CodeRequirements`, `CodeResources`, `CodeSignature`. The test expects that
  folder and those four files exactly when `codesign` is on `PATH`, and no `_CodeSignature`
  entry when it is not. install/TECHNICAL 4.2 says so in the tree (one line under
  `watane.app/`) and in the text.
- AC2 `tests/package.rs` `ac4_install_upgrade_uninstall_of_packed_archives` and every other test
  that uses `support/pty.rs` pass on a loaded Mac. Observed: on the Mac the uninstall step once
  failed with `support/pty.rs:62` "marker \"keep it: \" never came" after 30 s; a later run on the
  same Mac at load 17 passed, and every test of the crate takes 60 to 300 s there. Find the cause
  first (read the captured output in the panic: is the prompt missing, or late?). If it is only
  slowness, raise the wait for a marker to 180 s (a failing test then fails later, nothing
  passes that should not). If the prompt is missing on macOS (`/bin/sh` is bash in POSIX mode
  there, `script -q /dev/null` with a piped stdin), fix the harness or the script so it comes.
  Say which in install/NOTES.md.
- AC3 `shellcheck -s sh install/install.sh install/package.sh` exits 0. Fix real findings
  (SC2034 `HAD_RECEIPT` unused: remove it or use it; SC1007 `CDPATH= cd`: write `CDPATH=''`).
  The `~/...` texts that are printed for a person (SC2088, lines near 800, 803, 833) and the
  backquoted command in a printed text (SC2016) are meant literally: silence each with a
  `# shellcheck disable=SCxxxx` line directly above it, with a few words why. SC2018/SC2019
  (`tr 'A-Z' 'a-z'`): use `tr '[:upper:]' '[:lower:]'` only if the behaviour on a `y`/`yes`
  answer stays the same; otherwise disable with a reason.
- AC4 Every other test of the crate passes, on the Mac, with `-- --include-ignored`; and
  `cargo clippy -p watane-install --all-targets -- -D warnings` and `cargo +nightly fmt --check`
  pass.

## Spec
install/TECHNICAL 4.2 (the tree), 12.1 (tests on Linux and macOS, `codesign` skipped when absent),
3.3 (the tools the script uses). Record the decisions in install/NOTES.md and add a row to the
decisions table of install/TECHNICAL 15 (next free IN number) for AC1.

## Interface
none.

## Files
`install/install.sh`, `install/package.sh`, `install/tests/*`, `install/TECHNICAL.md`,
`install/NOTES.md`.

## Changed tests
- `install/tests/package.rs` `ac1_the_archive_has_the_tree_of_4_2`: after, it expects the four
  `_CodeSignature` files and their folder when `codesign` is on `PATH`.
- `install/tests/support/pty.rs` `run` and `install/tests/support/mod.rs` `Fx::wait_state`: the
  30 s waits may change per AC2 (keep the two equal, one named constant).
- Searched `install/tests/*.rs` for `_CodeSignature`, `Duration::from_secs(30)` and `HAD_RECEIPT`:
  no other test asserts these.

## Out of scope
The clean-Mac run (S50c), the receipt's resolved-path change (S50c), any change outside `install/`.
