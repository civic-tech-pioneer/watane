#![allow(clippy::unwrap_used, reason = "tests may panic")]

use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
const TIMEOUT: Duration = Duration::from_secs(30);
type Replies = Arc<Mutex<HashMap<u64, mpsc::Sender<Value>>>>;
pub struct App {
    pub home: PathBuf,
    pub port: u16,
    child: Child,
    input: Mutex<ChildStdin>,
    replies: Replies,
    next: AtomicU64,
    stderr: std::fs::File,
}
pub struct AskHandle {
    reply: Mutex<mpsc::Receiver<Value>>,
    answer: Mutex<Option<Value>>,
}
impl AskHandle {
    pub fn wait(&self, timeout: Duration) -> Option<Value> {
        let mut answer = self.answer.lock().unwrap();
        if answer.is_none() {
            *answer = self
                .reply
                .lock()
                .unwrap()
                .recv_timeout(timeout)
                .ok()
                .map(result);
        }
        answer.clone()
    }
    pub fn is_open(&self) -> bool {
        self.wait(Duration::ZERO).is_none()
    }
}
pub fn reads_true(raw: &Value) -> bool {
    serde_json::from_str::<Value>(raw.as_str().unwrap_or("")).is_ok_and(|v| v == true)
}
fn result(v: Value) -> Value {
    assert!(v.get("err").is_none(), "driver error: {v}");
    v["ok"].clone()
}
// ioreg reports HIDIdleTime in nanoseconds. Missing or malformed output is
// not evidence that the human stepped away.
pub fn idle_time(output: &str) -> Option<Duration> {
    output.lines().find_map(|line| {
        let (key, value) = line.trim().split_once('=')?;
        (key.trim_start_matches([' ', '\t', '|']).trim_end() == "\"HIDIdleTime\"")
            .then(|| value.trim().parse::<u64>().ok().map(Duration::from_nanos))
            .flatten()
    })
}
pub fn screen_locked(output: &str) -> Option<bool> {
    let (_, sessions) = output.split_once("\"IOConsoleUsers\"")?;
    if sessions.contains("\"CGSSessionScreenIsLocked\"=Yes") {
        Some(true)
    } else if sessions.contains("\"kCGSSessionOnConsoleKey\"=Yes") {
        Some(false)
    } else {
        None
    }
}
fn wait_for_idle() -> bool {
    let mut waiting = false;
    let deadline = Instant::now() + Duration::from_secs(600);
    loop {
        let output = Command::new("ioreg")
            .args(["-n", "Root", "-d1"])
            .output()
            .expect("front tests cannot read screen lock state: failed to run ioreg -n Root -d1");
        assert!(
            output.status.success(),
            "front tests cannot read screen lock state: ioreg failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        if screen_locked(&String::from_utf8_lossy(&output.stdout)).expect(
            "front tests cannot read screen lock state: ioreg output has no console session",
        ) {
            #[allow(
                clippy::explicit_write,
                reason = "Bypass libtest capture so a successful skip stays visible"
            )]
            writeln!(
                std::io::stderr(),
                "front test skipped: the screen is locked (CGSSessionScreenIsLocked)"
            )
            .unwrap();
            return false;
        }
        if !waiting {
            eprintln!("waiting for you to step away");
            waiting = true;
        }
        assert!(
            Instant::now() < deadline,
            "front tests waited 10 minutes for at least 30 seconds of human idle time (ioreg HIDIdleTime)"
        );
        let output = Command::new("ioreg")
            .args(["-c", "IOHIDSystem"])
            .output()
            .expect("front tests cannot read human idle time: failed to run ioreg -c IOHIDSystem");
        assert!(
            output.status.success(),
            "front tests cannot read human idle time: ioreg failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let idle = idle_time(&String::from_utf8_lossy(&output.stdout)).expect(
            "front tests cannot read human idle time: ioreg output has no valid HIDIdleTime in ns",
        );
        if idle >= Duration::from_secs(30) {
            return true;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}
impl App {
    pub fn start(home: &Path, env: &[(&str, &str)]) -> App {
        Self::spawn(home, env, "hidden", 0)
    }
    #[allow(
        dead_code,
        reason = "Shared support: gui_isolation uses only hidden/front"
    )]
    pub fn background(home: &Path, env: &[(&str, &str)]) -> App {
        Self::spawn(home, env, "background", 0)
    }
    pub fn front(home: &Path, env: &[(&str, &str)]) -> Option<App> {
        if !wait_for_idle() {
            return None;
        }
        Some(Self::spawn(home, env, "front", 0))
    }
    pub fn start_port(home: &Path, port: u16) -> App {
        Self::spawn(home, &[], "hidden", port)
    }
    pub fn stderr(&self) -> String {
        use std::os::unix::fs::FileExt;
        let mut bytes = vec![0; self.stderr.metadata().unwrap().len() as usize];
        self.stderr.read_exact_at(&mut bytes, 0).unwrap();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn spawn(home: &Path, env: &[(&str, &str)], screen: &str, port: u16) -> App {
        let mut command = Command::new(env!("CARGO_BIN_EXE_watane-app"));
        command
            .args([
                "--home",
                home.to_str().unwrap(),
                "--port",
                &port.to_string(),
            ])
            // The app never needs the real-GitHub token of the S53 journeys.
            .env_remove("CLAUDE_GH_TOKEN")
            .envs(env.iter().copied())
            .env("WATANE_DRIVER_SCREEN", screen);
        Self::launch(command, home, port)
    }
    pub fn over(command: &str, home_name: &str) -> App {
        let mut shell = Command::new("sh");
        shell.args(["-c", &format!("{command} \"$1\""), "sh", home_name]);
        shell.env_remove("CLAUDE_GH_TOKEN");
        Self::launch(shell, Path::new(""), 0)
    }
    fn launch(mut command: Command, home: &Path, port: u16) -> App {
        let stderr = tempfile::tempfile().unwrap();
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(stderr.try_clone().unwrap())
            .spawn()
            .unwrap();
        let input = Mutex::new(child.stdin.take().unwrap());
        let output = child.stdout.take().unwrap();
        let replies: Replies = Default::default();
        let routes = Arc::clone(&replies);
        let (ready_tx, ready_rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let line = line.unwrap();
                let v: Value = serde_json::from_str(&line).expect("driver stdout must be JSON");
                if let Some(id) = v["id"].as_u64() {
                    if let Some(tx) = routes.lock().unwrap().remove(&id) {
                        let _ = tx.send(v);
                    }
                } else {
                    let _ = ready_tx.send(v);
                }
            }
        });
        // Own the child before waiting, so a failed start also kills it.
        let mut app = App {
            home: home.into(),
            port: 0,
            child,
            input,
            replies,
            next: AtomicU64::new(1),
            stderr,
        };
        let ready = ready_rx
            .recv_timeout(TIMEOUT)
            .unwrap_or_else(|e| panic!("app did not become ready: {e}; {}", app.stderr()));
        assert!(
            ready["event"] == "ready"
                || (port != 0
                    && ready["event"] == "failed"
                    && ready["cause"] != "main page load timed out"),
            "{ready}; {}",
            app.stderr()
        );
        app.port = ready["port"].as_u64().unwrap_or(u64::from(port)) as u16;
        app
    }
    fn send(&self, mut v: Value) -> AskHandle {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        v["id"] = json!(id);
        let (tx, rx) = mpsc::channel();
        self.replies.lock().unwrap().insert(id, tx);
        let mut input = self.input.lock().unwrap();
        writeln!(input, "{v}").unwrap();
        input.flush().unwrap();
        AskHandle {
            reply: Mutex::new(rx),
            answer: Mutex::new(None),
        }
    }
    fn call(&self, v: Value) -> Value {
        self.send(v)
            .wait(TIMEOUT + Duration::from_secs(5))
            .expect("driver response timed out")
    }
    /// The raw eval result: "" when WebKit failed the eval (for example between documents).
    pub fn eval_raw(&self, label: &str, js: &str) -> Value {
        self.call(json!({"op":"eval","label":label,"js":js}))
    }
    /// True only when the eval returned JSON `true`; a failed eval is "not yet", never a panic.
    pub fn is_true(&self, label: &str, js: &str) -> bool {
        reads_true(&self.eval_raw(label, js))
    }
    pub fn eval(&self, label: &str, js: &str) -> Value {
        let v = self.eval_raw(label, js);
        serde_json::from_str(v.as_str().unwrap()).unwrap_or_else(|e| {
            panic!(
                "native eval returned invalid JSON: {e}; {label}: {js}; {v}; {}",
                self.hang_report()
            )
        })
    }
    pub fn wait(&self, label: &str, timeout: Duration) -> bool {
        self.send(json!({"op":"wait","label":label,"timeoutMs":timeout.as_millis() as u64}))
            .wait(timeout + Duration::from_secs(5))
            .expect("driver wait response timed out")
            .as_bool()
            .unwrap()
    }
    #[allow(dead_code, reason = "Shared support: mode probes live in gui_shell")]
    pub fn screen(&self) -> Value {
        self.call(json!({"op":"screen"}))
    }
    #[allow(dead_code, reason = "Shared support: inspected by manager's S39")]
    pub fn menu_items(&self) -> Value {
        self.call(json!({"op":"menu_items"}))
    }
    #[allow(
        dead_code,
        reason = "Shared support: manager checks the actual media query"
    )]
    pub fn theme(&self, theme: Option<&str>) {
        assert_eq!(self.call(json!({"op":"theme", "theme":theme})), true);
    }
    pub fn menu(&self, item: &str) {
        assert_eq!(self.call(json!({"op":"menu", "item":item})), true);
    }
    pub fn windows(&self) -> Vec<Value> {
        self.call(json!({"op":"windows"}))
            .as_array()
            .unwrap()
            .clone()
    }
    pub fn close(&self, label: &str) {
        self.call(json!({"op":"close","label":label}));
    }
    #[allow(
        dead_code,
        reason = "shared by several test binaries, each using only part of it"
    )]
    pub fn pick(&self, paths: &[PathBuf]) {
        assert_eq!(self.call(json!({"op":"pick", "paths":paths})), true);
    }
    #[cfg(all(feature = "driver", unix))]
    pub fn open_url(&self) -> Value {
        self.call(json!({"op":"open_url"}))
    }
    pub fn log(&self) -> Value {
        self.call(json!({"op":"log"}))
    }
    pub fn ask(&self, request: Value) -> AskHandle {
        self.send(json!({"op":"ask","request":request}))
    }
    pub fn quit(mut self) -> std::process::ExitStatus {
        self.send(json!({"op":"quit"}));
        self.wait_exit()
    }
    pub fn close_last(mut self, label: &str) -> std::process::ExitStatus {
        self.send(json!({"op":"close", "label":label}));
        self.wait_exit()
    }
    // What the app was doing: its stderr and, on macOS, a 2 s stack sample.
    fn hang_report(&self) -> String {
        let sample = Command::new("sample")
            .args([&self.child.id().to_string(), "2"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_else(|e| e.to_string());
        format!("stderr: {}\nsample: {sample}", self.stderr())
    }
    pub fn wait_exit(&mut self) -> std::process::ExitStatus {
        let deadline = std::time::Instant::now() + TIMEOUT;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            if std::time::Instant::now() >= deadline {
                panic!("app did not quit; {}", self.hang_report());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::{App, TIMEOUT, reads_true};
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn ac3_s98_is_true_parses_an_eval_result_without_panicking() {
        assert!(reads_true(&json!("true")));
        for raw in [
            json!(""),
            json!("false"),
            json!("null"),
            json!("1"),
            json!("\"true\""),
            json!("{"),
            json!(null),
        ] {
            assert!(!reads_true(&raw), "{raw}");
        }
    }

    #[test]
    fn ac1_over_routes_replies_and_quits_without_a_local_home() {
        // A protocol peer, not a window: reply out of order to prove id routing.
        let command = r#"
            [ "$#" -eq 1 ] && [ "$1" = 'bob name ; $HOME' ] || exit 9
            printf '%s\n' '{"event":"ready","port":1234}'
            read -r first
            read -r second
            printf '%s\n' '{"id":2,"ok":"second"}' '{"id":1,"ok":"first"}'
            read -r quit
            case "$quit" in *'"op":"quit"'*) exit 0;; *) exit 8;; esac
        "#;
        let temp = tempfile::tempdir().unwrap();
        let peer = temp.path().join("peer.sh");
        std::fs::write(&peer, command).unwrap();
        let app = App::over(&format!("sh '{}'", peer.display()), "bob name ; $HOME");
        assert_eq!(app.home, Path::new(""));
        assert_eq!(app.port, 1234);
        let first = app.ask(json!({"request":"first"}));
        let second = app.ask(json!({"request":"second"}));
        assert_eq!(first.wait(TIMEOUT), Some(json!("first")));
        assert_eq!(second.wait(TIMEOUT), Some(json!("second")));
        assert!(app.quit().success());
    }
}
