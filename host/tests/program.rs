use reqwest::Client;
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read},
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    process::{Child, ChildStdout, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
use tempfile::TempDir;
use watane_host::{Ready, RunInfo};

mod api;
mod hostile;
mod pages;

struct Preview {
    child: Child,
    stdout: BufReader<ChildStdout>,
    ready: Ready,
}

impl Preview {
    fn start(home: &Path) -> Self {
        Self::start_with(home, &[])
    }

    fn start_with(home: &Path, args: &[&std::ffi::OsStr]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_watane"))
            .arg("--preview")
            .arg(home)
            .args(args)
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
            .recv_timeout(Duration::from_secs(10))
            .unwrap_or_else(|e| {
                let _ = child.kill();
                let _ = child.wait();
                panic!("no ready line: {e}")
            });
        result.unwrap();
        let ready = serde_json::from_str(&line).unwrap_or_else(|e| {
            let _ = child.kill();
            let _ = child.wait();
            panic!("invalid ready line {line:?}: {e}")
        });
        Self {
            child,
            stdout,
            ready,
        }
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
        assert!(
            Command::new("kill")
                .args(["-TERM", &self.child.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert_eq!(status.code(), Some(0));
                break;
            }
            assert!(Instant::now() < deadline, "SIGTERM exceeded 5 seconds");
            std::thread::sleep(Duration::from_millis(10));
        }
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
                "mcp":null,"token":null,"mods":[]
            })
        );
        assert_ne!(preview.ready.port, 0);
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
        for reserved in ["state", "log", "dist", "app", "toolsets.json"] {
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
        vec![],
        vec!["--bad"],
        vec!["--preview"],
        vec!["--home", "/tmp/x"],
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
    assert!(!home.path().join("run.json").exists());
    for ip in ["127.0.0.1", "::1"] {
        assert!(
            std::net::TcpStream::connect(SocketAddr::new(ip.parse().unwrap(), preview.ready.port))
                .is_err()
        );
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
    let deadline = Instant::now() + Duration::from_secs(2);
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
        assert!(serde_json::from_value::<LocalName>(json!(name)).is_err());
    }
    fn serde_type<T: serde::Serialize + serde::de::DeserializeOwned>() {}
    serde_type::<InstalledMod>();
    serde_type::<Ready>();
    serde_type::<watane_host::ReadyMod>();
}
