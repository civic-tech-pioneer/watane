//! Test support for `script.rs`: the fake release, the fake tools and running `install.sh` (TECHNICAL 12.1).
#![allow(dead_code, reason = "s50b-3 and the other test files use the rest")]

pub mod pty;

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub const TARGET: &str = "aarch64-apple-darwin";
pub const PERL: &str = "/usr/bin/perl";

/// Endpoint security can queue each exec for tens of seconds on a loaded Mac.
pub const WAIT_TIMEOUT: Duration = Duration::from_secs(600);
static RUNS: (Mutex<usize>, Condvar) = (Mutex::new(0), Condvar::new());

pub struct RunPermit;

impl RunPermit {
    pub fn acquire() -> Self {
        let (count, ready) = &RUNS;
        let mut count = ready
            .wait_while(count.lock().expect("runs"), |n| *n >= 4)
            .expect("runs");
        *count += 1;
        Self
    }
}

impl Drop for RunPermit {
    fn drop(&mut self) {
        *RUNS.0.lock().expect("runs") -= 1;
        RUNS.1.notify_one();
    }
}

pub struct RunChild {
    child: Child,
    _permit: RunPermit,
}

impl std::ops::Deref for RunChild {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.child
    }
}

impl std::ops::DerefMut for RunChild {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.child
    }
}

/// Acquires before spawning, so callers start their deadlines after the gate.
pub fn spawn_run(c: &mut Command) -> RunChild {
    let permit = RunPermit::acquire();
    RunChild {
        child: c.spawn().expect("spawn"),
        _permit: permit,
    }
}

/// What a run printed and how it ended.
#[derive(Debug)]
pub struct Out {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Out {
    pub fn out_lines(&self) -> Vec<&str> {
        self.stdout.lines().collect()
    }
    pub fn first_err(&self) -> &str {
        self.stderr.lines().next().unwrap_or("")
    }
    /// Asserts that stdout holds the lines, in this order (other lines may lie between).
    pub fn assert_in_order(&self, wanted: &[&str]) {
        let mut at = 0;
        let lines = self.out_lines();
        for w in wanted {
            match lines[at..].iter().position(|l| l == w) {
                Some(i) => at += i + 1,
                None => panic!("line {w:?} not found in order in stdout:\n{}", self.stdout),
            }
        }
    }
    pub fn has_line(&self, line: &str) -> bool {
        self.stdout.lines().any(|l| l == line)
    }
}

/// The real path of a stock tool.
pub fn which(name: &str) -> PathBuf {
    for d in ["/usr/bin", "/bin", "/usr/local/bin", "/usr/sbin", "/sbin"] {
        let p = Path::new(d).join(name);
        if p.exists() {
            return p;
        }
    }
    panic!("the tool {name} is not on this machine");
}

/// One test's world: a temporary `HOME` and `TMPDIR`, the fake tools, the fake release.
pub struct Fx {
    pub root: tempfile::TempDir,
    pub home: PathBuf,
    pub tmp: PathBuf,
    pub state: PathBuf,
    pub fakes: PathBuf,
    pub tools: PathBuf,
    pub real: PathBuf,
    pub script: PathBuf,
    pub rel: Release,
}

const TOOLS: [&str; 18] = [
    "sh", "curl", "tar", "mktemp", "mkdir", "cp", "rmdir", "ls", "cat", "chmod", "grep", "sed",
    "awk", "tr", "head", "dirname", "readlink",
    // GNU tar runs gzip; macOS tar has it built in, so the script does not name it
    "gzip",
];

impl Fx {
    /// A fresh world with the release `0.1.0` served and the baked script `0.1.0`.
    pub fn new() -> Fx {
        let root = tempfile::tempdir().expect("tempdir");
        let r = root.path().to_path_buf();
        let [home, tmp, state, fakes, tools, real] =
            ["home", "tmp", "state", "fakes", "tools", "real"].map(|n| {
                let p = r.join(n);
                fs::create_dir(&p).expect("mkdir");
                p
            });
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fakes");
        for e in fs::read_dir(&src).expect("fakes dir") {
            let e = e.expect("entry");
            if matches!(e.file_name().to_str(), Some("rustup" | "rustc")) {
                continue; // only a scenario that has them puts them on the PATH (add_rustup)
            }
            let to = fakes.join(e.file_name());
            fs::copy(e.path(), &to).expect("copy fake");
            fs::set_permissions(&to, fs::Permissions::from_mode(0o755)).expect("chmod");
        }
        for t in TOOLS {
            symlink(which(t), tools.join(t)).expect("link tool");
        }
        let sum = if which_opt("shasum").is_some() {
            "shasum"
        } else {
            "sha256sum"
        };
        symlink(which(sum), tools.join(sum)).expect("link sum");
        if let Some(k) = which_opt("kill") {
            symlink(k, tools.join("kill")).expect("link kill");
        }
        for t in ["mv", "rm", "sleep", "mkdir", "chmod"] {
            symlink(which(t), real.join(t)).expect("link real");
        }
        let script = r.join("install.sh");
        let rel = Release::start();
        let fx = Fx {
            root,
            home,
            tmp,
            state,
            fakes,
            tools,
            real,
            script,
            rel,
        };
        fx.bake_script("0.1.0");
        fx.rel.set_version("0.1.0", &["serde-1.0.0"]);
        fx
    }

    /// The script under test: a copy of the checkout's `install.sh` with `@RELEASE@` replaced.
    pub fn bake_script(&self, version: &str) {
        fs::write(&self.script, baked_script(version)).expect("write script");
    }

    /// The unbaked checkout copy.
    pub fn use_checkout_script(&self) {
        fs::copy(checkout_script(), &self.script).expect("copy script");
    }

    /// Waits until the fake `mv` or `rm` wrote that it sleeps (the file of `state`), then returns it.
    pub fn wait_state(&self, name: &str) -> String {
        let t = Instant::now();
        loop {
            let s = self.state_read(name);
            if !s.is_empty() {
                return s;
            }
            assert!(t.elapsed() < WAIT_TIMEOUT, "{name} never appeared");
            thread::sleep(Duration::from_millis(20));
        }
    }

    pub fn state_write(&self, name: &str, content: &str) {
        fs::write(self.state.join(name), content).expect("state write");
    }
    pub fn state_read(&self, name: &str) -> String {
        fs::read_to_string(self.state.join(name)).unwrap_or_default()
    }

    /// Takes a tool off the `PATH` of the script.
    pub fn remove_tool(&self, name: &str) {
        fs::remove_file(self.tools.join(name)).expect("remove tool");
    }

    /// Puts the fake `rustup` and `rustc` on the `PATH`, with a sysroot that has the target or not.
    pub fn add_rustup(&self, with_target: bool) {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fakes");
        for n in ["rustup", "rustc"] {
            fs::copy(src.join(n), self.fakes.join(n)).expect("copy fake");
        }
        let sysroot = self.root.path().join("sysroot");
        fs::create_dir_all(sysroot.join("lib")).expect("sysroot");
        if with_target {
            fs::create_dir_all(sysroot.join("lib/rustlib/wasm32-wasip2/lib")).expect("target");
        }
        self.state_write("rustup_sysroot", &format!("{}\n", sysroot.display()));
    }

    /// A rustup installer served as `file://`, which writes a fake rustup under `$HOME/.cargo`.
    pub fn rustup_installer(&self) -> String {
        let p = self.root.path().join("rustup-init.sh");
        let body = r#"#!/bin/sh
echo "$*" >> "$FAKE_STATE/installer.log"
mkdir -p "$HOME/.cargo/bin" "$HOME/.rustup/toolchains/stable/lib/rustlib/wasm32-wasip2/lib"
cat > "$HOME/.cargo/bin/rustup" <<'INNER'
#!/bin/sh
echo "$*" >> "$FAKE_STATE/rustup.log"
[ "${1:-}" = which ] && echo "${0%/*}/rustc"
exit 0
INNER
cat > "$HOME/.cargo/bin/rustc" <<'INNER'
#!/bin/sh
[ "${1:-}" = "--print" ] && echo "$HOME/.rustup/toolchains/stable"
exit 0
INNER
chmod 755 "$HOME/.cargo/bin/rustup" "$HOME/.cargo/bin/rustc"
"#;
        fs::write(&p, body).expect("write installer");
        format!("file://{}", p.display())
    }

    pub fn path_value(&self) -> String {
        format!("{}:{}", self.fakes.display(), self.tools.display())
    }

    /// The environment of a run, cleared first (TECHNICAL 12.1).
    pub fn env_pairs(&self) -> Vec<(String, String)> {
        let s = |p: &Path| p.display().to_string();
        let mut v = vec![
            ("HOME".into(), s(&self.home)),
            ("TMPDIR".into(), s(&self.tmp)),
            ("PATH".into(), self.path_value()),
            ("FAKE_STATE".into(), s(&self.state)),
            ("FAKE_REAL".into(), s(&self.real)),
            ("WATANE_RELEASE_BASE".into(), self.rel.base()),
        ];
        v.sort();
        v
    }

    /// `sh install.sh <args>` with no controlling terminal (a new session), stdin a pipe.
    pub fn cmd(&self, args: &[&str]) -> Command {
        let mut c = Command::new(PERL);
        c.args(["-MPOSIX", "-e", "POSIX::setsid(); exec @ARGV", "/bin/sh"]);
        c.arg(&self.script).args(args);
        c.env_clear().envs(self.env_pairs());
        c
    }

    pub fn run(&self, args: &[&str]) -> Out {
        exec(self.cmd(args))
    }
    pub fn run_env(&self, args: &[&str], env: &[(&str, &str)]) -> Out {
        let mut c = self.cmd(args);
        for (k, v) in env {
            c.env(k, v);
        }
        exec(c)
    }

    /// Lists a folder: relative paths, with `/` after a folder and ` -> target` after a link.
    pub fn tree(&self, p: &Path) -> Vec<String> {
        snapshot(p).into_iter().map(|(n, _)| n).collect()
    }
    pub fn home_tree(&self) -> Vec<String> {
        self.tree(&self.home)
    }
    pub fn tmp_tree(&self) -> Vec<String> {
        self.tree(&self.tmp)
    }

    pub fn bin(&self, name: &str) -> PathBuf {
        self.home.join(".local/bin").join(name)
    }
    pub fn share(&self, name: &str) -> PathBuf {
        self.home.join(".local/share").join(name)
    }
    pub fn app(&self) -> PathBuf {
        self.home.join("Applications/watane.app")
    }
    pub fn receipt(&self) -> PathBuf {
        self.share("watane/receipt")
    }
    pub fn read(&self, p: &Path) -> String {
        fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    }

    /// All the files of INST-S-1 exist, and the programs run.
    pub fn assert_installed(&self, version: &str) {
        for n in ["watane", "watane-dev", "watane-node"] {
            let p = self.bin(n);
            let mode = fs::metadata(&p)
                .unwrap_or_else(|_| panic!("{} missing", p.display()))
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111, "{n} is executable");
            let out = Command::new(&p)
                .arg("--version")
                .output()
                .expect("run program");
            assert_eq!(
                String::from_utf8_lossy(&out.stdout).trim(),
                format!("{n} {version}")
            );
        }
        assert!(self.share("watane-dev/vendor").is_dir());
        assert_eq!(self.read(&self.share("watane/VERSION")).trim(), version);
        assert!(self.share("watane/install.sh").is_file());
        assert!(self.app().join("Contents/Info.plist").is_file());
        assert!(self.receipt().is_file());
        assert!(
            self.read(&self.receipt())
                .contains(&format!("\nversion {version}\n"))
        );
    }

    /// Runs the script on a pseudo-terminal, typing the answers (see `pty`).
    pub fn run_pty(&self, args: &[&str], env: &[(&str, &str)], steps: &[(&str, &str)]) -> Out {
        pty::run(self, args, env, steps)
    }
}

fn which_opt(name: &str) -> Option<PathBuf> {
    ["/usr/bin", "/bin", "/usr/local/bin"]
        .iter()
        .map(|d| Path::new(d).join(name))
        .find(|p| p.exists())
}

pub fn checkout_script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("install.sh")
}

/// `install.sh` with `@RELEASE@` replaced, as `package.sh` does.
pub fn baked_script(version: &str) -> String {
    let s = fs::read_to_string(checkout_script()).expect("read install.sh");
    let from = "DEFAULT_RELEASE=\"@RELEASE@\"";
    assert!(s.contains(from));
    s.replacen(from, &format!("DEFAULT_RELEASE=\"{version}\""), 1)
}

/// Starts a command with a closed stdin pipe and waits for it.
pub fn exec(mut c: Command) -> Out {
    let child = spawn(&mut c);
    finish(child)
}

pub fn spawn(c: &mut Command) -> RunChild {
    c.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = spawn_run(c);
    drop(child.stdin.take());
    child
}

pub fn finish(run: RunChild) -> Out {
    let RunChild { child, _permit } = run;
    let o = child.wait_with_output().expect("wait");
    Out {
        code: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

/// Sends a signal to the process group of a child that started with `setsid`.
pub fn signal_group(pid: u32, sig: &str) {
    // Group delivery is not atomic: a killed child can wake its parent before
    // the parent's KILL arrives. Freeze the leader first so it cannot roll back.
    let command = if sig == "KILL" {
        format!("kill -s STOP {pid} && kill -s KILL -- -{pid}")
    } else {
        format!("kill -s {sig} -- -{pid}")
    };
    let st = Command::new("/bin/sh")
        .args(["-c", &command])
        .status()
        .expect("kill");
    assert!(st.success(), "kill {sig} -{pid}");
}

/// Every path under `root` with its content: files by bytes, links by target.
pub fn snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let mut es: Vec<_> = fs::read_dir(dir)
            .expect("read_dir")
            .map(|e| e.expect("e"))
            .collect();
        es.sort_by_key(|e| e.file_name());
        for e in es {
            let p = e.path();
            let rel = p.strip_prefix(base).expect("prefix").display().to_string();
            let md = fs::symlink_metadata(&p).expect("md");
            if md.file_type().is_symlink() {
                let t = fs::read_link(&p).expect("link");
                out.push((format!("{rel} -> {}", t.display()), Vec::new()));
            } else if md.is_dir() {
                out.push((format!("{rel}/"), Vec::new()));
                walk(base, &p, out);
            } else {
                out.push((rel, fs::read(&p).expect("read")));
            }
        }
    }
    let mut v = Vec::new();
    walk(root, root, &mut v);
    v
}

/// A fake archive of TECHNICAL 4.2: programs are shell scripts that print `<name> <version>`.
pub fn build_archive(version: &str, vendor: &[&str]) -> Vec<u8> {
    build_archive_with(version, vendor, &|_| {})
}

/// `build_archive`, with `tweak` run on the top folder before it is packed.
pub fn build_archive_with(version: &str, vendor: &[&str], tweak: &dyn Fn(&Path)) -> Vec<u8> {
    let d = tempfile::tempdir().expect("tempdir");
    let top = format!("watane-{version}-{TARGET}");
    let t = d.path().join(&top);
    let w = |rel: &str, body: &str, mode: u32| {
        let p = t.join(rel);
        fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
        fs::write(&p, body).expect("write");
        fs::set_permissions(&p, fs::Permissions::from_mode(mode)).expect("chmod");
    };
    w("VERSION", &format!("{version}\n"), 0o644);
    for n in ["watane", "watane-dev", "watane-node"] {
        w(
            &format!("bin/{n}"),
            &format!("#!/bin/sh\necho \"{n} {version}\"\n"),
            0o755,
        );
    }
    for c in vendor {
        w(
            &format!("share/watane-dev/vendor/{c}/Cargo.toml"),
            "[package]\n",
            0o644,
        );
    }
    w("share/watane/install.sh", &baked_script(version), 0o755);
    w("share/watane/VERSION", &format!("{version}\n"), 0o644);
    w(
        "Applications/watane.app/Contents/Info.plist",
        "<plist/>\n",
        0o644,
    );
    w(
        "Applications/watane.app/Contents/MacOS/watane",
        "#!/bin/sh\n",
        0o755,
    );
    tweak(&t);
    let o = Command::new(which("tar"))
        .args(["-czf", "-", "-C"])
        .arg(d.path())
        .arg(&top)
        .env("COPYFILE_DISABLE", "1")
        .output()
        .expect("tar");
    assert!(o.status.success());
    o.stdout
}

pub fn sha256(bytes: &[u8]) -> String {
    let d = tempfile::tempdir().expect("tempdir");
    let p = d.path().join("f");
    fs::write(&p, bytes).expect("write");
    let o = Command::new(which("sha256sum"))
        .arg(&p)
        .output()
        .expect("sha256sum");
    String::from_utf8_lossy(&o.stdout)
        .split_whitespace()
        .next()
        .expect("sum")
        .to_string()
}

/// The fake release server: a `tiny_http` thread that logs each request path.
pub struct Release {
    port: u16,
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    log: Arc<Mutex<Vec<String>>>,
    stall: Arc<Mutex<Option<String>>>,
    stalled: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for Release {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Release(port {})", self.port)
    }
}

impl Release {
    pub fn start() -> Release {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("bind");
        let port = server.server_addr().to_ip().expect("ip").port();
        let files: Arc<Mutex<HashMap<String, Vec<u8>>>> = Arc::default();
        let log: Arc<Mutex<Vec<String>>> = Arc::default();
        let stall: Arc<Mutex<Option<String>>> = Arc::default();
        let stalled = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let (f2, l2, s2, st2, sp2) = (
            Arc::clone(&files),
            Arc::clone(&log),
            Arc::clone(&stall),
            Arc::clone(&stalled),
            Arc::clone(&stop),
        );
        let handle = thread::spawn(move || {
            let mut held = Vec::new();
            while !sp2.load(Ordering::SeqCst) {
                let Ok(Some(req)) = server.recv_timeout(Duration::from_millis(50)) else {
                    continue;
                };
                let url = req.url().to_string();
                l2.lock().expect("log").push(url.clone());
                if s2.lock().expect("stall").as_deref() == Some(url.as_str()) {
                    st2.store(true, Ordering::SeqCst);
                    held.push(req);
                    continue;
                }
                let body = f2.lock().expect("files").get(&url).cloned();
                let _ = match body {
                    Some(b) => req.respond(tiny_http::Response::from_data(b)),
                    None => req.respond(
                        tiny_http::Response::from_string("not found").with_status_code(404),
                    ),
                };
            }
        });
        Release {
            port,
            files,
            log,
            stall,
            stalled,
            stop,
            handle: Some(handle),
        }
    }

    pub fn base(&self) -> String {
        format!("http://127.0.0.1:{}/release", self.port)
    }
    pub fn put(&self, name: &str, bytes: Vec<u8>) {
        self.files
            .lock()
            .expect("files")
            .insert(format!("/release/{name}"), bytes);
    }
    pub fn remove(&self, name: &str) {
        self.files
            .lock()
            .expect("files")
            .remove(&format!("/release/{name}"));
    }
    /// Serves the archive and `SHA256SUMS` of this version, as one release holds them.
    pub fn set_version(&self, version: &str, vendor: &[&str]) {
        let bytes = build_archive(version, vendor);
        let name = archive_name(version);
        self.put(
            "SHA256SUMS",
            format!("{}  {name}\n", sha256(&bytes)).into_bytes(),
        );
        self.put(&name, bytes);
    }
    pub fn requests(&self) -> Vec<String> {
        self.log.lock().expect("log").clone()
    }
    /// The next request for this file is accepted and never answered.
    pub fn stall_on(&self, name: &str) {
        *self.stall.lock().expect("stall") = Some(format!("/release/{name}"));
    }
    pub fn wait_stalled(&self) {
        let t = Instant::now();
        while !self.stalled.load(Ordering::SeqCst) {
            assert!(t.elapsed() < WAIT_TIMEOUT, "the request never came");
            thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Release {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

pub fn archive_name(version: &str) -> String {
    format!("watane-{version}-{TARGET}.tar.gz")
}

pub fn write_file(p: &Path, body: &str) {
    fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
    let mut f = fs::File::create(p).expect("create");
    f.write_all(body.as_bytes()).expect("write");
}

/// A pid that is dead now.
pub fn dead_pid() -> u32 {
    let mut c = Command::new("/bin/sh");
    c.args(["-c", "exit 0"]);
    let mut child = c.spawn().expect("spawn");
    let pid = child.id();
    child.wait().expect("wait");
    pid
}

/// A checkout of watane in miniature: what `package.sh` reads, with fake programs (TECHNICAL 12.1).
pub struct Checkout {
    pub root: tempfile::TempDir,
}

impl Checkout {
    /// Programs print `<name> <version>`; `vendor` are the folders the fake `vendor.sh` makes.
    pub fn new(version: &str, vendor: &[&str]) -> Checkout {
        let root = tempfile::tempdir().expect("tempdir");
        let r = root.path();
        let w = |rel: &str, body: &str, mode: u32| {
            let p = r.join(rel);
            fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
            fs::write(&p, body).expect("write");
            fs::set_permissions(&p, fs::Permissions::from_mode(mode)).expect("chmod");
        };
        let me = Path::new(env!("CARGO_MANIFEST_DIR"));
        w(
            "install/package.sh",
            &fs::read_to_string(me.join("package.sh")).expect("package.sh"),
            0o755,
        );
        w(
            "install/install.sh",
            &fs::read_to_string(checkout_script()).expect("install.sh"),
            0o644,
        );
        for n in ["watane", "watane-dev", "watane-node"] {
            w(
                &format!("target/release/{n}"),
                &format!("#!/bin/sh\necho \"{n} {version}\"\n"),
                0o755,
            );
        }
        w(
            "target/release/bundle/macos/watane.app/Contents/Info.plist",
            "<plist/>\n",
            0o644,
        );
        w(
            "target/release/bundle/macos/watane.app/Contents/MacOS/watane",
            "#!/bin/sh\n",
            0o755,
        );
        let mut v = String::from("#!/bin/sh\nmkdir -p \"$1\"\n");
        for c in vendor {
            v.push_str(&format!(
                "mkdir -p \"$1/{c}\" && echo '[package]' > \"$1/{c}/Cargo.toml\"\n"
            ));
        }
        w("dev/scripts/vendor.sh", &v, 0o755);
        Checkout { root }
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.root.path().join(rel)
    }

    /// `sh install/package.sh <target> <version> <out>`, with `front` first on the `PATH`.
    pub fn pack(&self, version: &str, out: &Path, front: Option<&Path>) -> Out {
        let mut c = Command::new("/bin/sh");
        c.arg(self.path("install/package.sh"))
            .args([TARGET, version])
            .arg(out);
        if let Some(f) = front {
            let old = std::env::var("PATH").unwrap_or_default();
            c.env("PATH", format!("{}:{old}", f.display()));
        }
        exec(c)
    }
}

/// The names in a `.tar.gz`, without a trailing `/`, sorted.
pub fn listing(archive: &Path) -> Vec<String> {
    let o = Command::new(which("tar"))
        .arg("-tzf")
        .arg(archive)
        .output()
        .expect("tar");
    assert!(o.status.success(), "tar -t failed");
    let mut v: Vec<String> = String::from_utf8_lossy(&o.stdout)
        .lines()
        .map(|l| l.trim_end_matches('/').to_string())
        .collect();
    v.sort();
    v
}

/// A `.tar.gz` of one file with exactly this member name; the ustar header is written by hand,
/// so that no tar tool has a say in the name, then gzip'd with the tool.
pub fn archive_with_member(name: &str) -> Vec<u8> {
    let body = b"evil\n";
    let mut h = [0u8; 512];
    h[..name.len()].copy_from_slice(name.as_bytes());
    h[100..107].copy_from_slice(b"0000644");
    h[108..115].copy_from_slice(b"0000000");
    h[116..123].copy_from_slice(b"0000000");
    h[124..135].copy_from_slice(format!("{:011o}", body.len()).as_bytes());
    h[136..147].copy_from_slice(b"00000000000");
    h[156] = b'0';
    h[257..263].copy_from_slice(b"ustar\0");
    h[263..265].copy_from_slice(b"00");
    h[148..156].fill(b' ');
    let sum: u32 = h.iter().map(|b| u32::from(*b)).sum();
    h[148..155].copy_from_slice(format!("{sum:06o}\0").as_bytes());
    let mut tar = h.to_vec();
    tar.extend_from_slice(body);
    tar.resize(tar.len() + (512 - body.len()), 0);
    tar.extend_from_slice(&[0u8; 1024]);
    let d = tempfile::tempdir().expect("tempdir");
    let p = d.path().join("a.tar");
    fs::write(&p, &tar).expect("write");
    let st = Command::new(which("gzip")).arg(&p).status().expect("gzip");
    assert!(st.success());
    fs::read(d.path().join("a.tar.gz")).expect("read")
}
