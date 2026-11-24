#![allow(clippy::unwrap_used, reason = "tests may panic")]

use reqwest::Client;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use watane_host::{Ready, RunInfo};

mod api;
mod catalog;
mod connection;
mod events;
mod hostile;
mod pages;

/// True where this machine can bind `::1`; tests reach `::1` only then (S101).
fn has_v6() -> bool {
    std::net::TcpListener::bind("[::1]:0").is_ok()
}

struct Preview {
    child: Child,
    stdout: BufReader<ChildStdout>,
    ready: Ready,
    raw_ready: Value,
}

impl Preview {
    fn start(home: &Path) -> Self {
        Self::start_with(home, &[])
    }

    fn start_with(home: &Path, args: &[&std::ffi::OsStr]) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_watane"));
        command.arg("--preview").arg(home).args(args);
        Self::start_command(command)
    }

    fn home(home: &Path) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_watane"));
        command.arg("--home").arg(home).args(["--port", "0"]);
        Self::start_command(command)
    }

    fn start_command(command: Command) -> Self {
        Self::try_start_command(command).unwrap_or_else(|e| panic!("start failed: {e}"))
    }

    /// Err carries the `error` object of a failure line (`{"watane":1,"error":...}`).
    fn try_start_command(mut command: Command) -> Result<Self, Value> {
        let mut child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let result = stdout.read_line(&mut line);
            let _ = tx.send((stdout, line, result));
        });
        let (stdout, line, result) = rx
            .recv_timeout(Duration::from_secs(60)) // hang guard, not a requirement
            .unwrap_or_else(|e| {
                let _ = child.kill();
                let _ = child.wait();
                panic!("no ready line: {e}")
            });
        result.unwrap();
        let raw_ready: Value = serde_json::from_str(&line).unwrap();
        if let Some(error) = raw_ready.get("error") {
            let _ = child.wait();
            return Err(error.clone());
        }
        let ready = serde_json::from_value(raw_ready.clone()).unwrap_or_else(|e| {
            let _ = child.kill();
            let _ = child.wait();
            panic!("invalid ready line {line:?}: {e}")
        });
        Ok(Self {
            child,
            stdout,
            ready,
            raw_ready,
        })
    }

    fn client(&self, name: &str, ip: &str) -> Client {
        Client::builder()
            .no_proxy()
            .resolve(
                name,
                SocketAddr::new(ip.parse::<IpAddr>().unwrap(), self.ready.port),
            )
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap()
    }

    async fn install(&self, folder: &Path) -> reqwest::Response {
        self.client("localhost", "127.0.0.1")
            .post(format!("{}install", self.ready.api))
            .json(&json!({"folders":[folder]}))
            .send()
            .await
            .unwrap()
    }

    fn stop(&mut self) {
        #[cfg(unix)] // SIGTERM tests require a Unix signal and exit status.
        {
            assert!(
                Command::new("kill")
                    .args(["-TERM", &self.child.id().to_string()])
                    .status()
                    .unwrap()
                    .success()
            );
            let deadline = Instant::now() + Duration::from_secs(30); // hang guard: spec drains up to 5 s
            loop {
                if let Some(status) = self.child.try_wait().unwrap() {
                    assert_eq!(status.code(), Some(0));
                    break;
                }
                assert!(Instant::now() < deadline, "SIGTERM exceeded 30 seconds");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        #[cfg(windows)] // Tests that only need cleanup can wait for an ordinary child kill.
        {
            self.child.kill().unwrap();
            self.child.wait().unwrap()
        };
        let mut rest = String::new();
        self.stdout.read_to_string(&mut rest).unwrap();
        assert_eq!(rest, "", "stdout must contain exactly one line");
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mods")
        .join(name)
}

fn copy_hello() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for file in ["mod.toml", "wit/world.wit", "ui/index.html", "ui/app.js"] {
        fs::create_dir_all(dir.path().join(file).parent().unwrap()).unwrap();
        fs::copy(fixture("hello").join(file), dir.path().join(file)).unwrap();
    }
    dir
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac5_program_installs_counter_and_wires_page_calls() {
    let home = tempfile::tempdir().unwrap();
    let counter = fixture("counter");
    let mut preview =
        Preview::start_with(home.path(), &["--install".as_ref(), counter.as_os_str()]);
    assert_eq!(preview.ready.mods.len(), 1);
    assert_eq!(preview.ready.mods[0].local.to_string(), "counter");
    assert_eq!(
        preview.raw_ready["mods"],
        json!([{"local":"counter","page":format!("http://counter.localhost:{}/", preview.ready.port)}])
    );
    assert!(
        preview
            .ready
            .mcp
            .as_deref()
            .unwrap()
            .ends_with("/mcp/preview")
    );
    assert!(preview.ready.token.as_deref().unwrap().starts_with("wt_"));
    let origin = format!("http://counter.localhost:{}", preview.ready.port);
    assert_eq!(
        preview.ready.mods[0].page.as_deref(),
        Some(format!("{origin}/").as_str())
    );
    let response = preview
        .client("counter.localhost", "127.0.0.1")
        .post(format!("{origin}/call"))
        .header("Origin", &origin)
        .json(&json!({"fn":"api.add","args":null}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.json::<Value>().await.unwrap(), json!({"value":1}));
    preview.stop();
    let preview_home = tempfile::tempdir().unwrap();
    let mut preview = Preview::start(preview_home.path());
    let response = preview.install(&counter).await;
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.json::<Value>().await.unwrap()["installed"][0]["has_logic"],
        true
    );
    preview.stop();
}

#[test]
fn ac1_preview_creates_home_and_reports_ready() {
    for missing in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let home = if missing {
            dir.path().join("new-home")
        } else {
            dir.path().to_owned()
        };
        assert!(RunInfo::read(&home).unwrap().is_none());
        let mut preview = Preview::start(&home);
        let home = home.canonicalize().unwrap();
        assert_eq!(
            serde_json::to_value(&preview.ready).unwrap(),
            json!({
                "watane":1,"mode":"preview","home":home,"port":preview.ready.port,
                "api":format!("http://localhost:{}/api/",preview.ready.port),
                "mcp":format!("http://localhost:{}/mcp/preview",preview.ready.port),"token":preview.ready.token,"mods":[]
            })
        );
        assert_ne!(preview.ready.port, 0);
        assert!(
            preview
                .ready
                .token
                .as_ref()
                .is_some_and(|t| t.len() == 46 && t.starts_with("wt_"))
        );
        assert_eq!(
            fs::read(home.join("home.json")).unwrap(),
            b"{\"watane_home\":1,\"mode\":\"preview\"}"
        );
        assert!(home.join("lock").is_file());
        // The ready line is the synchronization point for readers of run.json.
        let run = RunInfo::read(&home).unwrap().unwrap();
        assert_eq!(
            serde_json::to_value(run).unwrap(),
            json!({
                "watane":1,"pid":preview.child.id(),"port":preview.ready.port,"mode":"preview"
            })
        );
        assert!(home.join("state/state.redb").is_file());
        for reserved in ["log", "dist", "app", "toolsets.json"] {
            assert!(!home.join(reserved).exists());
        }
        preview.stop();
    }
}

#[test]
fn ac6_program_arguments_lock_and_sigterm() {
    let version = Command::new(env!("CARGO_BIN_EXE_watane"))
        .arg("--version")
        .output()
        .unwrap();
    assert_eq!(version.status.code(), Some(0));
    assert_eq!(version.stdout, b"watane 0.1.0\n");
    for args in [
        vec!["--bad"],
        vec!["--preview"],
        vec!["--home", "/tmp/x", "--preview", "/tmp/y"],
        vec!["--home", "/tmp/x", "--install", "/tmp/z"],
        vec!["--preview", "/tmp/x", "--port", "65536"],
        vec!["--version", "--bad"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_watane"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["watane"], 1);
        assert_eq!(value["error"]["code"], "usage");
    }
    let home = tempfile::tempdir().unwrap();
    let mut preview = Preview::start(home.path());
    let second = Command::new(env!("CARGO_BIN_EXE_watane"))
        .arg("--preview")
        .arg(home.path())
        .output()
        .unwrap();
    assert_eq!(second.status.code(), Some(1));
    let error: Value = serde_json::from_slice(&second.stdout).unwrap();
    assert_eq!(error["watane"], 1);
    assert_eq!(error["error"]["code"], "busy");
    preview.stop();
    #[cfg(unix)] // SIGTERM tests assert the Unix process did its graceful cleanup.
    assert!(!home.path().join("run.json").exists());
    for ip in ["127.0.0.1", "::1"] {
        std::net::TcpStream::connect(SocketAddr::new(ip.parse().unwrap(), preview.ready.port))
            .unwrap_err();
    }
    let mut restarted = Preview::start(home.path());
    restarted.stop();
}

#[test]
fn ac6_preview_refuses_participant_or_invalid_home() {
    for (metadata, code) in [
        (r#"{"watane_home":1,"mode":"home"}"#, "mode_mismatch"),
        (r#"{"watane_home":2,"mode":"preview"}"#, "bad_home"),
        ("broken", "bad_home"),
    ] {
        let home = tempfile::tempdir().unwrap();
        fs::write(home.path().join("home.json"), metadata).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_watane"))
            .arg("--preview")
            .arg(home.path())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["error"]["code"], code);
        assert_eq!(
            fs::read_to_string(home.path().join("home.json")).unwrap(),
            metadata
        );
    }
}

#[test]
fn ac6_option_cannot_replace_directory_argument() {
    let home = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_watane"))
        .args(["--preview", "--version"])
        .current_dir(home.path())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30); // hang guard
    while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    if child.try_wait().unwrap().is_none() {
        child.kill().unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], "usage");
    assert!(!home.path().join("--version").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac1_initial_install_and_explicit_port() {
    let home = tempfile::tempdir().unwrap();
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
        .to_string();
    let hello = fixture("hello");
    let mut preview = Preview::start_with(
        home.path(),
        &[
            "--install".as_ref(),
            hello.as_os_str(),
            "--port".as_ref(),
            port.as_ref(),
        ],
    );
    assert_eq!(preview.ready.port.to_string(), port);
    assert_eq!(preview.ready.mods.len(), 1);
    assert_eq!(preview.ready.mods[0].local.to_string(), "hello");
    let url = format!("http://hello.localhost:{}/", preview.ready.port);
    let response = preview
        .client("hello.localhost", "127.0.0.1")
        .get(url)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.bytes().await.unwrap().as_ref(),
        fs::read(fixture("hello").join("ui/index.html")).unwrap()
    );
    preview.stop();
}

#[test]
fn ac7_public_types_validate_and_round_trip() {
    use watane_host::{HostError, InstalledMod, LocalName};
    for name in ["hello", "a", "a1-b2", "abcdefghijklmnopqrstuvwx"] {
        let local = LocalName::parse(name).unwrap();
        assert_eq!(local.as_ref(), name);
        assert_eq!(local.to_string(), name);
        assert_eq!(
            serde_json::from_value::<LocalName>(json!(name)).unwrap(),
            local
        );
    }
    for name in [
        "",
        "-a",
        "a-",
        "a--b",
        "A",
        "a_b",
        "2x",
        "é",
        "abcdefghijklmnopqrstuvwxy",
    ] {
        let error: HostError = LocalName::parse(name).unwrap_err();
        assert_eq!(error.code(), "bad_name");
        assert_eq!(error.status(), 400);
        serde_json::from_value::<LocalName>(json!(name)).unwrap_err();
    }
    fn serde_type<T: serde::Serialize + serde::de::DeserializeOwned>() {}
    serde_type::<InstalledMod>();
    serde_type::<Ready>();
    serde_type::<watane_host::ReadyMod>();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac6_preview_ready_token_is_live_and_never_persisted() {
    let home = tempfile::tempdir().unwrap();
    let counter = fixture("counter");
    let mut preview =
        Preview::start_with(home.path(), &["--install".as_ref(), counter.as_os_str()]);
    let token = preview.ready.token.as_ref().expect("preview token");
    assert_eq!(token.len(), 46);
    assert!(token.starts_with("wt_"));
    assert!(
        token[3..]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    );
    let url = format!("http://localhost:{}/mcp/preview", preview.ready.port);
    assert_eq!(preview.ready.mcp.as_deref(), Some(url.as_str()));
    let client = preview.client("localhost", "127.0.0.1");
    let r = client
        .post(&url)
        .bearer_auth(token)
        .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body = r.json::<Value>().await.unwrap();
    assert_eq!(
        body["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].clone())
            .collect::<Vec<_>>(),
        vec![
            json!("counter_api_add"),
            json!("counter_api_add-then-fail"),
            json!("counter_api_get"),
            json!("counter_api_who")
        ]
    );
    assert_eq!(
        client
            .post(format!("http://localhost:{}/mcp/other", preview.ready.port))
            .bearer_auth(token)
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    fn check(path: &Path, token: &str, locked: &mut Vec<std::path::PathBuf>) {
        for e in fs::read_dir(path).unwrap() {
            let e = e.unwrap();
            let path = e.path();
            if e.file_type().unwrap().is_dir() {
                check(&path, token, locked);
            } else {
                match fs::read(&path) {
                    Ok(bytes) => {
                        assert!(!bytes.windows(token.len()).any(|w| w == token.as_bytes()))
                    }
                    // Windows mandatory file locks can prevent reading live state.
                    #[cfg(windows)]
                    Err(error) if error.raw_os_error() == Some(33) => locked.push(path),
                    Err(error) => panic!("{}: {error}", path.display()),
                }
            }
        }
    }
    let mut locked = Vec::new();
    check(home.path(), token, &mut locked);
    assert!(!locked.contains(&home.path().join("run.json")));
    let token = token.clone();
    preview.stop();
    for path in locked {
        assert!(
            !fs::read(path)
                .unwrap()
                .windows(token.len())
                .any(|w| w == token.as_bytes())
        );
    }
}

#[test]
fn ac1_program_refuses_both_mode_mismatches_without_changing_marker() {
    for preview_first in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let mut first = if preview_first {
            Preview::start(dir.path())
        } else {
            Preview::home(dir.path())
        };
        first.stop();
        let marker = fs::read(dir.path().join("home.json")).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_watane"))
            .arg(if preview_first { "--home" } else { "--preview" })
            .arg(dir.path())
            .args(["--port", "0"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(output.stdout.iter().filter(|&&b| b == b'\n').count(), 1);
        let error: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(error["watane"], 1);
        assert_eq!(error["error"]["code"], "mode_mismatch");
        assert_eq!(fs::read(dir.path().join("home.json")).unwrap(), marker);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac2_home_creates_marker_status_run_and_cleans_up() {
    let dir = tempfile::tempdir().unwrap();
    let mut home = Preview::home(dir.path());
    assert_eq!(
        fs::read(dir.path().join("home.json")).unwrap(),
        br#"{"watane_home":1,"mode":"home"}"#
    );
    assert_eq!(
        home.raw_ready,
        json!({
            "watane":1,"mode":"home","home":dir.path().canonicalize().unwrap(),
            "port":home.ready.port,"api":format!("http://localhost:{}/api/",home.ready.port),
            "mcp":null,"token":null,"mods":[]
        })
    );
    assert_eq!(RunInfo::read(dir.path()).unwrap().unwrap().mode, "home");
    let status = home
        .client("localhost", "127.0.0.1")
        .get(format!("{}status", home.ready.api))
        .send()
        .await
        .unwrap();
    assert_eq!(status.status(), 200);
    assert_eq!(
        status.json::<Value>().await.unwrap(),
        json!({"watane":1,"mode":"home","home":dir.path().canonicalize().unwrap(),"port":home.ready.port,"app":false})
    );
    home.stop();
    #[cfg(unix)] // SIGTERM tests assert the Unix process did its graceful cleanup.
    assert!(!dir.path().join("run.json").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac3_home_refuses_install_without_app() {
    let dir = tempfile::tempdir().unwrap();
    let mut home = Preview::home(dir.path());
    let before = fs::read(dir.path().join("installed.json")).ok();
    let response = home.install(&fixture("hello")).await;
    assert_eq!(response.status(), 403);
    assert_eq!(
        response.json::<Value>().await.unwrap(),
        json!({"error":{"code":"refused","message":"no app is attached to confirm the install"}})
    );
    assert_eq!(fs::read_dir(dir.path().join("mods")).unwrap().count(), 0);
    assert_eq!(fs::read(dir.path().join("installed.json")).ok(), before);
    home.stop();
    let output = Command::new(env!("CARGO_BIN_EXE_watane"))
        .arg("--home")
        .arg(dir.path())
        .args(["--install", "x"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["error"]["code"],
        "usage"
    );
}

#[test]
fn ac4_program_defaults_to_environment_home_and_port() {
    for explicit_home in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let make = || {
            let mut command = Command::new(env!("CARGO_BIN_EXE_watane"));
            command.env("WATANE_HOME", dir.path());
            if explicit_home {
                command.arg("--home").arg(dir.path());
            }
            command
        };
        // Start once on the default port. If another process holds it, the program
        // names `port_in_use`; then the home defaults are checked with port 0.
        let first = make();
        let mut home = match Preview::try_start_command(first) {
            Ok(home) => {
                assert_eq!(home.ready.port, 47341);
                home
            }
            Err(error) => {
                assert_eq!(error["code"], "port_in_use");
                let mut command = make();
                command.args(["--port", "0"]);
                Preview::start_command(command)
            }
        };
        assert_eq!(home.ready.home, dir.path().canonicalize().unwrap());
        assert_eq!(home.ready.mode, "home");
        home.stop();
    }
    for unset in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_watane"));
        command.env("HOME", dir.path()).args(["--port", "0"]);
        if unset {
            command.env_remove("WATANE_HOME");
        } else {
            command.env("WATANE_HOME", "");
        }
        let mut home = Preview::start_command(command);
        assert_eq!(
            home.ready.home,
            dir.path().join(".watane").canonicalize().unwrap()
        );
        home.stop();
    }
}

#[test]
fn ac6_second_home_program_is_busy() {
    let dir = tempfile::tempdir().unwrap();
    let mut home = Preview::home(dir.path());
    // Port 0: the lock must refuse it, never a clash on the default port.
    let output = Command::new(env!("CARGO_BIN_EXE_watane"))
        .arg("--home")
        .arg(dir.path())
        .args(["--port", "0"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["error"]["code"],
        "busy"
    );
    home.stop();
}

#[test]
fn ac7_public_modes_and_default_home() {
    use watane_host::{DEFAULT_PORT, Host, Limits, Mode, default_home};
    assert_eq!(DEFAULT_PORT, 47341);
    let expected = std::env::var_os("WATANE_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|s| !s.is_empty())
                .map(|s| PathBuf::from(s).join(".watane"))
        });
    assert_eq!(default_home().ok(), expected);
    for (mode, preview) in [(Mode::Home { app: None }, false), (Mode::Preview, true)] {
        let dir = tempfile::tempdir().unwrap();
        let host = Host::open(dir.path(), mode, Limits::default()).unwrap();
        assert_eq!(host.is_preview(), preview);
    }
}

/// AC2: where `::1` is absent the program serves 127.0.0.1 alone and says so on stderr,
/// for port 0 and for a fixed port.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac2_preview_serves_v4_alone_without_ipv6() {
    if has_v6() {
        return; // this machine has ::1; AC3 covers it
    }
    let free = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    for requested in [0, free] {
        let home = tempfile::tempdir().unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_watane"));
        command
            .arg("--preview")
            .arg(home.path())
            .args(["--port", &requested.to_string()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        let ready: Value = serde_json::from_str(&line).unwrap();
        let port = ready["port"].as_u64().unwrap();
        if requested != 0 {
            assert_eq!(port, u64::from(requested));
        }
        let response = Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(format!("http://127.0.0.1:{port}/api/status"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let _ = child.kill();
        let output = child.wait_with_output().unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(
            stderr,
            "watane: no IPv6 loopback (::1); serving 127.0.0.1 only\n"
        );
        assert!(ready.get("v6").is_none());
    }
}

/// AC4: `[::1]:<port>` held by another socket stays `port_in_use`, and binds nothing.
#[test]
fn ac4_held_v6_port_is_port_in_use_and_binds_nothing() {
    let Ok(held) = std::net::TcpListener::bind("[::1]:0") else {
        return; // no ::1 on this machine; CI proves the in-use branch
    };
    let port = held.local_addr().unwrap().port();
    let home = tempfile::tempdir().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_watane"));
    command
        .arg("--preview")
        .arg(home.path())
        .args(["--port", &port.to_string()]);
    let error = Preview::try_start_command(command).err().unwrap();
    assert_eq!(error["code"], "port_in_use");
    std::net::TcpListener::bind(("127.0.0.1", port)).unwrap();
}
