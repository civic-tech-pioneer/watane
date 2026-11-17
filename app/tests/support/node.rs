#![allow(clippy::unwrap_used, reason = "tests may panic")]

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{OnceLock, mpsc};
use std::time::Duration;

pub fn watane_node_bin() -> PathBuf {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .args([
                "build",
                "-p",
                "watane-node",
                "--bin",
                "watane-node",
                "--message-format=json",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "building node: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find_map(|v| {
                (v["reason"] == "compiler-artifact" && v["target"]["name"] == "watane-node")
                    .then(|| v["executable"].as_str().map(PathBuf::from))
                    .flatten()
            })
            .expect("Cargo did not report the workspace watane-node executable")
    })
    .clone()
}

pub struct NodeProc {
    pub child: Child,
    lines: mpsc::Receiver<String>,
    errors: std::thread::JoinHandle<String>,
}
impl NodeProc {
    pub fn spawn(home: &Path, command: &str, args: &[&str]) -> Self {
        assert!(["join", "run"].contains(&command));
        let mut child = Command::new(watane_node_bin())
            .arg("--home")
            .arg(home)
            .args([command, "--bind", "127.0.0.1:0"])
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if tx.send(line.unwrap()).is_err() {
                    break;
                }
            }
        });
        let errors = std::thread::spawn(move || {
            use std::io::Read;
            let mut text = String::new();
            BufReader::new(stderr).read_to_string(&mut text).unwrap();
            text
        });
        Self {
            child,
            lines,
            errors,
        }
    }
    pub fn line(&self) -> String {
        self.lines
            .recv_timeout(Duration::from_secs(30))
            .expect("node stdout line")
    }
    pub fn until(&self, prefix: &str) -> String {
        loop {
            let line = self.line();
            if line.starts_with(prefix) {
                return line;
            }
        }
    }
    pub fn exited(&mut self) -> std::process::ExitStatus {
        let end = std::time::Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(std::time::Instant::now() < end, "node did not exit");
            std::thread::yield_now();
        }
    }
    pub fn stderr(&mut self) -> String {
        let placeholder = std::thread::spawn(String::new);
        std::mem::replace(&mut self.errors, placeholder)
            .join()
            .unwrap()
    }
}
impl Drop for NodeProc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
