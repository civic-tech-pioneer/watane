//! `watane-node`: a home with no window and no identity that joins the participant's
//! machines (node TECHNICAL 6).
use std::{
    error::Error,
    io::{BufRead, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    process::ExitCode,
    task::Poll,
};
use watane_node::{
    Machine, Node, NodeError, Options,
    net::{DeviceList, Event, NetError, NodeKey, PairCode, Role},
    words,
};

const USAGE: &str = "usage:
  watane-node --home <dir> join [--replace] [--bind <addr:port>] <code>
  watane-node --home <dir> run [--relay <addr:port>] [--bind <addr:port>]
  watane-node --home <dir> status
  watane-node --home <dir> relay add <url> [--yes]
  watane-node --home <dir> relay remove <url>
  watane-node --home <dir> dht on [--yes]
  watane-node --home <dir> dht off
WATANE_NODE_HOME stands in for --home.";

#[derive(Debug, PartialEq, Eq)]
enum Cmd {
    Join {
        code: String,
        replace: bool,
        bind: Option<SocketAddr>,
    },
    Run {
        relay: Option<SocketAddr>,
        bind: Option<SocketAddr>,
    },
    Status,
    RelayAdd {
        url: String,
        yes: bool,
    },
    RelayRemove {
        url: String,
    },
    DhtOn {
        yes: bool,
    },
    DhtOff,
}

#[derive(Debug, PartialEq, Eq)]
struct Args {
    home: PathBuf,
    cmd: Cmd,
}

/// Parses the arguments after the program name; `env_home` is `WATANE_NODE_HOME`.
fn parse(args: &[String], env_home: Option<String>) -> Result<Args, String> {
    let mut home = None;
    let mut relay = None;
    let mut bind = None;
    let (mut yes, mut replace) = (false, false);
    let mut words = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--home" => {
                home = Some(it.next().ok_or("--home needs a directory")?.clone());
            }
            "--relay" => {
                let text = it.next().ok_or("--relay needs addr:port")?;
                let addr: SocketAddr = text
                    .parse()
                    .map_err(|_| format!("--relay needs addr:port, not {text}"))?;
                if addr.port() == 0 {
                    return Err(
                        "--relay needs a fixed port: your other machines store its URL".into(),
                    );
                }
                relay = Some(addr);
            }
            "--bind" => {
                let text = it.next().ok_or("--bind needs addr:port")?;
                bind = Some(
                    text.parse::<SocketAddr>()
                        .map_err(|_| format!("--bind needs addr:port, not {text}"))?,
                );
            }
            "--yes" => yes = true,
            "--replace" => replace = true,
            flag if flag.starts_with("--") => return Err(format!("unknown flag {flag}")),
            word => words.push(word.to_owned()),
        }
    }
    let home = home
        .or(env_home.filter(|h| !h.is_empty()))
        .ok_or("--home <dir> is required (or WATANE_NODE_HOME)")?;
    let allow = |ok: bool, flag: &str| {
        if ok {
            Ok(())
        } else {
            Err(format!("{flag} does not belong to this command"))
        }
    };
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    let cmd = match words.as_slice() {
        ["join", code] => {
            allow(!yes && relay.is_none(), "--yes and --relay")?;
            Cmd::Join {
                bind,
                code: (*code).to_owned(),
                replace,
            }
        }
        ["run"] => {
            allow(!yes && !replace, "--yes and --replace")?;
            Cmd::Run { relay, bind }
        }
        ["status"] => {
            allow(
                !yes && !replace && relay.is_none() && bind.is_none(),
                "a flag",
            )?;
            Cmd::Status
        }
        ["relay", "add", url] => {
            allow(
                !replace && relay.is_none() && bind.is_none(),
                "--replace, --relay and --bind",
            )?;
            Cmd::RelayAdd {
                url: (*url).to_owned(),
                yes,
            }
        }
        ["relay", "remove", url] => {
            allow(
                !yes && !replace && relay.is_none() && bind.is_none(),
                "a flag",
            )?;
            Cmd::RelayRemove {
                url: (*url).to_owned(),
            }
        }
        ["dht", "on"] => {
            allow(
                !replace && relay.is_none() && bind.is_none(),
                "--replace, --relay and --bind",
            )?;
            Cmd::DhtOn { yes }
        }
        ["dht", "off"] => {
            allow(
                !yes && !replace && relay.is_none() && bind.is_none(),
                "a flag",
            )?;
            Cmd::DhtOff
        }
        [] => return Err("no command".into()),
        other => return Err(format!("not a command: {}", other.join(" "))),
    };
    if replace && !matches!(cmd, Cmd::Join { .. }) {
        return Err("--replace belongs to join".into());
    }
    Ok(Args {
        home: home.into(),
        cmd,
    })
}

fn role_text(role: Role) -> &'static str {
    match role {
        Role::Full => "full",
        Role::Light => "light",
    }
}

fn machines(list: &DeviceList) -> String {
    list.devices
        .iter()
        .map(|(key, role)| format!("{} {}", key.short(), role_text(*role)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The line of `run` for an event (6.2); `None` for an event that prints nothing.
fn line(event: &Event) -> Option<String> {
    Some(match event {
        Event::PeerUp(n) => format!("up {}", n.short()),
        Event::PeerDown(n) => format!("down {}", n.short()),
        Event::Pulled { channel, seq, from } => {
            format!(
                "pulled {} to {seq} from {}",
                channel.fingerprint(),
                from.short()
            )
        }
        Event::Refused { node, cause } => format!("refused {}: {cause}", node.short()),
        Event::Unreachable { node, cause } => format!("unreachable {}: {cause}", node.short()),
        Event::DevicesChanged(list) => format!("machines {}: {}", list.seq, machines(list)),
        Event::JoinAsked { .. } => return None,
    })
}

/// The whole cause chain, each cause once (as app 13 does).
fn chain(error: &dyn Error) -> String {
    let mut text = error.to_string();
    let mut next = error.source();
    while let Some(cause) = next {
        let cause_text = cause.to_string();
        if !text.contains(&cause_text) {
            text = format!("{text}: {cause_text}");
        }
        next = cause.source();
    }
    text
}

enum Fail {
    Usage(String),
    Failed(String),
}

impl From<NodeError> for Fail {
    fn from(e: NodeError) -> Self {
        match e {
            NodeError::Net(NetError::Declined) => Fail::Failed("declined".into()),
            e => Fail::Failed(chain(&e)),
        }
    }
}
impl From<NetError> for Fail {
    fn from(e: NetError) -> Self {
        NodeError::from(e).into()
    }
}

fn failed(text: impl Into<String>) -> Fail {
    Fail::Failed(text.into())
}

/// What `book.json` keeps of the switches that this program changes.
struct Switches {
    relays: Vec<url::Url>,
    dht: bool,
}

fn switches(home: &Path) -> Result<Switches, Fail> {
    let path = home.join("net/book.json");
    let bad = || failed(format!("{} is not a book of switches", path.display()));
    let book: serde_json::Value = match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| bad())?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => serde_json::Value::Null,
        Err(e) => return Err(failed(format!("{}: {e}", path.display()))),
    };
    let relays = book["relays"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|v| {
                    v.as_str()
                        .and_then(|t| url::Url::parse(t).ok())
                        .ok_or_else(bad)
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(Switches {
        relays,
        dht: book["dht"].as_bool().unwrap_or(false),
    })
}

/// A node home with net off: no command but `join` and `run` makes contact (Q1).
fn open(home: &Path) -> Result<Node, Fail> {
    std::fs::create_dir_all(home).map_err(|e| failed(format!("{}: {e}", home.display())))?;
    Ok(Node::open_keyless(home, Options::default())?)
}

fn set(
    node: &Node,
    home: &Path,
    on: bool,
    relay: Option<SocketAddr>,
    bind: Option<SocketAddr>,
) -> Result<(), Fail> {
    let s = switches(home)?;
    Ok(node.net().set_options(watane_node::net::Options {
        on,
        relays: s.relays,
        dht: s.dht,
        relay_server: relay,
        bind,
        ..Default::default()
    })?)
}

fn out(text: &str) {
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{text}");
    let _ = stdout.flush();
}

fn own_role(node: &Node) -> &'static str {
    match node.machine() {
        Machine::Light => "light",
        _ => "full",
    }
}

enum Told {
    Signal,
    Joined(Result<DeviceList, NodeError>),
}

fn join(home: &Path, code: &str, replace: bool, bind: Option<SocketAddr>) -> Result<(), Fail> {
    let code: PairCode = code
        .parse()
        .map_err(|e: <PairCode as std::str::FromStr>::Err| failed(chain(&e)))?;
    let node = open(home)?;
    let wait = signals()?;
    set(&node, home, true, None, bind)?;
    out(&format!("this machine: {}", node.net().node_key().short()));
    out("waiting for your answer on the anchor");
    let (tx, rx) = std::sync::mpsc::channel();
    let signalled = tx.clone();
    std::thread::spawn(move || {
        wait();
        let _ = signalled.send(Told::Signal);
    });
    let result = std::thread::scope(|scope| {
        scope.spawn(|| {
            let _ = tx.send(Told::Joined(node.join(&code, replace)));
        });
        match rx.recv() {
            Ok(Told::Joined(result)) => result,
            _ => {
                // the join thread still waits for the anchor: leave without it
                let _ = set(&node, home, false, None, None);
                eprintln!("watane-node: interrupted");
                std::process::exit(1);
            }
        }
    });
    let _ = set(&node, home, false, None, None);
    let list = result?;
    out(&format!(
        "joined as {}; machines: {}",
        own_role(&node),
        machines(&list)
    ));
    node.close();
    Ok(())
}

/// Waits for SIGINT or SIGTERM on a runtime that does nothing else. The signals are
/// registered when this returns, so one that comes later is not lost.
fn signals() -> Result<impl FnOnce(), Fail> {
    use tokio::signal::unix::{SignalKind, signal};
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| failed(e.to_string()))?;
    let (mut int, mut term) = rt
        .block_on(async {
            Ok::<_, std::io::Error>((
                signal(SignalKind::interrupt())?,
                signal(SignalKind::terminate())?,
            ))
        })
        .map_err(|e| failed(e.to_string()))?;
    Ok(move || {
        rt.block_on(std::future::poll_fn(|cx| {
            let a = int.poll_recv(cx).is_ready();
            let b = term.poll_recv(cx).is_ready();
            if a || b {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        }));
    })
}

fn run(home: &Path, relay: Option<SocketAddr>, bind: Option<SocketAddr>) -> Result<(), Fail> {
    let node = open(home)?;
    match node.machine() {
        Machine::Alone => {
            return Err(failed("not paired: run watane-node join <code> first"));
        }
        Machine::Removed => {
            return Err(failed(
                "this machine was removed from your machines: run watane-node join --replace <code>",
            ));
        }
        _ => {}
    }
    let wait = signals()?;
    let events = node.net().events();
    set(&node, home, true, relay, bind)?;
    std::thread::spawn(move || {
        while let Ok(event) = events.recv() {
            if let Some(text) = line(&event) {
                out(&text);
            }
        }
    });
    out(&format!(
        "running as {}, node {}",
        own_role(&node),
        node.net().node_key().short()
    ));
    if let Some(addr) = relay {
        let Some(url) = node.net().relay_url() else {
            let _ = set(&node, home, false, None, None);
            return Err(failed(format!("the relay on {addr} did not start")));
        };
        out(&format!(
            "relay on {addr}; add {url} to the relays of your other machines"
        ));
    }
    wait();
    let _ = set(&node, home, false, None, None);
    node.close();
    Ok(())
}

fn status(home: &Path) -> Result<(), Fail> {
    let node = open(home)?;
    let s = switches(home)?;
    let key = node.net().node_key();
    out(&format!("node {key}"));
    out(&format!(
        "machine {}",
        match node.machine() {
            Machine::Alone => "alone",
            Machine::Anchor => "anchor",
            Machine::Full => "full",
            Machine::Light => "light",
            Machine::Removed => "removed",
        }
    ));
    match node.net().devices() {
        None => {
            out("anchor none");
            out("devices none");
        }
        Some(list) => {
            out(&format!("anchor {}", list.anchor.short()));
            let expires = match list.expires_days {
                0 => "never".to_owned(),
                n => format!("after {n} days"),
            };
            out(&format!(
                "devices seq {}, expires {expires}: {}",
                list.seq,
                machines(&list)
            ));
        }
    }
    out(&format!(
        "relays {}",
        if s.relays.is_empty() {
            "none".to_owned()
        } else {
            s.relays
                .iter()
                .map(url::Url::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        }
    ));
    out(&format!("dht {}", if s.dht { "on" } else { "off" }));
    let dist = node.dist();
    out(&format!(
        "channels held {}",
        dist.heads(None).map_err(|e| failed(chain(&e)))?.len()
    ));
    out(&format!(
        "packages missing {}",
        dist.missing_packages()
            .map_err(|e| failed(chain(&e)))?
            .len()
    ));
    node.close();
    Ok(())
}

/// A consent text, then one line from standard input; only `y` or `yes` counts.
fn consent(text: &str, yes: bool) -> bool {
    out(text);
    if yes {
        return true;
    }
    let _ = write!(std::io::stdout(), "Turn this on? [y/N] ");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    if std::io::stdin().lock().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim().to_lowercase().as_str(), "y" | "yes")
}

fn store(home: &Path, relays: Option<Vec<url::Url>>, dht: Option<bool>) -> Result<(), Fail> {
    let node = open(home)?;
    let s = switches(home)?;
    node.net().set_options(watane_node::net::Options {
        on: false,
        relays: relays.unwrap_or(s.relays),
        dht: dht.unwrap_or(s.dht),
        ..Default::default()
    })?;
    node.close();
    Ok(())
}

fn relay_url(text: &str) -> Result<url::Url, Fail> {
    url::Url::parse(text).map_err(|e| failed(format!("{text} is not a URL: {e}")))
}

fn dispatch(args: Args) -> Result<(), Fail> {
    let home = args.home.as_path();
    match args.cmd {
        Cmd::Join {
            code,
            replace,
            bind,
        } => join(home, &code, replace, bind),
        Cmd::Run { relay, bind } => run(home, relay, bind),
        Cmd::Status => status(home),
        Cmd::RelayAdd { url, yes } => {
            let url = relay_url(&url)?;
            open(home)?.close(); // a host home is refused before any question
            let mut relays = switches(home)?.relays;
            if !consent(&words::relay_add(url.as_str()), yes) {
                return Err(failed("not changed"));
            }
            if !relays.contains(&url) {
                relays.push(url);
            }
            store(home, Some(relays), None)
        }
        Cmd::RelayRemove { url } => {
            let url = relay_url(&url)?;
            let mut relays = switches(home)?.relays;
            relays.retain(|r| *r != url);
            store(home, Some(relays), None)
        }
        Cmd::DhtOn { yes } => {
            let key: NodeKey = open(home)?.net().node_key();
            if !consent(&words::dht_on(&key), yes) {
                return Err(failed("not changed"));
            }
            store(home, None, Some(true))
        }
        Cmd::DhtOff => store(home, None, Some(false)),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = parse(&args, std::env::var("WATANE_NODE_HOME").ok())
        .map_err(Fail::Usage)
        .and_then(dispatch);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(Fail::Usage(text)) => {
            eprintln!("watane-node: {text}\n{USAGE}");
            ExitCode::from(2)
        }
        Err(Fail::Failed(text)) => {
            eprintln!("watane-node: {text}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use watane_node::net::PublicKey;

    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }
    fn ok(text: &str) -> Args {
        parse(&args(text), None).unwrap()
    }
    fn addr() -> SocketAddr {
        "127.0.0.1:7000".parse().unwrap()
    }

    #[test]
    fn ac7_each_command_parses() {
        let cmd = |text| ok(text).cmd;
        assert_eq!(
            cmd("--home h join abc"),
            Cmd::Join {
                code: "abc".into(),
                replace: false,
                bind: None
            }
        );
        assert_eq!(
            cmd("--home h join --replace abc"),
            Cmd::Join {
                code: "abc".into(),
                replace: true,
                bind: None
            }
        );
        assert_eq!(
            cmd("--home h run"),
            Cmd::Run {
                relay: None,
                bind: None
            }
        );
        assert_eq!(
            cmd("--home h run --relay 127.0.0.1:7000"),
            Cmd::Run {
                relay: Some(addr()),
                bind: None
            }
        );
        assert_eq!(
            cmd("--home h run --bind 127.0.0.1:0"),
            Cmd::Run {
                relay: None,
                bind: Some("127.0.0.1:0".parse().unwrap())
            }
        );
        assert_eq!(
            cmd("--home h join --bind 127.0.0.1:0 abc"),
            Cmd::Join {
                code: "abc".into(),
                replace: false,
                bind: Some("127.0.0.1:0".parse().unwrap())
            }
        );
        assert_eq!(cmd("--home h status"), Cmd::Status);
        assert_eq!(
            cmd("--home h relay add http://r/ --yes"),
            Cmd::RelayAdd {
                url: "http://r/".into(),
                yes: true
            }
        );
        assert_eq!(
            cmd("--home h relay add http://r/"),
            Cmd::RelayAdd {
                url: "http://r/".into(),
                yes: false
            }
        );
        assert_eq!(
            cmd("--home h relay remove http://r/"),
            Cmd::RelayRemove {
                url: "http://r/".into()
            }
        );
        assert_eq!(cmd("--home h dht on"), Cmd::DhtOn { yes: false });
        assert_eq!(cmd("--home h dht on --yes"), Cmd::DhtOn { yes: true });
        assert_eq!(cmd("--home h dht off"), Cmd::DhtOff);
        assert_eq!(ok("--home h status").home, PathBuf::from("h"));
    }

    #[test]
    fn ac7_the_home_comes_from_the_flag_or_the_environment() {
        let env = Some("e".to_owned());
        assert_eq!(
            parse(&args("status"), env.clone()).unwrap().home,
            PathBuf::from("e")
        );
        assert_eq!(
            parse(&args("--home h status"), env).unwrap().home,
            PathBuf::from("h")
        );
        assert!(parse(&args("status"), None).is_err());
        assert!(parse(&args("status"), Some(String::new())).is_err());
    }

    #[test]
    fn ac7_bad_arguments_are_refused() {
        for bad in [
            "--home h run --relay 127.0.0.1:0",
            "--home h run --relay nonsense",
            "--home h run --relay",
            "--home h run --bind",
            "--home h run --bind nonsense",
            "--home h status --bind 127.0.0.1:0",
            "--home h dht off --bind 127.0.0.1:0",
            "--home h status --bogus",
            "--home h",
            "--home h join",
            "--home h dht",
            "--home h dht maybe",
            "--home h relay add",
            "--home h relay remove http://r/ --yes",
            "--home h run --yes",
            "--home h dht off --yes",
            "--home h status --replace",
            "--home",
        ] {
            assert!(parse(&args(bad), None).is_err(), "{bad}");
        }
    }

    #[test]
    fn ac7_each_event_has_its_line() {
        let key = |n: u8| NodeKey::from_bytes([n; 32]);
        let channel = PublicKey::from_bytes(&[3; 32]).unwrap();
        let list = DeviceList {
            anchor: key(1),
            seq: 4,
            time: 0,
            expires_days: 0,
            devices: vec![(key(1), Role::Full), (key(2), Role::Light)],
        };
        let short = |n| key(n).short();
        assert_eq!(
            line(&Event::PeerUp(key(2))),
            Some(format!("up {}", short(2)))
        );
        assert_eq!(
            line(&Event::PeerDown(key(2))),
            Some(format!("down {}", short(2)))
        );
        assert_eq!(
            line(&Event::Pulled {
                channel,
                seq: 7,
                from: key(2)
            }),
            Some(format!(
                "pulled {} to 7 from {}",
                channel.fingerprint(),
                short(2)
            ))
        );
        assert_eq!(
            line(&Event::Refused {
                node: key(2),
                cause: "bad byte".into()
            }),
            Some(format!("refused {}: bad byte", short(2)))
        );
        assert_eq!(
            line(&Event::Unreachable {
                node: key(2),
                cause: "no path".into()
            }),
            Some(format!("unreachable {}: no path", short(2)))
        );
        assert_eq!(
            line(&Event::DevicesChanged(list)),
            Some(format!("machines 4: {} full, {} light", short(1), short(2)))
        );
        assert_eq!(
            line(&Event::JoinAsked {
                node: key(2),
                role: Role::Full
            }),
            None
        );
    }

    #[test]
    fn a_cause_is_printed_once() {
        #[derive(Debug, thiserror::Error)]
        #[error("outer: inner")]
        struct Outer(#[source] Inner);
        #[derive(Debug, thiserror::Error)]
        #[error("inner")]
        struct Inner;
        assert_eq!(chain(&Outer(Inner)), "outer: inner");
    }
}
