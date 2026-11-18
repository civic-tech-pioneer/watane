//! Regression checks for bounded test runs and generous deadlines (TECHNICAL 12.1).
mod support;

#[test]
fn ac2_waits_allow_endpoint_security_delays() {
    assert_eq!(support::WAIT_TIMEOUT.as_secs(), 600);
}

#[test]
fn ac2_gate_holds_the_fifth_run_until_a_run_finishes() {
    let permits: Vec<_> = (0..4).map(|_| support::RunPermit::acquire()).collect();
    let (tx, rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let _permit = support::RunPermit::acquire();
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
    for _ in 0..16 {
        let root = tempfile::tempdir().expect("root");
        let ready = root.path().join("ready");
        let cleanup = root.path().join("cleanup");
        let mut command = std::process::Command::new(support::PERL);
        command
            .args([
                "-MPOSIX",
                "-e",
                "POSIX::setsid(); exec @ARGV",
                "/bin/sh",
                "-c",
            ])
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
        let child = support::spawn(&mut command);
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
