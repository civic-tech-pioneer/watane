//! Reaching machines (TECHNICAL 5.10): which services the endpoint runs, the
//! switches and addresses kept in `book.json`, the cause of a failed dial and
//! the relay server.

use crate::NodeKey;
use crate::error::NetError;
use crate::pairing::{read_book, write_atomic};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Mutex;

/// At most this many addresses are kept of one machine.
pub(crate) const MAX_ADDRS: usize = 4;

/// What the endpoint runs, from the effective switches (5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Services {
    pub(crate) mdns: bool,
    pub(crate) relays: Vec<url::Url>,
    pub(crate) dht: bool,
}

/// mDNS only when on and bound beyond loopback; exactly the relays chosen;
/// the DHT exactly when chosen. Off runs none.
pub(crate) fn services(
    on: bool,
    relays: &[url::Url],
    dht: bool,
    bind: Option<SocketAddr>,
) -> Services {
    if !on {
        return Services {
            mdns: false,
            relays: Vec::new(),
            dht: false,
        };
    }
    let bind = bind.unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], 0)));
    Services {
        mdns: !bind.ip().is_loopback(),
        relays: relays.to_vec(),
        dht,
    }
}

/// Why a dial failed (NET-F-28, NET-F-32): the cause names what the
/// participant has not yet chosen, a relay, the DHT or both. With `known`
/// direct addresses of the machine that did not answer, it says so and does
/// not say there is no direct path.
pub(crate) fn no_path_cause(relays: bool, dht: bool, known: usize) -> String {
    if known == 0 {
        return match (relays, dht) {
            (false, false) => "no direct path; choose a relay or turn on the DHT",
            (true, false) => "no direct path and no answer through the relay; turn on the DHT",
            (false, true) => "no direct path and no answer through the DHT; choose a relay",
            (true, true) => "no direct path and no answer by the relay or by the DHT",
        }
        .to_owned();
    }
    let at = format!(
        "did not answer at its {known} known address{}",
        if known == 1 { "" } else { "es" }
    );
    match (relays, dht) {
        (false, false) => format!("{at}; choose a relay or turn on the DHT"),
        (true, false) => format!("{at} or through the relay; turn on the DHT"),
        (false, true) => format!("{at} or through the DHT; choose a relay"),
        (true, true) => format!("{at}, by the relay or by the DHT"),
    }
}

/// The switches of `book.json`: `on`, `relays`, `dht`.
pub(crate) struct Switches {
    pub(crate) on: bool,
    pub(crate) relays: Vec<url::Url>,
    pub(crate) dht: bool,
}

fn corrupt(dir: &Path, cause: impl Into<String>) -> NetError {
    NetError::Corrupt {
        path: dir.join("book.json"),
        cause: cause.into(),
    }
}

pub(crate) fn load_switches(dir: &Path) -> Result<Switches, NetError> {
    let book = read_book(dir)?;
    let flag = |name: &str| match &book[name] {
        serde_json::Value::Null => Ok(false),
        serde_json::Value::Bool(b) => Ok(*b),
        _ => Err(corrupt(dir, format!("{name} is not true or false"))),
    };
    let mut relays = Vec::new();
    match &book["relays"] {
        serde_json::Value::Null => {}
        serde_json::Value::Array(list) => {
            for item in list {
                let url = item
                    .as_str()
                    .and_then(|t| url::Url::parse(t).ok())
                    .ok_or_else(|| corrupt(dir, "a relay is not a URL"))?;
                relays.push(url);
            }
        }
        _ => return Err(corrupt(dir, "relays is not a list")),
    }
    Ok(Switches {
        on: flag("on")?,
        relays,
        dht: flag("dht")?,
    })
}

/// The stored addresses, of the machines in `keep` only.
pub(crate) fn load_addrs(
    dir: &Path,
    keep: impl Fn(&NodeKey) -> bool,
) -> Result<HashMap<NodeKey, Vec<SocketAddr>>, NetError> {
    let book = read_book(dir)?;
    let mut out = HashMap::new();
    let serde_json::Value::Object(map) = &book["addrs"] else {
        return match &book["addrs"] {
            serde_json::Value::Null => Ok(out),
            _ => Err(corrupt(dir, "addrs is not an object")),
        };
    };
    for (key, list) in map {
        let node: NodeKey = key
            .parse()
            .map_err(|_| corrupt(dir, "an addrs key is not a node key"))?;
        let serde_json::Value::Array(list) = list else {
            return Err(corrupt(dir, "an addrs value is not a list"));
        };
        let mut addrs = Vec::new();
        for item in list {
            let addr = item
                .as_str()
                .and_then(|t| t.parse::<SocketAddr>().ok())
                .ok_or_else(|| corrupt(dir, "an address is not a socket address"))?;
            if !addrs.contains(&addr) {
                addrs.push(addr);
            }
        }
        addrs.truncate(MAX_ADDRS);
        if keep(&node) && !addrs.is_empty() {
            out.insert(node, addrs);
        }
    }
    Ok(out)
}

/// Serializes every read-modify-write of `book.json` in this process.
static BOOK: Mutex<()> = Mutex::new(());

/// Change `book.json`, keeping the fields `change` leaves alone.
pub(crate) fn update_book(
    dir: &Path,
    change: impl FnOnce(&mut serde_json::Value),
) -> Result<(), NetError> {
    let _guard = BOOK
        .lock()
        .expect("lock poisoned: another thread panicked while holding it");
    let mut book = read_book(dir)?;
    let before = book.clone();
    change(&mut book);
    if book == before && dir.join("book.json").exists() {
        return Ok(());
    }
    let text = serde_json::to_vec_pretty(&book).map_err(|e| corrupt(dir, e.to_string()))?;
    write_atomic(dir, "book.json", &text)
}

pub(crate) fn write_switches(
    dir: &Path,
    on: bool,
    relays: &[url::Url],
    dht: bool,
) -> Result<(), NetError> {
    update_book(dir, |book| {
        book["on"] = on.into();
        book["relays"] = relays.iter().map(|u| u.as_str()).collect();
        book["dht"] = dht.into();
    })
}

pub(crate) fn write_addrs(
    dir: &Path,
    node: &NodeKey,
    addrs: &[SocketAddr],
) -> Result<(), NetError> {
    update_book(dir, |book| {
        if !book["addrs"].is_object() {
            book["addrs"] = serde_json::json!({});
        }
        book["addrs"][node.to_string()] = addrs.iter().map(|a| a.to_string()).collect();
    })
}

/// The URL of a relay server bound at `addr`.
pub(crate) fn relay_url_of(addr: SocketAddr) -> url::Url {
    url::Url::parse(&format!("http://{addr}/")).expect("an address makes a URL")
}

/// Who may use the relay: the machines of the newest device list on the
/// serving node (expiry included), nobody else (NET-F-29).
struct OwnMachines(std::sync::Arc<crate::Core>);

impl std::fmt::Debug for OwnMachines {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OwnMachines")
    }
}

impl iroh_relay::server::AccessControl for OwnMachines {
    async fn on_connect(
        &self,
        request: &iroh_relay::server::ClientRequest,
    ) -> iroh_relay::server::Access {
        let node = NodeKey::from_bytes(*request.endpoint_id().as_bytes());
        if self.0.class_of(&node) == crate::Class::Trusted {
            iroh_relay::server::Access::Allow
        } else {
            iroh_relay::server::Access::Deny {
                reason: Some("not an own machine".into()),
            }
        }
    }
}

/// Start the relay for the participant's own machines: HTTP, no TLS (the
/// relay forwards only QUIC that is encrypted end to end), no QUIC address
/// discovery, no metrics server, no ACME.
pub(crate) async fn start_relay(
    addr: SocketAddr,
    core: std::sync::Arc<crate::Core>,
) -> Result<iroh_relay::server::Server, String> {
    let mut relay = iroh_relay::server::RelayConfig::new(addr);
    relay.access = std::sync::Arc::new(OwnMachines(core));
    let mut config = iroh_relay::server::ServerConfig::default();
    config.relay = Some(relay);
    iroh_relay::server::Server::spawn(config)
        .await
        .map_err(|e| format!("the relay server could not start on {addr}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn relay(text: &str) -> url::Url {
        url::Url::parse(text).unwrap()
    }

    /// AC3: the services follow the switches and the bind address.
    #[test]
    fn ac3_services_by_configuration() {
        let any: SocketAddr = "0.0.0.0:0".parse().unwrap();
        let lo: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let r = [relay("http://127.0.0.1:9/")];
        let s = services(true, &[], false, Some(any));
        assert_eq!(
            s,
            Services {
                mdns: true,
                relays: vec![],
                dht: false
            }
        );
        assert!(services(true, &[], false, None).mdns, "default bind");
        assert!(!services(true, &[], false, Some(lo)).mdns, "loopback");
        let s = services(true, &r, true, Some(lo));
        assert_eq!((s.mdns, s.relays.as_slice(), s.dht), (false, &r[..], true));
        let s = services(true, &r, false, Some(any));
        assert_eq!((s.mdns, s.relays.as_slice(), s.dht), (true, &r[..], false));
        // off runs nothing, whatever is chosen
        let s = services(false, &r, true, Some(any));
        assert_eq!((s.mdns, s.relays.len(), s.dht), (false, 0, false));
    }

    /// AC7: the cause names only what the participant has not yet chosen.
    #[test]
    fn ac7_the_cause_follows_what_is_chosen() {
        assert_eq!(
            no_path_cause(false, false, 0),
            "no direct path; choose a relay or turn on the DHT"
        );
        assert_eq!(
            no_path_cause(true, false, 0),
            "no direct path and no answer through the relay; turn on the DHT"
        );
        assert_eq!(
            no_path_cause(false, true, 0),
            "no direct path and no answer through the DHT; choose a relay"
        );
        assert_eq!(
            no_path_cause(true, true, 0),
            "no direct path and no answer by the relay or by the DHT"
        );
    }

    /// S80 AC3: with known addresses the cause says they did not answer, and
    /// never "no direct path"; the advice comes once.
    #[test]
    fn ac3_known_addresses_that_did_not_answer() {
        assert_eq!(
            no_path_cause(false, false, 1),
            "did not answer at its 1 known address; choose a relay or turn on the DHT"
        );
        assert_eq!(
            no_path_cause(true, false, 2),
            "did not answer at its 2 known addresses or through the relay; turn on the DHT"
        );
        for (r, d) in [(false, true), (true, true)] {
            let text = no_path_cause(r, d, 3);
            assert!(text.starts_with("did not answer at its 3 known addresses"));
            assert!(!text.contains("no direct path"), "{text}");
        }
    }

    /// AC4: the stored addresses round trip, at most four per machine, only
    /// for the machines kept, and the other fields stay.
    #[test]
    fn ac4_the_book_keeps_addresses_and_other_fields() {
        let dir = tempfile::tempdir().unwrap();
        let a = crate::key::public_of(&[1; 32]);
        let b = crate::key::public_of(&[2; 32]);
        let addrs: Vec<SocketAddr> = (1..=6)
            .map(|n| format!("10.0.0.{n}:4433").parse().unwrap())
            .collect();
        update_book(dir.path(), |book| book["anchor_seen"] = 7.into()).unwrap();
        write_addrs(dir.path(), &a, &addrs).unwrap();
        write_addrs(dir.path(), &b, &addrs[..1]).unwrap();
        let all = load_addrs(dir.path(), |_| true).unwrap();
        assert_eq!(all[&a], addrs[..4]);
        assert_eq!(all[&b], addrs[..1]);
        let only_a = load_addrs(dir.path(), |n| *n == a).unwrap();
        assert_eq!(only_a.len(), 1);
        assert_eq!(read_book(dir.path()).unwrap()["anchor_seen"], 7);
    }

    /// AC2: the switches round trip and keep the other fields.
    #[test]
    fn ac2_the_book_keeps_the_switches() {
        let dir = tempfile::tempdir().unwrap();
        let fresh = load_switches(dir.path()).unwrap();
        assert!(!fresh.on && !fresh.dht && fresh.relays.is_empty());
        update_book(dir.path(), |book| book["anchor_seen"] = 9.into()).unwrap();
        let r = [relay("http://127.0.0.1:9/")];
        write_switches(dir.path(), true, &r, true).unwrap();
        let got = load_switches(dir.path()).unwrap();
        assert!(got.on && got.dht);
        assert_eq!(got.relays, r);
        assert_eq!(read_book(dir.path()).unwrap()["anchor_seen"], 9);
        std::fs::write(dir.path().join("book.json"), r#"{"relays":"x"}"#).unwrap();
        assert!(matches!(
            load_switches(dir.path()),
            Err(NetError::Corrupt { .. })
        ));
    }
}
