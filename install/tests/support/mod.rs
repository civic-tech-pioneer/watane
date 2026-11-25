//! Test support for `script.rs`: the fake release, the fake tools and running `install.sh` (TECHNICAL 12.1).
#![allow(dead_code, reason = "s50b-3 and the other test files use the rest")]

pub mod pty;
pub mod unit;

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub const TARGET: &str = "aarch64-apple-darwin";
pub const PERL: &str = "/usr/bin/perl";
/// `perl` arguments that start the rest in a new session with the signals the script traps at
/// their defaults: a shell started in the background ignores INT, and an ignored signal stays
/// ignored through `exec`, so the script's trap and `curl` would never see the test's INT.
pub const LAUNCH: [&str; 3] = [
    "-MPOSIX",
    "-e",
    "POSIX::setsid(); $ENV{FAKE_INSTALL_PID} = $$; $SIG{$_} = 'DEFAULT' for qw(HUP INT QUIT TERM PIPE); exec @ARGV",
];

/// Endpoint security can queue each exec for tens of seconds on a loaded Mac.
pub const WAIT_TIMEOUT: Duration = Duration::from_secs(600);
/// How many runs of the script may be alive at once (TECHNICAL 12.1).
const GATE_LIMIT: usize = 4;

/// Counts the runs alive; `acquire` waits while `GATE_LIMIT` are.
pub struct Gate {
    used: Mutex<usize>,
    ready: Condvar,
}

impl Gate {
    pub const fn new() -> Self {
        Gate {
            used: Mutex::new(0),
            ready: Condvar::new(),
        }
    }

    pub fn acquire(&'static self) -> RunPermit {
        let mut used = self
            .ready
            .wait_while(self.used.lock().expect("runs"), |n| *n >= GATE_LIMIT)
            .expect("runs");
        *used += 1;
        RunPermit(self)
    }
}

/// The gate of the test binary: a test that starts the script takes a permit first.
static RUNS: Gate = Gate::new();

pub struct RunPermit(&'static Gate);

impl RunPermit {
    pub fn acquire() -> Self {
        RUNS.acquire()
    }
}

impl Drop for RunPermit {
    fn drop(&mut self) {
        *self.0.used.lock().expect("runs") -= 1;
        self.0.ready.notify_one();
    }
}

pub struct RunChild {
    child: Child,
    _permit: Option<RunPermit>,
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
        _permit: Some(permit),
    }
}

/// `spawn_run` without the gate, for a test of the launcher or of a test helper that starts no
/// run of the script: it need not wait for the runs of other tests, nor hold one back.
pub fn spawn_free(c: &mut Command) -> RunChild {
    c.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = RunChild {
        child: c.spawn().expect("spawn"),
        _permit: None,
    };
    drop(child.stdin.take());
    child
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

/// The shell the fakes and the fake programs run under: `dash` where there is one, since
/// macOS's `/bin/sh` starts bash in a second process. `install.sh` itself runs under `/bin/sh`.
pub fn fake_shell() -> &'static str {
    if Path::new("/bin/dash").exists() {
        "/bin/dash"
    } else {
        "/bin/sh"
    }
}

/// The fakes of `tests/fakes`, under `fake_shell`, written once per test binary: `bin` holds
/// those every test has, `faults-cp`, `faults-mv`, `faults-rm` and `faults-chmod` hold the fake of
/// that name (each a shell that runs the real tool, so a run that does not inject a fault would pay
/// two process starts for every call: only a test that writes the fault file of a tool puts that
/// tool's folder on the `PATH`), `rustup` holds `rustup` and `rustc`. They keep no state of their own
/// (that is in `$FAKE_STATE`), so every test shares them.
fn shared_fakes() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fakes");
        let mut files: Vec<(String, String)> = fs::read_dir(&src)
            .expect("fakes dir")
            .map(|e| {
                let e = e.expect("entry");
                let body = fs::read_to_string(e.path()).expect("read fake");
                let body = body.replacen("#!/bin/sh\n", &format!("#!{}\n", fake_shell()), 1);
                (e.file_name().to_string_lossy().into_owned(), body)
            })
            .collect();
        files.sort();
        let mut h = std::hash::DefaultHasher::new();
        std::hash::Hash::hash(&files, &mut h);
        std::hash::Hash::hash("layout: bin, faults-<tool>, rustup", &mut h);
        let base = Path::new(env!("CARGO_TARGET_TMPDIR"));
        let dir = base.join(format!(
            "install-fakes-{:016x}",
            std::hash::Hasher::finish(&h)
        ));
        if !dir.is_dir() {
            // Written aside, then renamed, so a test binary running at the same time never
            // sees half a folder.
            let tmp = base.join(format!("install-fakes-tmp-{}", std::process::id()));
            let _ = fs::remove_dir_all(&tmp);
            for (name, body) in &files {
                let sub = match name.as_str() {
                    "rustup" | "rustc" => "rustup".to_string(),
                    "cp" | "mv" | "rm" | "chmod" => format!("faults-{name}"),
                    _ => "bin".to_string(),
                };
                let to = tmp.join(sub).join(name);
                fs::create_dir_all(to.parent().expect("parent")).expect("mkdir");
                fs::write(&to, body).expect("write fake");
                fs::set_permissions(&to, fs::Permissions::from_mode(0o755)).expect("chmod");
            }
            if fs::rename(&tmp, &dir).is_err() {
                let _ = fs::remove_dir_all(&tmp); // another binary was first
            }
        }
        dir
    })
}

/// A folder under `CARGO_TARGET_TMPDIR` for what the tests make once and keep between runs: the
/// fake archives and the home of one install, so that a run of one module, or of the whole
/// binary, starts no process to make them again. Its name holds a hash of everything their content
/// depends on (`install.sh`, this file, the folder of the fakes), so a stale one is never read.
fn cache_dir() -> &'static Path {
    static DIR: OnceLock<(PathBuf, fs::File)> = OnceLock::new();
    &DIR.get_or_init(|| {
        let mut h = std::hash::DefaultHasher::new();
        std::hash::Hash::hash(&fs::read(checkout_script()).expect("install.sh"), &mut h);
        std::hash::Hash::hash(include_str!("mod.rs"), &mut h);
        std::hash::Hash::hash(&shared_fakes().display().to_string(), &mut h);
        let base = Path::new(env!("CARGO_TARGET_TMPDIR"));
        let dir = base.join(format!(
            "install-cache-{:016x}",
            std::hash::Hasher::finish(&h)
        ));
        // Serialize opening a generation with pruning it. Keep a shared directory
        // lock for the lifetime of the test binary so an old but active generation
        // is never removed under a concurrent reader or writer.
        let gate = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(base.join(".install-cache.lock"))
            .expect("cache gate");
        gate.lock().expect("lock cache gate");
        fs::create_dir_all(&dir).expect("cache dir");
        let active = fs::File::open(&dir).expect("open cache directory");
        active.lock_shared().expect("lock cache directory");
        prune_install_caches(base, &dir, Duration::from_secs(7 * 24 * 60 * 60));
        (dir, active)
    })
    .0
}

fn prune_install_caches(base: &Path, active: &Path, max_age: Duration) {
    for entry in fs::read_dir(base).expect("cache parent").flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let name = entry.file_name();
        let Some(hash) = name
            .to_str()
            .and_then(|name| name.strip_prefix("install-cache-"))
        else {
            continue;
        };
        let path = entry.path();
        if path == active || hash.len() != 16 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            continue;
        }
        let Ok(dir) = fs::File::open(&path) else {
            continue;
        };
        if dir.try_lock().is_ok()
            && dir
                .metadata()
                .and_then(|meta| meta.modified())
                .ok()
                .and_then(|mtime| mtime.elapsed().ok())
                .is_some_and(|age| age >= max_age)
        {
            let _ = fs::remove_dir_all(path);
        }
    }
}

#[test]
fn cache_pruning_keeps_current_busy_and_unrelated_directories() {
    let base = tempfile::tempdir().expect("cache parent");
    let make = |name| {
        let path = base.path().join(name);
        fs::create_dir(&path).expect("cache directory");
        path
    };
    let active = make("install-cache-0000000000000001");
    let busy = make("install-cache-0000000000000002");
    let stale = make("install-cache-0000000000000003");
    let unrelated = make("install-cache-not-a-hash");
    let lock = fs::File::open(&busy).expect("busy directory");
    lock.lock_shared().expect("busy reader");

    prune_install_caches(base.path(), &active, Duration::ZERO);
    assert!(active.is_dir() && busy.is_dir() && unrelated.is_dir());
    assert!(!stale.exists());
}

/// The archive of `version` with these vendored crates and its sum, from the cache or made now.
/// The two are written aside in one folder that is then renamed into place, so that a reader
/// never pairs the archive of one test binary with the sum of another (`tar` stamps times, so
/// two packings differ); the first binary to rename wins.
fn cached_archive(version: &str, vendor: &[&str]) -> (Vec<u8>, String) {
    let name = format!("archive-{version}-{}", vendor.join("+"));
    let dir = cache_dir().join(&name);
    if let (Ok(bytes), Ok(text)) = (
        fs::read(dir.join("a.tar.gz")),
        fs::read_to_string(dir.join("sha256")),
    ) {
        return (bytes, text.trim().to_string());
    }
    let bytes = build_archive(version, vendor);
    let hash = sha256(&bytes);
    let aside = cache_dir().join(format!("{name}.tmp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&aside);
    fs::create_dir_all(&aside).expect("aside");
    fs::write(aside.join("a.tar.gz"), &bytes).expect("write archive");
    fs::write(aside.join("sha256"), hash.as_bytes()).expect("write sum");
    if fs::rename(&aside, &dir).is_err() {
        let _ = fs::remove_dir_all(&aside); // another binary was first: use its pair
        if let (Ok(b), Ok(t)) = (
            fs::read(dir.join("a.tar.gz")),
            fs::read_to_string(dir.join("sha256")),
        ) {
            return (b, t.trim().to_string());
        }
    }
    (bytes, hash)
}

/// One test's world: a temporary `HOME` and `TMPDIR`, the fake tools, the fake release.
/// `fakes` is this test's own folder, first on the `PATH` and empty unless the scenario adds
/// `rustup` (`add_rustup`) or another fake; the shared fakes come after it.
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
    /// Set by `state_write` of a fault file: the fake of that tool is on the `PATH` (one bit per
    /// entry of `FAULT_FILES`).
    faulty: AtomicU8,
}

/// The fault file of each fake that has one: the fake of the tool is on the `PATH` while a test
/// has written it (`tests/fakes`).
const FAULT_FILES: [(&str, &str); 4] = [
    ("cp_fault", "cp"),
    ("mv_fault", "mv"),
    ("rm_fault", "rm"),
    ("chmod_wait", "chmod"),
];

const TOOLS: [&str; 24] = [
    "sh", "curl", "tar", "mktemp", "mkdir", "cp", "rmdir", "ls", "cat", "chmod", "grep", "sed",
    "awk", "tr", "head", "tail", "tee", "dirname", "readlink", "dd", "od", "mv", "rm",
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
        for t in ["mv", "rm", "sleep", "mkdir", "chmod", "cp"] {
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
            faulty: AtomicU8::new(0),
        };
        fx.bake_script("0.1.0");
        fx.rel.set_version("0.1.0", &["serde-1.0.0"]);
        fx
    }

    /// A world in which `0.1.0` is installed, as `Fx::new()` then one run of the script leaves it,
    /// but without that run: the home of one real install is made once per test binary and
    /// written again here (its own paths in the receipt rewritten), so that a test that starts
    /// from an installed home does not pay the ~45 process starts of an install.
    pub fn installed() -> Fx {
        Fx::installed_with(&["serde-1.0.0"])
    }

    /// `installed()` with the release `0.1.0` holding these vendored crates.
    pub fn installed_with(vendor: &[&str]) -> Fx {
        let fx = Fx::new();
        fx.rel.set_version("0.1.0", vendor);
        write_installed(&installed_template(vendor), &fx.home);
        fx
    }

    /// A second home of this world, `<root>/<name>`, with the `0.1.0` install in it (see
    /// `installed`), for a rule that is run against several homes in one process.
    pub fn installed_home(&self, name: &str) -> PathBuf {
        let home = self.root.path().join(name);
        fs::create_dir(&home).expect("home");
        write_installed(&installed_template(&["serde-1.0.0"]), &home);
        home
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
        if let Some(i) = FAULT_FILES.iter().position(|(f, _)| *f == name) {
            self.faulty.fetch_or(1 << i, Ordering::SeqCst);
        }
        fs::write(self.state.join(name), content).expect("state write");
    }
    /// Removes a fault file the test wrote, and takes the fake of its tool off the `PATH` again.
    pub fn unfault(&self, name: &str) {
        let _ = fs::remove_file(self.state.join(name));
        if let Some(i) = FAULT_FILES.iter().position(|(f, _)| *f == name) {
            self.faulty.fetch_and(!(1 << i), Ordering::SeqCst);
        }
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
        // A copy, not a link: a test that rewrites its rustup must not change the shared one.
        for n in ["rustup", "rustc"] {
            fs::copy(shared_fakes().join("rustup").join(n), self.fakes.join(n)).expect("copy fake");
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
        let on = self.faulty.load(Ordering::SeqCst);
        let faults: String = FAULT_FILES
            .iter()
            .enumerate()
            .filter(|(i, _)| on & (1 << i) != 0)
            .map(|(_, (_, tool))| {
                format!(
                    "{}:",
                    shared_fakes().join(format!("faults-{tool}")).display()
                )
            })
            .collect();
        format!(
            "{}:{faults}{}:{}",
            self.fakes.display(),
            shared_fakes().join("bin").display(),
            self.tools.display()
        )
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
        c.args(LAUNCH).arg("/bin/sh");
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
    finish_within(run, WAIT_TIMEOUT).unwrap_or_else(|out| panic!("run never ended: {out:?}"))
}

/// Waits until the run ended and its pipes closed; past `limit` it kills the run's process group
/// (a process left behind can hold the pipes forever) and returns what it printed as `Err`.
pub fn finish_within(run: RunChild, limit: Duration) -> Result<Out, Out> {
    let RunChild { child, _permit } = run;
    let pid = child.id();
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || tx.send(child.wait_with_output().expect("wait")));
    let (o, ended) = match rx.recv_timeout(limit) {
        Ok(o) => (o, true),
        Err(_) => {
            // the run is not reaped yet, so its pid still names it and its group
            let _ = Command::new("/bin/sh")
                .args([
                    "-c",
                    &format!("kill -s KILL -- -{pid} || kill -s KILL {pid}"),
                ])
                .status();
            (rx.recv().expect("wait"), false)
        }
    };
    let out = Out {
        code: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    };
    if ended { Ok(out) } else { Err(out) }
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
            &format!("#!{}\necho \"{n} {version}\"\n", fake_shell()),
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

/// The home of one real install of `0.1.0` with this vendor list, made once per test binary.
fn installed_template(vendor: &[&str]) -> Arc<Template> {
    // One cell per vendor list: the first test that needs it builds it while the others wait, so
    // that the number of installs made does not depend on how many tests start together.
    type Cells = HashMap<Vec<String>, Arc<OnceLock<Arc<Template>>>>;
    static MADE: OnceLock<Mutex<Cells>> = OnceLock::new();
    let key: Vec<String> = vendor.iter().map(|v| (*v).to_string()).collect();
    let cell = Arc::clone(
        MADE.get_or_init(Mutex::default)
            .lock()
            .expect("made")
            .entry(key)
            .or_default(),
    );
    Arc::clone(cell.get_or_init(|| {
        let dir = cache_dir().join(format!("template-{}", vendor.join("+")));
        if let Ok(meta) = fs::read_to_string(dir.join("meta")) {
            let mut l = meta.lines();
            let (home, home_r) = (l.next().expect("home"), l.next().expect("home_r"));
            return Arc::new(Template {
                home: home.to_string(),
                home_r: home_r.to_string(),
                entries: tree_entries(&dir.join("home")),
            });
        }
        let fx = Fx::new();
        fx.rel.set_version("0.1.0", vendor);
        let out = fx.run(&[]);
        assert_eq!(out.code, 0, "the template install: {out:?}");
        let tmpl = Template {
            home: fx.home.display().to_string(),
            home_r: fs::canonicalize(&fx.home)
                .expect("canonical home")
                .display()
                .to_string(),
            entries: tree_entries(&fx.home),
        };
        // written aside and renamed, so that a reader sees all of it or nothing
        let aside = cache_dir().join(format!(
            "template-{}.tmp-{}",
            vendor.join("+"),
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&aside);
        fs::create_dir_all(aside.join("home")).expect("aside");
        tmpl.write_into(&aside.join("home"));
        fs::write(
            aside.join("meta"),
            format!("{}\n{}\n", tmpl.home, tmpl.home_r),
        )
        .expect("meta");
        if fs::rename(&aside, &dir).is_err() {
            let _ = fs::remove_dir_all(&aside); // another binary was first
        }
        Arc::new(tmpl)
    }))
}

/// Writes the template into the empty folder `home`, its own paths in the receipt rewritten.
fn write_installed(tmpl: &Template, home: &Path) {
    tmpl.write_into(home);
    let home_r = fs::canonicalize(home).expect("canonical home");
    // the longer (canonical) form first: it holds the other as its tail on macOS
    let swaps = [
        (tmpl.home_r.clone(), home_r.display().to_string()),
        (tmpl.home.clone(), home.display().to_string()),
    ];
    let p = home.join(".local/share/watane/receipt");
    let mut text = fs::read_to_string(&p).expect("receipt");
    for (from, to) in &swaps {
        text = text.replace(from.as_str(), to);
    }
    let mut receipt = String::new();
    for line in text.lines() {
        if let Some((folder, _)) = line
            .strip_prefix("created ")
            .and_then(|line| line.rsplit_once(' '))
        {
            let inode = fs::metadata(home.join(folder))
                .expect("created directory")
                .ino();
            receipt.push_str(&format!("created {folder} {inode}\n"));
        } else {
            receipt.push_str(line);
            receipt.push('\n');
        }
    }
    fs::write(&p, receipt).expect("write receipt");
}

/// One node of a home, as `tree_entries` records it.
enum Node {
    Dir(u32),
    File(u32, Vec<u8>),
}

/// The home of one real install (`Fx::installed`).
struct Template {
    home: String,
    home_r: String,
    entries: Vec<(String, Node)>,
}

impl Template {
    fn write_into(&self, home: &Path) {
        for (rel, node) in &self.entries {
            let p = home.join(rel);
            match node {
                Node::Dir(mode) => {
                    fs::create_dir_all(&p).expect("mkdir");
                    fs::set_permissions(&p, fs::Permissions::from_mode(*mode)).expect("chmod");
                }
                Node::File(mode, bytes) => {
                    fs::write(&p, bytes).expect("write");
                    fs::set_permissions(&p, fs::Permissions::from_mode(*mode)).expect("chmod");
                }
            }
        }
    }
}

/// Every folder and file under `root`, parents first, with modes; the install makes no link.
fn tree_entries(root: &Path) -> Vec<(String, Node)> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, Node)>) {
        let mut es: Vec<_> = fs::read_dir(dir)
            .expect("read_dir")
            .map(|e| e.expect("e"))
            .collect();
        es.sort_by_key(|e| e.file_name());
        for e in es {
            let p = e.path();
            let rel = p.strip_prefix(base).expect("prefix").display().to_string();
            let md = fs::symlink_metadata(&p).expect("md");
            let mode = md.permissions().mode() & 0o7777;
            if md.is_dir() {
                out.push((rel, Node::Dir(mode)));
                walk(base, &p, out);
            } else {
                assert!(md.is_file(), "the install made a link: {}", p.display());
                out.push((rel, Node::File(mode, fs::read(&p).expect("read"))));
            }
        }
    }
    let mut v = Vec::new();
    walk(root, root, &mut v);
    v
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
    /// The archive and its sum are made once per test binary for each version and vendor list.
    pub fn set_version(&self, version: &str, vendor: &[&str]) {
        // One cell per version and vendor list, built by the first test that asks while the others
        // wait (the packing runs `tar` and `gzip`: how many tests start together must not count).
        type Made = HashMap<(String, Vec<String>), Arc<OnceLock<(Vec<u8>, String)>>>;
        static MADE: OnceLock<Mutex<Made>> = OnceLock::new();
        let key = (
            version.to_string(),
            vendor.iter().map(|v| (*v).to_string()).collect(),
        );
        let cell = Arc::clone(
            MADE.get_or_init(Mutex::default)
                .lock()
                .expect("made")
                .entry(key)
                .or_default(),
        );
        let (bytes, sum) = cell.get_or_init(|| cached_archive(version, vendor)).clone();
        let name = archive_name(version);
        self.put("SHA256SUMS", format!("{sum}  {name}\n").into_bytes());
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
