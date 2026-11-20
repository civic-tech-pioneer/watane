Subsystem: install
Effort: low
Depends on: none
Slice: S55 — Alice publishes on one machine, Bob installs on another and uses the mod (J3 between two machines)

## Goal
vm_two_machines.sh's failure output tells a maintainer what happened: a missing `--from` folder
is named, an interrupt says it was interrupted, and scp prints no known-hosts warning. The
offline fixture tests leave no fake web host running.

## Acceptance
- AC1 The red test `regress_s55::regress_s55_missing_from_dir_is_named`
  (install/tests/it/regress_s55.rs, commit e4add09) passes: the error line names the folder given
  to `--from`. Check vm_mac.sh for the same `from=$(cd "$from" ...) || fail "cd $from"` pattern and
  fix it there too (a test for it).
- AC2 On HUP, INT or TERM, the script (both drivers, the shared trap in vm_lib.sh) prints
  `<driver>: interrupted` before cleanup, and exits 1 as now. A test sends INT to a run with fake
  tools and checks the line.
- AC3 `copy` (scp) passes `-o LogLevel=ERROR`, as the ssh helpers do; the lifecycle test checks
  the option.
- AC4 The offline `clean_mac::` tests that start the fake `python3 -m http.server` (the
  two-machine modes) leave no process behind: after the test, no process started by it runs. The
  user-sim found one orphaned (`.../two machines' XXXX/bin/python3 -m http.server 0 --bind
  127.0.0.1`, parent 1). Find which mode leaks it and fix it in the fake or the test; a test asserts
  no leftover (e.g. the fake writes its pid, and the test checks it is gone).

## Spec
install/TECHNICAL.md 12.3, 12.6 (state the `interrupted` line in 12.6, and in 12.3 if vm_mac.sh
prints it too).

## Interface
none.

## Files
install/tests/vm_lib.sh, install/tests/vm_two_machines.sh, install/tests/vm_mac.sh,
install/tests/it/clean_mac.rs, install/tests/it/regress_s55.rs (only a non-weakening adjustment),
install/TECHNICAL.md.

## Changed tests
clean_mac.rs lifecycle tests may gain assertions (the scp option, the interrupt line); none lose one.

## Out of scope
app test support (s55-fix2). Product code.
