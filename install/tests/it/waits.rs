//! Regression checks for bounded test runs and generous deadlines (TECHNICAL 12.1).
use crate::support;

/// TECHNICAL 12.1: `script(1)` keeps the last typed answer until the tty reader consumes it.
#[test]
#[ignore = "slow: proves script(1) preserves the final answer until the tty read"]
fn ac1_pty_preserves_the_last_answer_until_consumed() {
    let fx = support::Fx::new();
    std::fs::write(
        &fx.script,
        "printf 'answer: '\n/bin/sleep 1\nIFS= read -r answer </dev/tty || exit 11\nprintf 'consumed: <%s>\\n' \"$answer\"\n",
    )
    .expect("delayed tty reader");
    let out = fx.run_pty(&[], &[], &[("answer: ", "delete")]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.has_line("consumed: <delete>"), "{out:?}");
}

#[test]
fn ac2_waits_allow_endpoint_security_delays() {
    assert_eq!(support::WAIT_TIMEOUT.as_secs(), 600);
}

/// TECHNICAL 12.1: the gate holds the fifth run until one ends. It is tested on a gate of its
/// own, so that it neither waits for the runs of other tests nor holds them back.
#[test]
fn ac2_gate_holds_the_fifth_run_until_a_run_finishes() {
    static GATE: support::Gate = support::Gate::new();
    let permits: Vec<_> = (0..4).map(|_| GATE.acquire()).collect();
    let (tx, rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let _permit = GATE.acquire();
        tx.send(()).expect("entered");
    });
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(100))
            .is_err()
    );
    drop(permits);
    rx.recv_timeout(support::WAIT_TIMEOUT).expect("released");
    worker.join().expect("worker");
}

#[test]
fn ac3_shellcheck_reports_no_findings_when_available() {
    let result = std::process::Command::new("shellcheck")
        .args(["-s", "sh", "install.sh", "package.sh"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output();
    let output = match result {
        Ok(output) => output,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => panic!("shellcheck: {e}"),
    };
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

/// A failed child must not let its parent run cleanup during a simulated power loss.
#[test]
fn ac2_group_kill_leaves_no_window_for_parent_cleanup() {
    for _ in 0..8 {
        let root = tempfile::tempdir().expect("root");
        let ready = root.path().join("ready");
        let cleanup = root.path().join("cleanup");
        let mut command = std::process::Command::new(support::PERL);
        command
            .args(support::LAUNCH)
            .args(["/bin/sh", "-c"])
            .arg(
                r#"
trap 'echo cleanup > "$2"' EXIT
# More group members widen the interval between child and parent delivery.
for n in 1 2 3 4 5 6 7 8; do /bin/sleep 600 & done
/bin/sh -c 'echo ready > "$1"; exec /bin/sleep 600' sh "$1"
exit 1
"#,
            )
            .arg("sh")
            .arg(&ready)
            .arg(&cleanup);
        let child = support::spawn_free(&mut command);
        let start = std::time::Instant::now();
        while !ready.exists() {
            assert!(start.elapsed() < support::WAIT_TIMEOUT, "child never ready");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        support::signal_group(child.id(), "KILL");
        let out = support::finish(child);
        assert_eq!(out.code, -1, "{out:?}");
        assert!(!cleanup.exists(), "parent ran cleanup during group KILL");
    }
}

/// A test run from a background shell inherits an ignored INT; the launcher restores it.
#[test]
fn ac2_the_launcher_restores_an_ignored_int() {
    let mut command = std::process::Command::new(support::PERL);
    command
        .args(["-e", "$SIG{INT} = 'IGNORE'; exec @ARGV", support::PERL])
        .args(support::LAUNCH)
        .args(["/bin/sh", "-c", "trap 'exit 7' INT; kill -s INT $$; exit 0"]);
    let out = support::finish(support::spawn_free(&mut command));
    assert_eq!(out.code, 7, "{out:?}");
}

/// A process left behind that holds the pipes cannot make a test wait forever.
#[test]
fn ac2_a_run_past_its_deadline_is_killed_with_its_group() {
    let mut command = std::process::Command::new(support::PERL);
    command
        .args(support::LAUNCH)
        .args(["/bin/sh", "-c", "/bin/sleep 600 & exit 0"]);
    let start = std::time::Instant::now();
    let out = support::finish_within(
        support::spawn_free(&mut command),
        std::time::Duration::from_secs(2),
    );
    assert!(out.is_err(), "{out:?}");
    assert!(start.elapsed() < std::time::Duration::from_secs(300));
}
