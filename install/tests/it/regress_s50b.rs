//! Regression tests from S50b's user-sim run on the real programs (install/TECHNICAL 3.8).

use crate::support::*;

/// TECHNICAL 3.8, 3.9 (S50b regress): a closed stdout does not leave the temporary folder.
/// 3.8: the temporary folder is removed whatever ends the run, short of `SIGKILL`. An agent
/// that reads only the first lines (`curl ... | sh | head -20`) closes the pipe; the script's
/// next `printf` gets `SIGPIPE`, which the trap does not catch, so the shell dies without its
/// `EXIT` trap and leaves `$TMPDIR/watane-install.*` with the archive and its extracted tree
/// (about 770 MB with the debug programs of the run, one copy per run).
#[test]
#[ignore = "slow: a whole run whose stdout reader goes away mid-run, which only a whole run shows (TECHNICAL 3.8, 3.9)"]
fn regress_s50b_closed_stdout_leaves_the_temporary_folder() {
    let fx = Fx::new();
    // the fake chmod waits in step 10 (3.6 step 1), once the temporary folder exists
    fx.state_write("chmod_wait", "");
    let mut c = fx.cmd(&[]);
    let mut child = spawn(&mut c);
    fx.wait_state("chmod_waiting");
    // the reader goes away: as `| head -3` after it read its lines
    drop(child.stdout.take());
    fx.unfault("chmod_wait");
    let out = finish(child);
    assert!(
        fx.tmp_tree().is_empty(),
        "the run ended with {} and left in TMPDIR: {:?}",
        out.code,
        fx.tmp_tree()
    );
}
