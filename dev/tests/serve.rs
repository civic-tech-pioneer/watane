use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
    path::PathBuf,
    process::Command,
    time::Duration,
};
struct Mod {
    temp: tempfile::TempDir,
    dir: PathBuf,
    host: PathBuf,
}
impl Mod {
    fn new(real: bool) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("hello");
        let host = if real {
            std::env::var_os("WATANE_BIN")
                .expect("set WATANE_BIN")
                .into()
        } else {
            temp.path().join("missing")
        };
        let m = Self { temp, dir, host };
        assert_eq!(m.run(&["new", "hello", "--json"]).0, 0);
        m
    }
    fn run(&self, args: &[&str]) -> (i32, Value) {
        let cwd = if self.dir.exists() {
            &self.dir
        } else {
            self.temp.path()
        };
        let o = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .current_dir(cwd)
            .env("WATANE_DEV_CACHE", self.temp.path().join("cache"))
            .env("WATANE_BIN", &self.host)
            .args(args)
            .output()
            .unwrap();
        (
            o.status.code().unwrap(),
            serde_json::from_slice(&o.stdout).unwrap(),
        )
    }
    fn serve(&self) -> Value {
        let (c, v) = self.run(&["serve", "--json"]);
        assert_eq!(c, 0, "{v}");
        v["result"].clone()
    }
}
impl Drop for Mod {
    fn drop(&mut self) {
        if self.dir.exists() {
            let _ = self.run(&["serve", "--stop", "--json"]);
        }
    }
}
fn port(v: &Value) -> u16 {
    v["api"]
        .as_str()
        .unwrap()
        .split(':')
        .nth(2)
        .unwrap()
        .split('/')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}
fn get(v: &Value, path: &str) -> (String, Vec<u8>) {
    let mut s = TcpStream::connect(("127.0.0.1", port(v))).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    write!(
        s,
        "GET /{path} HTTP/1.1\r\nHost: hello.localhost:{}\r\nConnection: close\r\n\r\n",
        port(v)
    )
    .unwrap();
    let mut bytes = Vec::new();
    s.read_to_end(&mut bytes).unwrap();
    let split = bytes.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    (
        String::from_utf8(bytes[..split].to_vec()).unwrap(),
        bytes[split + 4..].to_vec(),
    )
}
#[test]
#[ignore = "needs WATANE_BIN"]
fn ac4_real_preview_serves_page_and_csp() {
    let m = Mod::new(true);
    let v = m.serve();
    assert_eq!(v["reused"], false);
    assert!(v["mcp"].is_null());
    let hash = blake3::hash(m.dir.canonicalize().unwrap().to_str().unwrap().as_bytes()).to_hex();
    let folder = m
        .temp
        .path()
        .canonicalize()
        .unwrap()
        .join(format!("cache/previews/hello-{}/", &hash[..8]));
    assert_eq!(v["home"], folder.join("home").to_str().unwrap());
    assert_eq!(v["log"], folder.join("watane.log").to_str().unwrap());
    assert_eq!(
        v["pages"][0]["url"],
        format!("http://hello.localhost:{}/", port(&v))
    );
    let (headers, body) = get(&v, "");
    assert!(headers.starts_with("HTTP/1.1 200"));
    assert_eq!(body, fs::read(m.dir.join("ui/index.html")).unwrap());
    let csp = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; media-src 'self' blob:; connect-src 'self'; worker-src 'self'; manifest-src 'self'; form-action 'self'; base-uri 'none'; frame-src 'none'; frame-ancestors 'none'; object-src 'none'";
    assert!(
        headers
            .to_lowercase()
            .contains(&format!("content-security-policy: {csp}"))
    );
    let html = String::from_utf8(body).unwrap();
    assert!(html.contains("<script src=\"app.js\"></script>"));
    assert_eq!(html.matches("<script").count(), 1);
    assert_eq!(
        get(&v, "app.js").1,
        fs::read(m.dir.join("ui/app.js")).unwrap()
    );
}
#[test]
#[ignore = "needs WATANE_BIN"]
fn ac5_serve_reuses_preview_and_updates_page() {
    let m = Mod::new(true);
    let first = m.serve();
    fs::write(
        m.dir.join("ui/index.html"),
        b"<!doctype html><h1>Edited</h1>",
    )
    .unwrap();
    let second = m.serve();
    assert_eq!(second["reused"], true);
    assert_eq!(port(&first), port(&second));
    assert_eq!(get(&second, "").1, b"<!doctype html><h1>Edited</h1>");
}
#[test]
#[ignore = "needs WATANE_BIN"]
fn ac6_stop_closes_port_and_removes_preview() {
    let m = Mod::new(true);
    let v = m.serve();
    let (c, stopped) = m.run(&["serve", "--stop", "--json"]);
    assert_eq!(c, 0);
    assert_eq!(
        stopped["result"],
        serde_json::json!({"stopped":true,"home":v["home"]})
    );
    assert!(TcpStream::connect(("127.0.0.1", port(&v))).is_err());
    assert!(
        !PathBuf::from(v["home"].as_str().unwrap())
            .parent()
            .unwrap()
            .exists()
    );
    let (c, v) = m.run(&["serve", "--stop", "--json"]);
    assert_eq!(c, 0);
    assert_eq!(v["result"]["stopped"], false);
}
#[test]
fn ac7_missing_host_has_environment_error_and_fix() {
    let m = Mod::new(false);
    let (c, v) = m.run(&["serve", "--json"]);
    assert_eq!(c, 3);
    assert_eq!(v["errors"][0]["code"], "env.watane_missing");
    assert_eq!(
        v["errors"][0]["fix"],
        "Install watane, or set WATANE_BIN to the program"
    );
}
