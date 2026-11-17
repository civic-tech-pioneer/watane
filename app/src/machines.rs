//! Settings-only access to own machines. Local API routes never use this module.
use crate::CmdError;
use crate::consent::{Answer, Broker, Reach, Request, Role};
use crate::core::Core;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use watane_node::net::{Event, NodeKey};
use watane_node::{Machine, Node};

pub(crate) type MachinesView = Value;
#[derive(Default)]
pub(crate) struct Machines {
    inner: Mutex<State>,
}
#[derive(Default)]
struct State {
    code: Option<Value>,
    joining: Option<Value>,
    peers: HashMap<NodeKey, Peer>,
    problems: Vec<Value>,
}
#[derive(Default)]
struct Peer {
    online: bool,
    last_seen: Option<u64>,
    last_pull: Option<u64>,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
pub(crate) fn role(role: Role) -> watane_node::net::Role {
    match role {
        Role::Full => watane_node::net::Role::Full,
        Role::Light => watane_node::net::Role::Light,
    }
}
fn role_text(role: watane_node::net::Role) -> &'static str {
    match role {
        watane_node::net::Role::Full => "full",
        watane_node::net::Role::Light => "light",
    }
}
fn unique_key(text: &str, keys: impl IntoIterator<Item = NodeKey>) -> Result<NodeKey, CmdError> {
    let mut matches = keys
        .into_iter()
        .filter(|k| k.short() == text || k.to_string() == text);
    let key = matches
        .next()
        .ok_or_else(|| CmdError::new("bad_input", "no machine matches this key"))?;
    if matches.next().is_some() {
        return Err(CmdError::new(
            "bad_input",
            "more than one machine matches this key",
        ));
    }
    Ok(key)
}
impl Machines {
    pub fn start(node: &Arc<Node>, broker: Arc<Broker>) -> Arc<Self> {
        let machines = Arc::new(Self::default());
        let events = node.net().events();
        let weak = Arc::downgrade(node);
        let state = Arc::clone(&machines);
        std::thread::spawn(move || {
            loop {
                let event = match events.recv_timeout(Duration::from_millis(250)) {
                    Ok(event) => event,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) if weak.strong_count() > 0 => {
                        continue;
                    }
                    Err(_) => break,
                };
                let Some(node) = weak.upgrade() else {
                    break;
                };
                if let Event::JoinAsked {
                    node: key,
                    role: asked_role,
                } = event
                {
                    let answer = broker.ask_blocking(Request::Join {
                        node: key.short(),
                        role: match asked_role {
                            watane_node::net::Role::Full => Role::Full,
                            watane_node::net::Role::Light => Role::Light,
                        },
                        text: watane_node::words::join_asked(&key, asked_role),
                    });
                    node.net()
                        .answer_join(&key, matches!(answer, Answer::Approve { .. }));
                } else {
                    state.event(event);
                }
            }
        });
        machines
    }
    fn event(&self, event: Event) {
        let mut state = self
            .inner
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        match event {
            Event::PeerUp(node) | Event::PeerDown(node) => {
                let peer = state.peers.entry(node).or_default();
                peer.online = matches!(event, Event::PeerUp(_));
                peer.last_seen = Some(now());
            }
            Event::Pulled { from, .. } => {
                state.peers.entry(from).or_default().last_pull = Some(now())
            }
            Event::Refused { node, ref cause } | Event::Unreachable { node, ref cause } => {
                let kind = if matches!(event, Event::Refused { .. }) {
                    "refused"
                } else {
                    "unreachable"
                };
                state.problems.insert(0,json!({"node":node.short(),"text":format!("{kind} {}: {cause}",node.short()),"at":now()}));
                state.problems.truncate(20);
            }
            _ => {}
        }
    }
    fn view(&self, node: &Node) -> MachinesView {
        let net = node.net();
        let me = net.node_key();
        let options = net.options();
        let list = net.devices();
        let state = self
            .inner
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let machines:Vec<_>=list.iter().flat_map(|l|l.devices.iter().map(|(key,role)| {
            let peer=state.peers.get(key);
            json!({"node":key.short(),"role":role_text(*role),"you":*key==me,"anchor":*key==l.anchor,
                "online":if *key==me {options.on} else {peer.is_some_and(|p|p.online)},
                "lastSeen":peer.and_then(|p|p.last_seen),"lastPull":peer.and_then(|p|p.last_pull)})
        })).collect();
        json!({"on":options.on,"node":me.short(),"machine":match node.machine(){Machine::Alone=>"alone",Machine::Anchor=>"anchor",Machine::Full=>"full",Machine::Light=>"light",Machine::Removed=>"removed"},
            "anchor":list.as_ref().map(|l|l.anchor.short()),"seq":list.as_ref().map(|l|l.seq),"expiresDays":list.as_ref().map_or(0,|l|l.expires_days),
            "machines":machines,"relays":options.relays.iter().map(ToString::to_string).collect::<Vec<_>>(),"dht":options.dht,
            "code":state.code,"joining":state.joining,"problems":state.problems})
    }
}
impl Core {
    pub fn machines_view(&self) -> MachinesView {
        self.machines.view(&self.node)
    }
    pub fn machines_on(&self, on: bool) -> Result<MachinesView, CmdError> {
        let mut options = self.node.net().options();
        options.on = on;
        self.node.net().set_options(options)?;
        Ok(self.machines_view())
    }
    pub fn pair_code(&self, asked_role: Role) -> Result<Value, CmdError> {
        let code = self.node.net().pair_code(role(asked_role))?;
        let code = json!({"code":code.to_string(),"expiresAt":now()+600_000});
        self.machines
            .inner
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .code = Some(code.clone());
        Ok(code)
    }
    pub fn machine_join(&self, text: &str, replace: bool) -> Result<MachinesView, CmdError> {
        let code: watane_node::net::PairCode = text
            .parse()
            .map_err(|e| CmdError::new("bad_input", crate::cause_chain(&e)))?;
        {
            let mut state = self
                .machines
                .inner
                .lock()
                .expect("lock poisoned: another thread panicked while holding it");
            if state.joining.is_some() {
                return Err(CmdError::new("bad_input", "a join is already in progress"));
            }
            state.joining = Some(
                json!({"node":self.node.net().node_key().short(),"anchor":code.anchor.short()}),
            )
        };
        let result = self.node.join(&code, replace);
        self.machines
            .inner
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .joining = None;
        result?;
        self.channels()?;
        Ok(self.machines_view())
    }
    fn machine_key(&self, text: &str) -> Result<NodeKey, CmdError> {
        unique_key(
            text,
            self.node
                .net()
                .devices()
                .into_iter()
                .flat_map(|l| l.devices.into_iter().map(|(k, _)| k)),
        )
    }
    pub fn machine_remove(&self, text: &str) -> Result<MachinesView, CmdError> {
        self.node.net().remove_device(&self.machine_key(text)?)?;
        Ok(self.machines_view())
    }
    pub fn machine_role(&self, text: &str, asked_role: Role) -> Result<MachinesView, CmdError> {
        self.node
            .net()
            .set_role(&self.machine_key(text)?, role(asked_role))?;
        Ok(self.machines_view())
    }
    pub fn machines_expiry(&self, days: u32) -> Result<MachinesView, CmdError> {
        self.node.net().set_expiry(days)?;
        Ok(self.machines_view())
    }
    pub fn relay(&self, url: &str, add: bool) -> Result<MachinesView, CmdError> {
        let parsed =
            tauri::Url::parse(url).map_err(|e| CmdError::new("bad_input", e.to_string()))?;
        if add
            && !matches!(
                self.broker.ask_blocking(Request::Reach {
                    what: Reach::Relay,
                    url: Some(url.into()),
                    text: watane_node::words::relay_add(url)
                }),
                Answer::Approve { .. }
            )
        {
            return Err(CmdError::new("refused", "You declined the relay."));
        }
        // Read after consent: another settings action may have changed the switches.
        let mut options = self.node.net().options();
        if add {
            if !options.relays.contains(&parsed) {
                options.relays.push(parsed);
            }
        } else {
            options.relays.retain(|r| r != &parsed);
        }
        self.node.net().set_options(options)?;
        Ok(self.machines_view())
    }
    pub fn dht_set(&self, on: bool) -> Result<MachinesView, CmdError> {
        if on
            && !matches!(
                self.broker.ask_blocking(Request::Reach {
                    what: Reach::Dht,
                    url: None,
                    text: watane_node::words::dht_on(&self.node.net().node_key())
                }),
                Answer::Approve { .. }
            )
        {
            return Err(CmdError::new("refused", "You declined the DHT."));
        }
        let mut options = self.node.net().options();
        options.dht = on;
        self.node.net().set_options(options)?;
        Ok(self.machines_view())
    }
    pub fn own_hints(&self) -> Value {
        json!(self.node.own_hints().iter().map(|hint|json!({"node":hint.node.short(),"seq":hint.head.map(|h|h.seq),"id":hint.head.map(|h|h.id.to_string()),"forked":hint.head.is_some_and(|h|h.forked)})).collect::<Vec<_>>())
    }
    pub fn identity_confirm_hint(&self, node: &str, seq: u64, id: &str) -> Result<(), CmdError> {
        let hint = self
            .node
            .own_hints()
            .into_iter()
            .find(|h| {
                h.node.short() == node
                    && h.head
                        .is_some_and(|h| !h.forked && h.seq == seq && h.id.to_string() == id)
            })
            .ok_or_else(|| CmdError::new("bad_input", "this machine's hint is gone or forked"))?;
        self.node.confirm_own_hint(&hint)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ac10_remove_and_role_require_a_unique_short_or_full_key() {
        let a = NodeKey::from_bytes([1; 32]);
        let mut bytes = [1; 32];
        bytes[31] = 2;
        let b = NodeKey::from_bytes(bytes);
        assert_eq!(a.short(), b.short());
        assert_eq!(unique_key("missing", [a]).unwrap_err().code, "bad_input");
        assert_eq!(
            unique_key(&a.short(), [a, b]).unwrap_err().code,
            "bad_input"
        );
        assert_eq!(unique_key(&a.to_string(), [a, b]).unwrap(), a);
        assert_eq!(unique_key(&a.short(), [a]).unwrap(), a);
    }
    #[test]
    fn ac6_s33_problems_preserve_cause_and_bound_newest_first() {
        let machines = Machines::default();
        let key = NodeKey::from_bytes([2; 32]);
        machines.event(Event::Refused {
            node: key,
            cause: "changed byte".into(),
        });
        assert_eq!(
            machines
                .inner
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .problems[0]["text"],
            format!("refused {}: changed byte", key.short())
        );
        for n in 0..25 {
            machines.event(Event::Unreachable {
                node: key,
                cause: format!("choose a relay or turn on the DHT: {n}"),
            });
        }
        let state = machines
            .inner
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        assert_eq!(state.problems.len(), 20);
        assert_eq!(
            state.problems[0]["text"],
            format!(
                "unreachable {}: choose a relay or turn on the DHT: 24",
                key.short()
            )
        );
    }
    #[test]
    fn ac4_peer_status_tracks_online_seen_and_pull() {
        let machines = Machines::default();
        let key = NodeKey::from_bytes([3; 32]);
        machines.event(Event::PeerUp(key));
        assert!(
            machines
                .inner
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .peers[&key]
                .online
        );
        machines.event(Event::PeerDown(key));
        let state = machines
            .inner
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        assert!(!state.peers[&key].online);
        assert!(state.peers[&key].last_seen.is_some());
    }
}

#[cfg(test)]
mod startup_tests {
    use super::*;
    struct Refuse;
    impl watane_dist::Confirm for Refuse {
        fn confirm(&self, _: &watane_dist::SignRequest) -> watane_dist::Decision {
            panic!("opening and viewing must not sign")
        }
    }
    #[test]
    fn ac1_new_node_stays_off_and_settings_copy_matches_node_words() {
        let home = tempfile::tempdir().unwrap();
        let node = Node::open(
            home.path(),
            Arc::new(Refuse),
            crate::node_options(Default::default()),
        )
        .unwrap();
        let view = Machines::default().view(&node);
        assert_eq!(view["on"], false);
        assert_eq!(view["machine"], "alone");
        assert_eq!(view["dht"], false);
        assert_eq!(view["machines"], json!([]));
        assert_eq!(view["relays"], json!([]));
        assert_eq!(view["code"], Value::Null);
        assert_eq!(view["joining"], Value::Null);
        assert!(node.net().addrs().is_empty());
        assert!(include_str!("../ui/index.html").contains(watane_node::words::net_on()));
        assert!(node.dist().identity().is_some());
    }
}
