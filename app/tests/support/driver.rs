use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
const TIMEOUT: Duration = Duration::from_secs(30);
type Replies = Arc<Mutex<HashMap<u64, mpsc::Sender<Value>>>>;
pub struct App {
    pub home: PathBuf,
    pub port: u16,
    child: Child,
    input: Mutex<ChildStdin>,
    replies: Replies,
    next: AtomicU64,
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
fn wait_for_idle() {
    eprintln!("waiting for you to step away");
    let deadline = Instant::now() + Duration::from_secs(600);
    loop {
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
            return;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}
impl App {
    pub fn start(home: &Path, env: &[(&str, &str)]) -> App {
        Self::spawn(home, env, "hidden")
    }
    #[allow(dead_code)] // Shared support: gui_isolation uses only hidden/front.
    pub fn background(home: &Path, env: &[(&str, &str)]) -> App {
        Self::spawn(home, env, "background")
    }
    pub fn front(home: &Path, env: &[(&str, &str)]) -> App {
        wait_for_idle();
        Self::spawn(home, env, "front")
    }
    fn spawn(home: &Path, env: &[(&str, &str)], screen: &str) -> App {
        let mut child = Command::new(env!("CARGO_BIN_EXE_watane-app"))
            .args(["--home", home.to_str().unwrap(), "--port", "0"])
            .env_remove("WATANE_APP_FAKE")
            .envs(env.iter().copied())
            .env("WATANE_DRIVER_SCREEN", screen)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let input = Mutex::new(child.stdin.take().unwrap());
        let output = child.stdout.take().unwrap();
        let replies: Replies = Default::default();
        let routes = replies.clone();
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
        };
        let ready = ready_rx
            .recv_timeout(TIMEOUT)
            .expect("app did not become ready");
        assert_eq!(ready["event"], "ready", "{ready}");
        app.port = ready["port"].as_u64().unwrap() as u16;
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
    pub fn eval(&self, label: &str, js: &str) -> Value {
        let v = self.call(json!({"op":"eval","label":label,"js":js}));
        serde_json::from_str(v.as_str().unwrap()).expect("native eval returned invalid JSON")
    }
    pub fn wait(&self, label: &str, timeout: Duration) -> bool {
        self.send(json!({"op":"wait","label":label,"timeoutMs":timeout.as_millis() as u64}))
            .wait(timeout + Duration::from_secs(5))
            .expect("driver wait response timed out")
            .as_bool()
            .unwrap()
    }
    #[allow(dead_code)] // Shared support: mode probes live in gui_shell.
    pub fn screen(&self) -> Value {
        self.call(json!({"op":"screen"}))
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
    pub fn log(&self) -> Value {
        self.call(json!({"op":"log"}))
    }
    pub fn ask(&self, request: Value) -> AskHandle {
        self.send(json!({"op":"ask","request":request}))
    }
    pub fn quit(mut self) -> std::process::ExitStatus {
        self.send(json!({"op":"quit"}));
        let deadline = std::time::Instant::now() + TIMEOUT;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(std::time::Instant::now() < deadline, "app did not quit");
            std::thread::yield_now();
        }
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
