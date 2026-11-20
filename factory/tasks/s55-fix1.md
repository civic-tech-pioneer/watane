Subsystem: install
Effort: medium
Depends on: none
Slice: S55 — Alice publishes on one machine, Bob installs on another and uses the mod (J3 between two machines)

## Goal
The foreman's real Tart run of `install/tests/vm_two_machines.sh` found three defects. Bob's mod
window never loaded in the guest, a failed run left its VM running, and a hung step could wait
without a bound. After this task, Bob's test bundle loads mod pages as the release app does, the
cleanup always stops and deletes the run's VM, and every step that talks to the VM has a hard
deadline that fails with a cause.

Found by the foreman (2026-11-20):
- `Bob.app/Contents/Info.plist` has no `NSAppTransportSecurity`, so App Transport Security blocks
  the mod window's `http://<local>.localhost:<port>/` page. The webview never navigates
  (`windows` shows `url: null`), and `eval` there answers "channel is empty and sending half is
  closed". The release bundle carries the exception in `app/Info.plist`. Reproduced on this Mac
  with the bundle launched by `open`. Copying that key made the journey pass.
- vm_lib.sh `cleanup`: the bridge removes its own `bridge-*.pid` between the `[ -f ]` and the
  `cat`. `cat` then fails, `set -e` ends the trap, and `tart stop`/`delete` never run (log:
  `cat: .../bridge-18655.pid: No such file or directory`, VM `watane-run-18168` left running).

## Acceptance
- AC1 The script builds `Bob.app/Contents/Info.plist` with the `NSAppTransportSecurity` value
  taken from `app/Info.plist` at run time (`plutil -extract NSAppTransportSecurity json` then
  `-insert ... -json`), so the two cannot drift. A test asserts the generated plist has that key
  equal to `app/Info.plist`'s.
- AC2 `cleanup` cannot stop part way. It runs with `set +e`; a pid file that vanished or is empty
  is skipped; `tart stop` and `tart delete` of the run's VM and `rm -rf "$work"` always run. A test
  makes the fake bridge remove its pid file at the moment cleanup reads it and checks that
  `tart delete watane-run-<pid>` is still recorded.
- AC3 Hard deadlines, each failing with the step named and its bound (`... timed out after N s`):
  each `remote`/`copy`/`remote_plain` call (300 s, or 900 s for the one-liner install); the
  journey's `cargo test` (1200 s); the whole script (3600 s, checked before each step). They use
  the existing perl `alarm` method, with no new tool. A test shows that a fake `ssh` which never
  returns ends the run by its deadline (shortened by an environment variable read only by
  the test helpers, `WATANE_VM_TEST_SCALE`, which divides every bound; document it in 12.6).
- AC4 The `cargo build`/`--no-run` of the journey run before `tart clone` (check that this is
  already so, and keep it so).
- AC5 install/TECHNICAL.md 12.6 states the ATS key copy (and why: the bundle gets ATS, the bare
  test binary does not), the cleanup rule, and the bounds.

## Spec
install/TECHNICAL.md 12.3, 12.6; app/Info.plist; app/TECHNICAL.md section 12 (the bundle's ATS
exception, near "loads in the bundled app").

## Interface
none (the s55-1 variables and test are unchanged).

## Files
install/tests/vm_lib.sh, install/tests/vm_two_machines.sh, install/tests/vm_mac.sh (only if the
shared deadline needs it), install/tests/it/clean_mac.rs, install/TECHNICAL.md (12.6).

## Changed tests
clean_mac.rs `ac2_ac4_two_machines_lifecycle_failure_and_term_cleanup`: still passes; it may get
the vanished-pid-file mode of AC2. No other assertion changes.

## Out of scope
app code and app tests. The real Tart run (the foreman does it).
