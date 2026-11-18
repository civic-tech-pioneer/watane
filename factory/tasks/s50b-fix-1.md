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
- AC2 Every test of the crate that waits (`support/pty.rs` `run`, `support/mod.rs`
  `Fx::wait_state` and the request wait near line 564) passes on this Mac. Measured by the
  foreman: the Mac runs endpoint security (ManageEngine app control, Microsoft Defender) that
  holds every `exec` while it checks it. Idle, `/bin/sh -c true` takes 0.08 s; while the suite
  runs on 16 test threads it takes 5 s and a fake `uname -m` 25 to 35 s. So one test that passes
  alone in 62 s (`inst_s15_uninstall_home_kept`) fails at four at once ("marker \"keep it: \"
  never came", empty output after 30 s), and a full run failed 10 script.rs tests the same way
  (`inst_s2`, `s11` "the request never came", `s15`, `s16`, `s28`, `s33 ...fresh install`, and
  the `ac2`/`ac3`/`ac4` question tests). Nothing in the script is wrong; the waits are too
  short for this Mac. Fix in the test support only:
  (a) one named constant for every wait, 600 s (a failing test then fails later; nothing passes
  that should not); and
  (b) a process-wide gate in `support/` that lets at most 4 runs of `install.sh` (plain, pty,
  and `package.sh`) be alive at once in one test binary, so the exec queue stays short; a
  test's wait starts when its run starts, not while it waits at the gate. Keep it small (a
  `Mutex<usize>` and `Condvar`, or `std::sync` only; no new dependency).
  Do not change the script for this. Write the cause and the fix in install/NOTES.md.
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
- `install/tests/support/pty.rs` `run` and `install/tests/support/mod.rs` (`Fx::wait_state`, the
  request wait): the 30 s and 20 s waits become one constant of 600 s, and runs pass the gate
  (AC2). No test's assertions change.
- Searched `install/tests/*.rs` for `_CodeSignature`, `Duration::from_secs(30)` and `HAD_RECEIPT`:
  no other test asserts these.

## Out of scope
The clean-Mac run (S50c), the receipt's resolved-path change (S50c), any change outside `install/`.
