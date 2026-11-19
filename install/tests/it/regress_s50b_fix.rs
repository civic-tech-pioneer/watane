//! Regression tests from S50b-fix's user-sim run on the Mac (install/TECHNICAL 3.8).

use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::support::*;

/// 3.8: `INT` exits 130 (INST-S-11). On the Mac a Ctrl-C in the first second of a run, while
/// the checks of 3.4 steps 1 to 3 run, is often reported as a cause that is not true: by hand
/// (`kill -INT -<pgid>` at 0.9 s, 1.2 s, 1.8 s, 2.4 s into an upgrade of packed archives) it
/// printed `watane install: the tool id is missing` (exit 2), `watane install: not a version:
/// 2.0.0` (exit 1) and `watane install: SHA256SUMS has more than one line for ...`. macOS sh
/// (bash 3.2) does not always act on an `INT` that killed the child of `$(id -u)` or of
/// `printf | grep -Eq` in `check_version`: it goes on with the failed status, and the script
/// names a missing tool or a bad version (a toy script with the same two lines does it in 15 of
/// 40 runs). A single interrupt at a fixed point does not show it, so this test interrupts runs
/// at delays across the checks and asserts that none of them blames a tool or the version.
#[test]
fn regress_s50b_fix_ctrl_c_during_the_checks_names_no_false_cause() {
    let fx = Fx::new();
    let mut wrong = Vec::new();
    for i in 0..40u64 {
        let child = spawn(&mut fx.cmd(&["--release", "0.1.0"]));
        thread::sleep(Duration::from_millis(i * 15));
        let pid = child.id();
        // the run may have ended already: then there is no group to signal, which is fine
        let _ = Command::new("/bin/sh")
            .args(["-c", &format!("kill -s INT -- -{pid} 2>/dev/null")])
            .status();
        let out = finish(child);
        let blames = out.stderr.contains("is missing")
            || out.stderr.contains("not a version")
            || out.stderr.contains("SHA256SUMS has");
        if blames || !(out.code == 130 || out.code == -1 || out.code == 0) {
            wrong.push((i * 15, out.code, out.stderr.clone()));
        }
    }
    assert!(
        wrong.is_empty(),
        "a Ctrl-C was reported as another failure (delay ms, exit, stderr): {wrong:#?}"
    );
}
