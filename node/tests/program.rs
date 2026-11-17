//! S10 to S13: Alice runs her server as the program `watane-node`. Each test runs the
//! built binary and reads its stdout, stderr and exit code; the anchor is a `Node` of
//! this process.
#![allow(clippy::unwrap_used, reason = "tests may panic")]

mod support;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use support::*;
use watane_net::{
    BlobId, Entry, Event, Hit, Holdings, Ingested, Net, NodeKey, PublicKey, Role, Why,
};
use watane_node::{Node, words};

/// The program binds loopback only, which keeps mDNS off (node TECHNICAL 10).
const LO: &str = "127.0.0.1:0";

fn command(home: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_watane-node"));
    c.env_remove("WATANE_NODE_HOME").arg("--home").arg(home);
    c
}

struct Done {
    code: i32,
    stdout: String,
    stderr: String,
}

/// Runs a command to its end with nothing on standard input.
fn run_end(home: &Path, args: &[&str]) -> Done {
    let out = command(home)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    Done {
        code: out.status.code().unwrap(),
        stdout: String::from_utf8(out.stdout).unwrap(),
        stderr: String::from_utf8(out.stderr).unwrap(),
    }
}

/// A running program whose stdout lines arrive on a channel.
struct Prog {
    child: Child,
    lines: Receiver<String>,
    seen: Vec<String>,
    stderr: Arc<Mutex<String>>,
}
impl Prog {
    fn start(home: &Path, args: &[&str]) -> Prog {
        let mut child = command(home)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let (tx, lines) = channel();
        let stdout = child.stdout.take().unwrap();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let stderr = Arc::new(Mutex::new(String::new()));
        let (err, pipe) = (Arc::clone(&stderr), child.stderr.take().unwrap());
        std::thread::spawn(move || {
            for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                err.lock().unwrap().push_str(&format!("{line}\n"));
            }
        });
        Prog {
            child,
            lines,
            seen: Vec::new(),
            stderr,
        }
    }
    /// The next line that `want` accepts, within `secs`.
    fn line(&mut self, secs: u64, want: impl Fn(&str) -> bool) -> String {
        let end = Instant::now() + Duration::from_secs(secs);
        while let Some(left) = end.checked_duration_since(Instant::now()) {
            match self.lines.recv_timeout(left) {
                Ok(line) => {
                    self.seen.push(line.clone());
                    if want(&line) {
                        return line;
                    }
                }
                Err(_) => break,
            }
        }
        panic!("no such line within {secs} s; saw {:?}", self.seen);
    }
    fn code(&mut self, secs: u64) -> i32 {
        let end = Instant::now() + Duration::from_secs(secs);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status.code().unwrap_or(-1);
            }
            assert!(Instant::now() < end, "still running; saw {:?}", self.seen);
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn stderr(&self) -> String {
        self.stderr.lock().unwrap().clone()
    }
    fn signal(&self, name: &str) {
        let status = Command::new("kill")
            .arg(format!("-{name}"))
            .arg(self.child.id().to_string())
            .status()
            .unwrap();
        assert!(status.success());
    }
}
impl Drop for Prog {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// `watane-node join` against `anchor`: the anchor answers `yes`; returns the program's
/// lines and the key the anchor saw.
fn join_with(anchor: &Node, home: &Path, role: Role, yes: bool) -> (Prog, NodeKey) {
    let events = anchor.net().events();
    let code = anchor.net().pair_code(role).unwrap().to_string();
    let prog = Prog::start(home, &["join", "--bind", LO, &code]);
    let node = loop {
        if let Event::JoinAsked { node, role: r } =
            events.recv_timeout(Duration::from_secs(15)).unwrap()
        {
            assert_eq!(r, role);
            break node;
        }
    };
    let mut prog = prog;
    // the short key is shown before the anchor is asked
    let shown = prog.line(5, |l| l.starts_with("this machine: "));
    assert_eq!(shown, format!("this machine: {}", node.short()));
    prog.line(5, |l| l == "waiting for your answer on the anchor");
    anchor.net().answer_join(&node, yes);
    (prog, node)
}

/// A home that `watane-node join` paired as a full machine of `anchor`.
fn paired(anchor: &Node) -> (tempfile::TempDir, NodeKey) {
    let dir = tempfile::tempdir().unwrap();
    let (mut prog, key) = join_with(anchor, dir.path(), Role::Full, true);
    prog.line(15, |l| l.starts_with("joined as full; machines: "));
    assert_eq!(prog.code(10), 0);
    (dir, key)
}

fn book(home: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(home.join("net/book.json")).unwrap()).unwrap()
}

fn role_text(role: Role) -> &'static str {
    match role {
        Role::Full => "full",
        Role::Light => "light",
    }
}

fn machines_of(anchor: &Node) -> String {
    anchor
        .net()
        .devices()
        .unwrap()
        .devices
        .iter()
        .map(|(k, r)| format!("{} {}", k.short(), role_text(*r)))
        .collect::<Vec<_>>()
        .join(", ")
}

#[test]
fn s10_join_shows_its_key_waits_and_says_who_joined() {
    let dirs = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
    let anchor = app(dirs[0].path());

    let home = dirs[1].path();
    let (mut prog, key) = join_with(&anchor, home, Role::Full, true);
    let joined = prog.line(15, |l| l.starts_with("joined as "));
    assert_eq!(
        joined,
        format!("joined as full; machines: {}", machines_of(&anchor))
    );
    assert!(joined.contains(&key.short()) && joined.contains(&key_of(&anchor).short()));
    assert_eq!(prog.code(10), 0);
    let list = anchor.net().devices().unwrap();
    assert!(list.devices.contains(&(key, Role::Full)));
    assert_eq!(book(home)["on"], false);

    // a no: exit 1 and the cause
    let other = tempfile::tempdir().unwrap();
    let (mut prog, _) = join_with(&anchor, other.path(), Role::Full, false);
    assert_eq!(prog.code(15), 1);
    assert_eq!(prog.stderr().trim(), "watane-node: declined");
    assert_eq!(book(other.path())["on"], false);
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn s11_run_relay_serves_a_relay_and_two_homes_connect_through_it() {
    let dirs: Vec<_> = (0..3).map(|_| tempfile::tempdir().unwrap()).collect();
    let laptop = app(dirs[0].path());
    let second = keyless(dirs[1].path());
    pair(&laptop, &second, Role::Full, false);
    let (server_home, server_key) = paired(&laptop);
    // both machines go away and come back on new ports: what each stored of the other
    // is stale, and nothing tells them the new ones
    let (second_key, laptop_key) = (key_of(&second), key_of(&laptop));
    laptop.close();
    second.close();
    let laptop = app(dirs[0].path());
    let second = keyless(dirs[1].path());

    let port = free_port();
    let relay_arg = format!("127.0.0.1:{port}");
    let mut prog = Prog::start(
        server_home.path(),
        &["run", "--bind", LO, "--relay", &relay_arg],
    );
    prog.line(10, |l| {
        l == format!("running as full, node {}", server_key.short())
    });
    let relay_url = format!("http://127.0.0.1:{port}/");
    prog.line(10, |l| {
        l == format!("relay on {relay_arg}; add {relay_url} to the relays of your other machines")
    });

    let (laptop_events, second_events) = (laptop.net().events(), second.net().events());
    let with_relay = watane_net::Options {
        relays: vec![relay_url.parse().unwrap()],
        ..net_opts()
    };
    laptop.net().set_options(with_relay.clone()).unwrap();
    second.net().set_options(with_relay).unwrap();
    let peer_up = |events: &Receiver<Event>, key: NodeKey| {
        let end = Instant::now() + Duration::from_secs(15);
        while let Some(left) = end.checked_duration_since(Instant::now()) {
            match events.recv_timeout(left) {
                Ok(Event::PeerUp(k)) if k == key => return true,
                Ok(_) => {}
                Err(_) => return false,
            }
        }
        false
    };
    assert!(peer_up(&laptop_events, second_key), "laptop: no PeerUp");
    assert!(peer_up(&second_events, laptop_key), "second: no PeerUp");
    laptop.net().hello(&second_key).unwrap();

    prog.signal("TERM");
    assert_eq!(prog.code(10), 0);
}

#[test]
fn s11_a_relay_on_port_zero_is_a_usage_error() {
    let dir = tempfile::tempdir().unwrap();
    let done = run_end(dir.path(), &["run", "--relay", "127.0.0.1:0"]);
    assert_eq!(done.code, 2);
    assert!(done.stderr.contains("usage:"), "{}", done.stderr);
    assert!(done.stdout.is_empty());
}

fn status_of(home: &Path) -> String {
    let done = run_end(home, &["status"]);
    assert_eq!(done.code, 0, "{}", done.stderr);
    done.stdout
}

fn node_key_in(status: &str) -> NodeKey {
    status
        .lines()
        .find_map(|l| l.strip_prefix("node "))
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn s12_dht_on_asks_first_and_changes_nothing_without_a_yes() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let status = status_of(home);
    assert!(status.lines().any(|l| l == "dht off"), "{status}");
    let text = words::dht_on(&node_key_in(&status));

    let refused = run_end(home, &["dht", "on"]);
    assert_eq!(refused.code, 1);
    assert!(refused.stdout.contains(&text), "{}", refused.stdout);
    assert_eq!(refused.stderr.trim(), "watane-node: not changed");
    assert!(status_of(home).lines().any(|l| l == "dht off"));

    let yes = run_end(home, &["dht", "on", "--yes"]);
    assert_eq!(yes.code, 0, "{}", yes.stderr);
    assert!(yes.stdout.contains(&text));
    assert!(status_of(home).lines().any(|l| l == "dht on"));
    assert_eq!(book(home)["on"], false);

    let off = run_end(home, &["dht", "off"]);
    assert_eq!(off.code, 0, "{}", off.stderr);
    assert!(status_of(home).lines().any(|l| l == "dht off"));
}

#[test]
fn s12_a_typed_yes_answers_the_question_and_anything_else_does_not() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let ask = |answer: &str| {
        let mut child = command(dir.path())
            .args(["dht", "on"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(answer.as_bytes())
            .unwrap();
        child.wait().unwrap().code().unwrap()
    };
    assert_eq!(ask("sure\n"), 1);
    assert!(status_of(dir.path()).lines().any(|l| l == "dht off"));
    assert_eq!(ask("yes\n"), 0);
    assert!(status_of(dir.path()).lines().any(|l| l == "dht on"));
}

#[test]
fn s12_relay_add_asks_first_and_relay_remove_does_not() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let url = "http://127.0.0.1:9/";
    let text = words::relay_add(url);

    let refused = run_end(home, &["relay", "add", url]);
    assert_eq!(refused.code, 1);
    assert!(refused.stdout.contains(&text), "{}", refused.stdout);
    assert_eq!(refused.stderr.trim(), "watane-node: not changed");
    assert!(status_of(home).lines().any(|l| l == "relays none"));

    let yes = run_end(home, &["relay", "add", url, "--yes"]);
    assert_eq!(yes.code, 0, "{}", yes.stderr);
    assert!(yes.stdout.contains(&text));
    assert!(
        status_of(home)
            .lines()
            .any(|l| l == format!("relays {url}"))
    );
    assert_eq!(book(home)["relays"], serde_json::json!([url]));

    let removed = run_end(home, &["relay", "remove", url]);
    assert_eq!(removed.code, 0, "{}", removed.stderr);
    assert!(status_of(home).lines().any(|l| l == "relays none"));
}

/// A trusted machine whose holder claims the head of a real channel and answers
/// `entries` with entries of another channel. (A holder returns `Entry` values, which
/// cannot carry a changed byte: `Entry::decode` checks the signature. net's own tests
/// send changed bytes over the wire; here the bad object is an entry of the wrong author.)
struct Liar {
    channel: PublicKey,
    head: watane_net::HeadInfo,
    entries: Vec<Entry>,
}
impl Holdings for Liar {
    fn is_anchor(&self) -> bool {
        false
    }
    fn heads(&self, _: Option<&[PublicKey]>) -> Vec<(PublicKey, Option<watane_net::HeadInfo>)> {
        vec![(self.channel, Some(self.head))]
    }
    fn entries(&self, _: &PublicKey, from: u64, _: usize, _: usize) -> Vec<Entry> {
        self.entries.iter().skip(from as usize).cloned().collect()
    }
    fn fork(&self, _: &PublicKey) -> Option<(Entry, Entry)> {
        None
    }
    fn blob(&self, _: &BlobId) -> Option<std::path::PathBuf> {
        None
    }
    fn wanted_blobs(&self, _: Role) -> Vec<(BlobId, u64)> {
        Vec::new()
    }
    fn ingest(&self, _: &PublicKey, _: Vec<Entry>, _: Why) -> Ingested {
        Ingested::NotWanted
    }
    fn store_blob(&self, _: &BlobId, _: &Path) -> Result<(), String> {
        Err("no".into())
    }
    fn search(&self, _: &watane_net::Query) -> Vec<Hit> {
        Vec::new()
    }
}

#[test]
fn s13_run_prints_one_refused_line_for_a_machine_that_sends_a_bad_entry() {
    let dir = tempfile::tempdir().unwrap();
    let anchor = app(dir.path());
    let (server_home, _) = paired(&anchor);
    let mut prog = Prog::start(server_home.path(), &["run", "--bind", LO]);
    prog.line(10, |l| l.starts_with("running as full, node "));

    // a machine that holds another participant's channel and lies about its entries
    let other = Alice::new();
    let wrong = Alice::new();
    let head = other.dist.heads(None).unwrap()[0].1.unwrap();
    let liar = Liar {
        channel: other.key,
        head: watane_net::HeadInfo {
            seq: head.seq,
            id: head.id,
            forked: false,
        },
        entries: wrong.entries(),
    };
    let liar_dir = tempfile::tempdir().unwrap();
    let liar_net = Net::open(&liar_dir.path().join("net"), Arc::new(liar), net_opts()).unwrap();
    liar_net.set_options(net_opts()).unwrap();
    let liar_key = liar_net.node_key();
    let events = anchor.net().events();
    let code = anchor.net().pair_code(Role::Full).unwrap();
    std::thread::scope(|s| {
        let joined = s.spawn(|| liar_net.join(&code, false));
        loop {
            if let Event::JoinAsked { node, .. } =
                events.recv_timeout(Duration::from_secs(15)).unwrap()
            {
                // the anchor knows where the new machine is before it tells the others
                anchor.net().add_addrs(&node, &liar_net.addrs());
                anchor.net().answer_join(&node, true);
                break;
            }
        }
        joined.join().unwrap().unwrap();
    });
    liar_net.add_addrs(&key_of(&anchor), &anchor.net().addrs());

    let prefix = format!("refused {}: ", liar_key.short());
    let line = prog.line(20, |l| l.starts_with(&prefix));
    assert!(line.len() > prefix.len(), "no cause: {line}");
    // once, not at every sync
    std::thread::sleep(Duration::from_millis(1500));
    prog.signal("TERM");
    assert_eq!(prog.code(10), 0);
    let all: Vec<_> = prog.lines.try_iter().collect();
    assert!(
        all.iter().all(|l| !l.starts_with(&prefix)),
        "told again: {all:?}"
    );
    liar_net.close();
}

#[test]
fn program_run_needs_a_pairing_and_a_home_of_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let done = run_end(dir.path(), &["run"]);
    assert_eq!(done.code, 1);
    assert_eq!(
        done.stderr.trim(),
        "watane-node: not paired: run watane-node join <code> first"
    );

    // a home of the app or of `watane` is refused by every command
    let host = tempfile::tempdir().unwrap();
    std::fs::write(host.path().join("home.json"), "{}").unwrap();
    for args in [
        &["run"][..],
        &["status"],
        &["dht", "off"],
        &["dht", "on", "--yes"],
        &["relay", "remove", "http://127.0.0.1:9/"],
        &["relay", "add", "http://127.0.0.1:9/"],
    ] {
        let done = run_end(host.path(), args);
        assert_eq!(done.code, 1, "{args:?}");
        assert!(
            done.stderr.starts_with("watane-node: ")
                && done.stderr.contains("is a watane home with mods"),
            "{}",
            done.stderr
        );
    }
}

#[test]
fn program_bad_arguments_exit_2_with_the_usage_and_the_environment_stands_in_for_home() {
    let dir = tempfile::tempdir().unwrap();
    for args in [&["bogus"][..], &["status", "--wat"], &["join"], &[]] {
        let done = run_end(dir.path(), args);
        assert_eq!(done.code, 2, "{args:?}");
        assert!(done.stderr.contains("usage:"), "{}", done.stderr);
    }
    let out = Command::new(env!("CARGO_BIN_EXE_watane-node"))
        .arg("status")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let out = Command::new(env!("CARGO_BIN_EXE_watane-node"))
        .arg("status")
        .env("WATANE_NODE_HOME", dir.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn program_run_ends_on_sigterm_and_sigint_with_net_off_and_status_says_where_it_stands() {
    let dir = tempfile::tempdir().unwrap();
    let anchor = app(dir.path());
    let (home, key) = paired(&anchor);

    let list = anchor.net().devices().unwrap();
    let status = status_of(home.path());
    let has = |line: String| assert!(status.lines().any(|l| l == line), "{line}\n{status}");
    has(format!("node {key}"));
    has("machine full".into());
    has(format!("anchor {}", key_of(&anchor).short()));
    has(format!(
        "devices seq {}, expires never: {}",
        list.seq,
        machines_of(&anchor)
    ));
    has("relays none".into());
    has("dht off".into());
    has("channels held 0".into());
    has("packages missing 0".into());

    for signal in ["TERM", "INT"] {
        let mut prog = Prog::start(home.path(), &["run", "--bind", LO]);
        prog.line(10, |l| {
            l == format!("running as full, node {}", key.short())
        });
        assert_eq!(book(home.path())["on"], true);
        prog.signal(signal);
        assert_eq!(prog.code(10), 0, "{signal}");
        assert_eq!(book(home.path())["on"], false, "{signal}");
    }
}

#[test]
fn s10_a_signal_during_join_ends_it_with_net_off() {
    let dirs = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
    let anchor = app(dirs[0].path());
    // the anchor is asked and never answers
    let events = anchor.net().events();
    let code = anchor.net().pair_code(Role::Full).unwrap().to_string();
    let mut prog = Prog::start(dirs[1].path(), &["join", "--bind", LO, &code]);
    prog.line(10, |l| l == "waiting for your answer on the anchor");
    loop {
        if let Event::JoinAsked { .. } = events.recv_timeout(Duration::from_secs(15)).unwrap() {
            break;
        }
    }
    assert_eq!(book(dirs[1].path())["on"], true);
    prog.signal("INT");
    assert_eq!(prog.code(10), 1);
    assert_eq!(prog.stderr().trim(), "watane-node: interrupted");
    assert_eq!(book(dirs[1].path())["on"], false);
}

fn usage_ok(done: &Done) {
    assert_eq!(done.code, 0, "{}", done.stderr);
    assert!(done.stdout.starts_with("usage:"), "{}", done.stdout);
}

#[test]
fn ac1_help_prints_the_usage_on_stdout_with_no_home() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("never");
    for args in [
        &["--help"][..],
        &["-h"],
        &["help"],
        &["run", "--help"],
        &["status", "-h"],
    ] {
        usage_ok(&run_end(&home, args));
    }
    let bare = Command::new(env!("CARGO_BIN_EXE_watane-node"))
        .env_remove("WATANE_NODE_HOME")
        .arg("--help")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(bare.status.code(), Some(0));
    assert!(String::from_utf8(bare.stdout).unwrap().contains("usage:"));
    assert!(!home.exists());
    let bad = run_end(&home, &["status", "--wat"]);
    assert_eq!(bad.code, 2);
    assert!(bad.stderr.contains("unknown flag --wat"), "{}", bad.stderr);
    assert!(bad.stderr.contains("usage:"));
}

const BUSY: &str = "another watane-node is running on this home";

#[test]
fn ac2_ac3_status_reads_beside_run_and_a_second_command_says_who_is_running() {
    let dir = tempfile::tempdir().unwrap();
    let anchor = app(dir.path());
    let (home, key) = paired(&anchor);
    let before = status_of(home.path());
    let mut prog = Prog::start(home.path(), &["run", "--bind", LO]);
    prog.line(10, |l| l.starts_with("running as full"));

    let beside = status_of(home.path());
    let want: Vec<&str> = before
        .lines()
        .filter(|l| !l.starts_with("channels held") && !l.starts_with("packages missing"))
        .collect();
    let got: Vec<&str> = beside.lines().collect();
    assert_eq!(got[..want.len()], want[..], "{beside}\nagainst\n{before}");
    assert_eq!(
        got[want.len()..],
        ["channels and packages: watane-node run is running on this home"]
    );
    assert!(beside.contains(&format!("node {key}")));

    let text = format!(
        "watane-node: {BUSY} ({}); stop it first",
        home.path().display()
    );
    let book_before = book(home.path());
    for args in [
        &["run", "--bind", LO][..],
        &["dht", "on", "--yes"],
        &["relay", "add", "http://127.0.0.1:9/", "--yes"],
        &["relay", "remove", "http://127.0.0.1:9/"],
        &["dht", "off"],
    ] {
        let done = run_end(home.path(), args);
        assert_eq!(done.code, 1, "{args:?}");
        assert_eq!(done.stderr.trim(), text, "{args:?}");
    }
    let code = anchor.net().pair_code(Role::Full).unwrap().to_string();
    let done = run_end(home.path(), &["join", "--replace", "--bind", LO, &code]);
    assert_eq!(done.code, 1);
    assert_eq!(done.stderr.trim(), text);
    let mut now = book(home.path());
    let mut then = book_before;
    now["on"] = then["on"].clone(); // run's own switch
    then["on"] = now["on"].clone();
    assert_eq!(now, then);
    assert_eq!(prog.child.try_wait().unwrap(), None);

    prog.signal("TERM");
    assert_eq!(prog.code(10), 0);
    assert_eq!(status_of(home.path()), before);
}

#[test]
fn ac4_ac5_the_answer_gets_its_own_line_and_a_change_is_confirmed() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let ask = |args: &[&str], answer: &str| {
        let mut child = command(dir.path())
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(answer.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        (
            out.status.code().unwrap(),
            String::from_utf8(out.stdout).unwrap(),
        )
    };
    let url = "http://127.0.0.1:9/";

    let (code, stdout) = ask(&["dht", "on"], "y\n");
    assert_eq!(code, 0);
    assert!(
        stdout.lines().any(|l| l == "Turn this on? [y/N] y"),
        "{stdout}"
    );
    assert_eq!(stdout.lines().last(), Some("dht on"), "{stdout}");

    let (code, stdout) = ask(&["dht", "off"], "");
    assert_eq!(code, 0);
    assert_eq!(stdout, "dht off\n");

    let (code, stdout) = ask(&["dht", "on"], "");
    assert_eq!(code, 1);
    assert!(stdout.ends_with("Turn this on? [y/N] \n"), "{stdout:?}");
    assert!(!stdout.contains("dht on"));

    let (code, stdout) = ask(&["relay", "add", url], "no\n");
    assert_eq!(code, 1);
    assert!(stdout.ends_with("Turn this on? [y/N] no\n"), "{stdout:?}");
    assert!(!stdout.contains("relays"));

    let (code, stdout) = ask(&["relay", "add", url], "yes\n");
    assert_eq!(code, 0);
    assert_eq!(
        stdout.lines().last(),
        Some(format!("relays {url}").as_str())
    );
    let (code, stdout) = ask(&["relay", "remove", url], "");
    assert_eq!(code, 0);
    assert_eq!(stdout, "relays none\n");
}
