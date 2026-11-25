//! S80: Alice's server says what is wrong. The server is the built `watane-node`; the anchor
//! and the light laptop are `Node`s of this process.
#![allow(clippy::unwrap_used, reason = "tests may panic")]

use crate::support;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};
use support::*;
use watane_net::{Event, Role};
use watane_node::Node;

const LO: &str = "127.0.0.1:0";

fn command(home: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_watane-node"));
    c.env_remove("WATANE_NODE_HOME").arg("--home").arg(home);
    c
}

struct Done {
    code: i32,
    stderr: String,
}

fn run_end(home: &Path, args: &[&str]) -> Done {
    let out = command(home)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    Done {
        code: out.status.code().unwrap(),
        stderr: String::from_utf8(out.stderr).unwrap(),
    }
}

/// A running `watane-node run` whose stdout lines arrive on a channel.
struct Prog {
    child: Child,
    lines: Receiver<String>,
    seen: Vec<String>,
}
impl Prog {
    fn start(home: &Path, args: &[&str]) -> Prog {
        let mut child = command(home)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
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
        Prog {
            child,
            lines,
            seen: Vec::new(),
        }
    }
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
}
impl Drop for Prog {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// `watane-node join` of a fresh home with `code`; the anchor says yes.
fn join_yes(anchor: &Node, home: &Path, code: &str) -> Done {
    let events = anchor.net().events();
    let mut join = command(home)
        .args(["join", "--bind", LO, code])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let node = loop {
        if let Event::JoinAsked { node, .. } = events.recv_timeout(Duration::from_secs(15)).unwrap()
        {
            break node;
        }
    };
    anchor.net().answer_join(&node, true);
    let out = join.wait_with_output_stderr();
    Done {
        code: out.0,
        stderr: out.1,
    }
}

trait WaitStderr {
    fn wait_with_output_stderr(&mut self) -> (i32, String);
}
impl WaitStderr for Child {
    fn wait_with_output_stderr(&mut self) -> (i32, String) {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut self.stderr.take().unwrap(), &mut text).unwrap();
        (self.wait().unwrap().code().unwrap(), text)
    }
}

/// Alice's anchor, her server run by `watane-node run`, and a light laptop that knows a
/// release whose package the server has stored, as the file `copy`.
struct Stored {
    dirs: Vec<tempfile::TempDir>,
    anchor: Node,
    light: Node,
    server: Prog,
    server_key: watane_net::NodeKey,
    published: watane_dist::Published,
    copy: PathBuf,
}

fn stored() -> Stored {
    let dirs: Vec<_> = (0..3).map(|_| tempfile::tempdir().unwrap()).collect();
    let anchor = app(dirs[0].path());
    let light = keyless(dirs[2].path());
    pair(&anchor, &light, Role::Light, false);

    let code = anchor.net().pair_code(Role::Full).unwrap().to_string();
    assert_eq!(join_yes(&anchor, dirs[1].path(), &code).code, 0);
    let port = free_port();
    let bind = format!("127.0.0.1:{port}");
    let mut server = Prog::start(dirs[1].path(), &["run", "--bind", &bind]);
    server.line(10, |l| l.starts_with("running as full, node "));
    let server_key = anchor
        .net()
        .devices()
        .unwrap()
        .devices
        .iter()
        .find(|(k, r)| *r == Role::Full && *k != key_of(&anchor))
        .unwrap()
        .0;
    light.net().add_addrs(&server_key, &[bind.parse().unwrap()]);

    let published = anchor.publish(package("1.0.0")).unwrap();
    let alice = anchor.dist().identity().unwrap();
    // the light laptop learns the release (not the package) while it is still a machine
    assert!(wait_for(15, || {
        light.net().need(&alice).is_ok() && head(&light, &alice).is_some()
    }));
    // the server stores the package: its cache file, not a temporary of the same size that the
    // fetch and the cache write pass through on the way
    let name = anchor
        .dist()
        .package_file(&published.info.release.package)
        .unwrap()
        .file_name()
        .unwrap()
        .to_owned();
    let copy = dirs[1].path().join("dist/packages").join(name);
    assert!(wait_for(20, || copy.is_file()));
    Stored {
        dirs,
        anchor,
        light,
        server,
        server_key,
        published,
        copy,
    }
}

#[test]
fn ac1_a_removed_light_machine_is_told_the_server_refused() {
    let Stored {
        dirs: _dirs,
        anchor,
        light,
        mut server,
        server_key,
        published,
        ..
    } = stored();
    let after = anchor.net().remove_device(&key_of(&light)).unwrap();
    server.line(15, |l| l.starts_with(&format!("machines {}:", after.seq)));

    // only the server answers: the anchor would refuse the removed machine too
    anchor.net().set_options(Default::default()).unwrap();
    let chain = chain(&light.fetch(&published.info.reference).unwrap_err());
    assert!(
        chain.contains(&format!("node {} refused: not_allowed", server_key.short())),
        "{chain}"
    );
    assert!(!chain.contains("is not followed"), "{chain}");
}

#[test]
fn ac2_a_bad_copy_on_the_server_is_not_read_as_not_followed() {
    let Stored {
        dirs: _dirs,
        anchor,
        light,
        server: _server,
        server_key,
        published,
        copy,
    } = stored();
    // the server's stored package no longer matches its hash
    std::fs::write(&copy, vec![0x5au8; std::fs::read(&copy).unwrap().len()]).unwrap();

    // only the server answers
    anchor.net().set_options(Default::default()).unwrap();
    let chain = chain(&light.fetch(&published.info.reference).unwrap_err());
    assert!(
        chain.contains(&format!(
            "node {} did not serve package",
            server_key.short()
        )) || chain.contains(&format!("node {} sent a bad object", server_key.short())),
        "{chain}"
    );
    assert!(!chain.contains("is not followed"), "{chain}");
}

#[test]
fn ac1_every_full_machine_off_says_no_full_machine_is_reachable() {
    let Stored {
        dirs: _dirs,
        anchor,
        light,
        mut server,
        published,
        ..
    } = stored();
    // the server and the anchor are both off
    let _ = server.child.kill();
    let _ = server.child.wait();
    anchor.net().set_options(Default::default()).unwrap();
    let error = light.fetch(&published.info.reference).unwrap_err();
    let chain = chain(&error);
    assert!(
        error.to_string().starts_with("your full machine"),
        "{chain}"
    );
    assert!(chain.contains("no full machine is reachable"), "{chain}");
    assert!(!chain.contains("is not followed"), "{chain}");
}

#[test]
fn ac1_a_web_host_still_wins_when_no_full_machine_is_reachable() {
    let dirs: Vec<_> = (0..2).map(|_| tempfile::tempdir().unwrap()).collect();
    let anchor = app(dirs[0].path());
    let light = keyless(dirs[1].path());
    pair(&anchor, &light, Role::Light, false);
    anchor.net().set_options(Default::default()).unwrap();
    let alice = Alice::new();
    // net is on and asked, has no full machine, and dist reads Alice's web host
    assert!(matches!(
        light.net().need(&alice.key),
        Err(watane_net::NetError::NoFullMachine)
    ));
    light.follow(&alice.link()).unwrap();
    let fetched = light.fetch(&alice.published.info.reference).unwrap();
    assert_eq!(fetched.package.into_bytes(), alice.package_bytes());
}

/// The error and its sources, each followed by ` | `.
fn chain(error: &dyn std::error::Error) -> String {
    let mut chain = String::new();
    let mut at = Some(error);
    while let Some(e) = at {
        chain.push_str(&format!("{e} | "));
        at = e.source();
    }
    chain
}

#[test]
fn ac2_a_used_pairing_code_says_it_was_used() {
    let dirs: Vec<_> = (0..3).map(|_| tempfile::tempdir().unwrap()).collect();
    let anchor = app(dirs[0].path());
    let code = anchor.net().pair_code(Role::Full).unwrap().to_string();
    assert_eq!(join_yes(&anchor, dirs[1].path(), &code).code, 0);
    let done = run_end(dirs[2].path(), &["join", "--bind", LO, &code]);
    assert_eq!(done.code, 1, "{}", done.stderr);
    assert!(
        done.stderr
            .contains("the pairing code was used or has expired"),
        "{}",
        done.stderr
    );
}

#[test]
fn ac3_a_join_while_the_anchor_is_off_says_it_did_not_answer() {
    let dirs: Vec<_> = (0..2).map(|_| tempfile::tempdir().unwrap()).collect();
    let anchor = app(dirs[0].path());
    let code = anchor.net().pair_code(Role::Full).unwrap().to_string();
    anchor.net().set_options(Default::default()).unwrap();
    let done = run_end(dirs[1].path(), &["join", "--bind", LO, &code]);
    assert_eq!(done.code, 1, "{}", done.stderr);
    let short = key_of(&anchor).short();
    assert_eq!(done.stderr.matches(&short).count(), 1, "{}", done.stderr);
    assert!(!done.stderr.contains("no direct path"), "{}", done.stderr);
    assert!(done.stderr.contains("did not answer"), "{}", done.stderr);
}
