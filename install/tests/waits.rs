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
