//! `watane-net`: machines that connect directly (stage N1: the node key, the
//! pairing code text, the wire of `watane/1`, the signed form of the device
//! list). See `TECHNICAL.md`.

mod allow;
mod blobs;
mod cbor;
mod client;
mod devices;
mod error;
mod gossip;
mod key;
mod light;
mod pairing;
mod serve;
mod sync;
mod ticket;
mod wire;

pub use devices::DeviceList;
pub use error::{Code, NetError};
pub use key::NodeKey;
pub use ticket::{Hint, PairCode};
pub use watane_log::{Entry, EntryId, PublicKey};
pub use wire::{Hello, Limits};

use iroh::{
    Endpoint, NetReportConfig, RelayMode,
    endpoint::{PortmapperConfig, presets},
    protocol::Router,
};
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, OnceLock, mpsc},
};
use zeroize::Zeroizing;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Unknown,
    Trusted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Light,
    Full,
}

/// A BLAKE3 hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlobId(pub [u8; 32]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeadInfo {
    pub seq: u64,
    pub id: EntryId,
    pub forked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    Kept,
    Asked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ingested {
    Stored(u64),
    NotWanted,
    Compromised,
    Failed(String),
}

/// A search (TECHNICAL 4.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub text: Option<String>,
    pub publisher: Option<PublicKey>,
    /// `(mode, name)`; mode is `offers`, `uses` or `sends`.
    pub interface: Option<(String, String)>,
    pub origin: Option<(PublicKey, String)>,
    pub limit: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub channel: PublicKey,
    pub seq: u64,
    pub entry: EntryId,
}

/// What `net` tells its owner (TECHNICAL 3). N3 emits `Pulled`, `Refused`,
/// `PeerUp` and `PeerDown` beside N2's `JoinAsked` and `DevicesChanged`.
#[derive(Debug, Clone)]
pub enum Event {
    /// The app shows the short key and asks.
    JoinAsked {
        node: NodeKey,
        role: Role,
    },
    DevicesChanged(DeviceList),
    Pulled {
        channel: PublicKey,
        seq: u64,
        from: NodeKey,
    },
    /// A bad object (NET-F-24).
    Refused {
        node: NodeKey,
        cause: String,
    },
    Unreachable {
        node: NodeKey,
        cause: String,
    },
    PeerUp(NodeKey),
    PeerDown(NodeKey),
}

/// The holder: the code that holds the channels and packages (TECHNICAL 3.1).
/// `net` calls it only from blocking threads. Stage N1 calls `heads`,
/// `entries` and `fork`.
pub trait Holdings: Send + Sync + 'static {
    fn is_anchor(&self) -> bool;
    /// `None`: every channel held.
    fn heads(&self, channels: Option<&[PublicKey]>) -> Vec<(PublicKey, Option<HeadInfo>)>;
    fn entries(&self, channel: &PublicKey, from: u64, limit: usize, max_bytes: usize)
    -> Vec<Entry>;
    fn fork(&self, channel: &PublicKey) -> Option<(Entry, Entry)>;
    fn blob(&self, id: &BlobId) -> Option<PathBuf>;
    fn wanted_blobs(&self, role: Role) -> Vec<(BlobId, u64)>;
    fn ingest(&self, channel: &PublicKey, entries: Vec<Entry>, why: Why) -> Ingested;
    fn store_blob(&self, id: &BlobId, file: &Path) -> Result<(), String>;
    fn search(&self, query: &Query) -> Vec<Hit>;
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    /// false: no socket at all (NET-F-3).
    pub on: bool,
    /// Chosen by the participant. Not wired before N4.
    pub relays: Vec<url::Url>,
    /// Not wired before N4.
    pub dht: bool,
    /// `watane-node` only (NET-F-29). Not wired before N4.
    pub relay_server: Option<SocketAddr>,
    pub limits: Limits,
    /// The address to bind; default `0.0.0.0:0`.
    pub bind: Option<SocketAddr>,
}

/// What the handler and the client share. It holds no runtime, so the tasks
/// of the runtime can hold it.
pub(crate) struct Core {
    pub(crate) dir: PathBuf,
    pub(crate) seed: Zeroizing<[u8; 32]>,
    pub(crate) key: NodeKey,
    pub(crate) holdings: Arc<dyn Holdings>,
    options: Mutex<Options>,
    addrs: Mutex<HashMap<NodeKey, Vec<SocketAddr>>>,
    /// The newest verified device list and the last contact with its anchor.
    pub(crate) state: Mutex<pairing::DevState>,
    pub(crate) codes: Mutex<Vec<pairing::PendingCode>>,
    pub(crate) joins: Mutex<HashMap<NodeKey, tokio::sync::oneshot::Sender<bool>>>,
    subscribers: Mutex<Vec<mpsc::Sender<Event>>>,
    /// The budget of the unknown peers (4.2).
    pub(crate) pool: Arc<allow::Pool>,
    /// The running endpoint, for the calls a handler makes.
    pub(crate) endpoint: Mutex<Option<Endpoint>>,
    /// The kept connections, one per own machine (5.2).
    pub(crate) links: Mutex<HashMap<NodeKey, sync::Link>>,
    pub(crate) syncs: Mutex<HashMap<NodeKey, sync::Slot>>,
    pub(crate) dials: Mutex<HashMap<NodeKey, sync::Dial>>,
    /// Machines that sent a bad object, for the life of this `Net` (5.6).
    pub(crate) dropped: Mutex<HashSet<NodeKey>>,
    /// Where gossip looks for a node.
    lookup: Mutex<Option<iroh::address_lookup::memory::MemoryLookup>>,
    pub(crate) gossip: Mutex<Option<gossip::Hub>>,
    /// The channels pulled for queries in the current window (5.9).
    pub(crate) pulls: Mutex<light::Window>,
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Core {
    pub(crate) fn limits(&self) -> Limits {
        lock(&self.options).limits.clone()
    }
    pub(crate) fn emit(&self, event: Event) {
        lock(&self.subscribers).retain(|tx| tx.send(event.clone()).is_ok());
    }
    pub(crate) fn addrs_of(&self, node: &NodeKey) -> Vec<SocketAddr> {
        lock(&self.addrs).get(node).cloned().unwrap_or_default()
    }
    /// Add addresses of a node, at the front when `first`.
    pub(crate) fn remember(&self, node: &NodeKey, addrs: &[SocketAddr], first: bool) {
        let all = {
            let mut book = lock(&self.addrs);
            let list = book.entry(*node).or_default();
            for addr in addrs {
                if let Some(at) = list.iter().position(|a| a == addr) {
                    if first {
                        list.remove(at);
                    } else {
                        continue;
                    }
                }
                if first {
                    list.insert(0, *addr);
                } else {
                    list.push(*addr);
                }
            }
            list.clone()
        };
        if let Some(lookup) = lock(&self.lookup).as_ref()
            && let Ok(id) = iroh::EndpointId::from_bytes(node.as_bytes())
        {
            lookup.add_endpoint_info(iroh::EndpointAddr::from_parts(
                id,
                all.into_iter().map(iroh::TransportAddr::Ip),
            ));
        }
    }
}

struct Running {
    router: Router,
    bind: Option<SocketAddr>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

struct Shared {
    /// Made on the first turn-on, so that a home with net off has no
    /// runtime and no socket.
    runtime: OnceLock<tokio::runtime::Runtime>,
    running: Mutex<Option<Running>>,
}

/// One node. Cheap to clone. The API blocks: do not call it from inside a
/// tokio runtime.
#[derive(Clone)]
pub struct Net {
    core: Arc<Core>,
    shared: Arc<Shared>,
}

/// The builder of every endpoint of `net`: no relay, no address lookup, no
/// port mapping, no probes (AGENTS rule 6; relays and the DHT arrive in N4).
fn endpoint_builder(
    seed: &[u8; 32],
    bind: Option<SocketAddr>,
) -> Result<iroh::endpoint::Builder, String> {
    let bind = bind.unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], 0)));
    Endpoint::builder(presets::Minimal)
        .secret_key(iroh::SecretKey::from_bytes(seed))
        .relay_mode(RelayMode::Disabled)
        .clear_address_lookup()
        .portmapper_config(PortmapperConfig::Disabled)
        .net_report_config(NetReportConfig::minimal())
        .clear_ip_transports()
        .bind_addr(bind)
        .map_err(|e| e.to_string())
}

impl Net {
    pub fn open(
        home_net: &Path,
        holdings: Arc<dyn Holdings>,
        options: Options,
    ) -> Result<Net, NetError> {
        let seed = key::open(home_net)?;
        let net = Net {
            core: Arc::new(Core {
                dir: home_net.to_owned(),
                key: key::public_of(&seed),
                seed,
                holdings,
                options: Mutex::new(Options::default()),
                addrs: Mutex::new(HashMap::new()),
                state: Mutex::new(pairing::load(home_net)?),
                codes: Mutex::new(Vec::new()),
                joins: Mutex::new(HashMap::new()),
                subscribers: Mutex::new(Vec::new()),
                pool: Arc::new(allow::Pool::default()),
                endpoint: Mutex::new(None),
                links: Mutex::new(HashMap::new()),
                syncs: Mutex::new(HashMap::new()),
                dials: Mutex::new(HashMap::new()),
                dropped: Mutex::new(HashSet::new()),
                lookup: Mutex::new(None),
                gossip: Mutex::new(None),
                pulls: Mutex::new(light::Window::default()),
            }),
            shared: Arc::new(Shared {
                runtime: OnceLock::new(),
                running: Mutex::new(None),
            }),
        };
        net.set_options(options)?;
        Ok(net)
    }

    pub(crate) fn runtime(&self) -> Result<&tokio::runtime::Runtime, NetError> {
        if let Some(rt) = self.shared.runtime.get() {
            return Ok(rt);
        }
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("watane-net")
            .enable_all()
            .build()
            .map_err(|e| NetError::Io {
                path: self.core.dir.clone(),
                cause: e.to_string(),
            })?;
        Ok(self.shared.runtime.get_or_init(|| rt))
    }

    pub fn set_options(&self, options: Options) -> Result<(), NetError> {
        let mut running = lock(&self.shared.running);
        *lock(&self.core.options) = options.clone();
        if let Some(current) = running.as_ref()
            && options.on
            && current.bind == options.bind
        {
            return Ok(());
        }
        if let Some(current) = running.take() {
            *lock(&self.core.endpoint) = None;
            *lock(&self.core.gossip) = None;
            *lock(&self.core.lookup) = None;
            for task in &current.tasks {
                task.abort();
            }
            self.runtime()?
                .block_on(async { current.router.shutdown().await })
                .ok();
        }
        if options.on {
            let builder =
                endpoint_builder(&self.core.seed, options.bind).map_err(|cause| NetError::Io {
                    path: self.core.dir.clone(),
                    cause,
                })?;
            let core = self.core.clone();
            let blobs_dir = self.core.dir.join("blobs");
            let (router, tasks) = self
                .runtime()?
                .block_on(async move {
                    let endpoint = builder.bind().await.map_err(|e| e.to_string())?;
                    let lookup = iroh::address_lookup::memory::MemoryLookup::new();
                    if let Ok(services) = endpoint.address_lookup() {
                        services.add(lookup.clone());
                    }
                    *lock(&core.lookup) = Some(lookup);
                    let known = lock(&core.addrs).clone();
                    for (node, addrs) in known {
                        core.remember(&node, &addrs, false);
                    }
                    let gossip = iroh_gossip::Gossip::builder().spawn(endpoint.clone());
                    *lock(&core.gossip) = Some(gossip::Hub::new(gossip.clone()));
                    let (store, events, hook) = blobs::open(core.clone(), &blobs_dir).await?;
                    let blobs = iroh_blobs::BlobsProtocol::new(&store, Some(events));
                    let serve = serve::Serve { core: core.clone() };
                    let router = Router::builder(endpoint)
                        .accept(
                            wire::ALPN,
                            allow::Gate::new(core.clone(), serve, wire::ALPN),
                        )
                        .accept(
                            iroh_blobs::ALPN,
                            allow::Gate::new(core.clone(), blobs, iroh_blobs::ALPN),
                        )
                        .accept(
                            iroh_gossip::ALPN,
                            allow::Gate::new(core.clone(), gossip, iroh_gossip::ALPN),
                        )
                        .spawn();
                    *lock(&core.endpoint) = Some(router.endpoint().clone());
                    lock(&core.syncs).clear();
                    lock(&core.dials).clear();
                    let keeper = tokio::spawn(sync::keeper(core));
                    Ok::<_, String>((router, vec![hook, keeper]))
                })
                .map_err(|cause| NetError::Io {
                    path: self.core.dir.clone(),
                    cause,
                })?;
            *running = Some(Running {
                router,
                bind: options.bind,
                tasks,
            });
        }
        Ok(())
    }

    /// Stops all contact; the node key stays.
    pub fn close(self) {
        let mut options = lock(&self.core.options).clone();
        options.on = false;
        let _ = self.set_options(options);
    }

    pub fn node_key(&self) -> NodeKey {
        self.core.key
    }

    pub fn class_of(&self, node: &NodeKey) -> Class {
        self.core.class_of(node)
    }

    /// The bound local addresses; empty while off.
    pub fn addrs(&self) -> Vec<SocketAddr> {
        lock(&self.shared.running)
            .as_ref()
            .map(|r| r.router.endpoint().bound_sockets())
            .unwrap_or_default()
    }

    /// Where to dial a node (N4: the address book).
    pub fn add_addrs(&self, node: &NodeKey, addrs: &[SocketAddr]) {
        self.core.remember(node, addrs, false);
    }

    /// A new head here (after an own append or an ingest that raised a head):
    /// tell the other own machines, which read it within seconds (5.7).
    pub fn told(&self, channel: &PublicKey, seq: u64, id: &EntryId) {
        if let Some(runtime) = self.shared.runtime.get() {
            let message = gossip::encode_hint(channel, seq, id);
            runtime.spawn(gossip::broadcast(self.core.clone(), message));
        }
    }

    pub fn hello(&self, node: &NodeKey) -> Result<Hello, NetError> {
        client::hello(self, node)
    }

    pub fn heads_of(
        &self,
        node: &NodeKey,
        channels: &[PublicKey],
    ) -> Result<Vec<(PublicKey, Option<HeadInfo>)>, NetError> {
        client::heads(self, node, channels)
    }

    pub fn entries_of(
        &self,
        node: &NodeKey,
        channel: &PublicKey,
        from: u64,
        limit: u64,
    ) -> Result<Vec<Entry>, NetError> {
        client::entries(self, node, channel, from, limit)
    }

    pub fn fork_of(
        &self,
        node: &NodeKey,
        channel: &PublicKey,
    ) -> Result<Option<(Entry, Entry)>, NetError> {
        client::fork(self, node, channel)
    }

    /// The code an anchor gives to a machine that joins with `role` (5.5).
    pub fn pair_code(&self, role: Role) -> Result<PairCode, NetError> {
        self.core.new_code(role)
    }

    /// The participant's answer to `Event::JoinAsked`.
    pub fn answer_join(&self, node: &NodeKey, yes: bool) {
        self.core.answer_join(node, yes);
    }

    /// Join the anchor of `code`; `replace` takes the new anchor's list
    /// whatever this machine holds.
    pub fn join(&self, code: &PairCode, replace: bool) -> Result<DeviceList, NetError> {
        client::join(self, code, replace)
    }

    /// The newest verified device list, when this machine has one.
    pub fn devices(&self) -> Option<DeviceList> {
        self.core.devices()
    }

    pub fn remove_device(&self, node: &NodeKey) -> Result<DeviceList, NetError> {
        let node = *node;
        self.change(move |list| {
            let before = list.devices.len();
            list.devices.retain(|(k, _)| *k != node);
            if list.devices.len() == before {
                return Err(NetError::BadList {
                    cause: format!("node {} is not a device", node.short()),
                });
            }
            Ok(())
        })
    }

    pub fn set_role(&self, node: &NodeKey, role: Role) -> Result<DeviceList, NetError> {
        let node = *node;
        self.change(
            move |list| match list.devices.iter_mut().find(|(k, _)| *k == node) {
                Some(device) => {
                    device.1 = role;
                    Ok(())
                }
                None => Err(NetError::BadList {
                    cause: format!("node {} is not a device", node.short()),
                }),
            },
        )
    }

    /// Days; 0 for never.
    pub fn set_expiry(&self, days: u32) -> Result<DeviceList, NetError> {
        self.change(move |list| {
            list.expires_days = days;
            Ok(())
        })
    }

    /// Light: pull `channel` from a full machine with `Why::Asked` (5.9).
    pub fn need(&self, channel: &PublicKey) -> Result<(), NetError> {
        light::need(self, channel)
    }

    /// Fetch a package of `size` bytes from a full machine (5.9).
    pub fn need_blob(&self, id: &BlobId, size: u64) -> Result<(), NetError> {
        light::need_blob(self, id, size)
    }

    /// This node's own hits first, then those of a full machine (5.9).
    pub fn query(&self, query: &Query) -> Vec<(NodeKey, Vec<Hit>)> {
        light::query(self, query)
    }

    /// The head of an own channel at each reachable full machine: restore
    /// hints; nothing is ingested (5.11).
    pub fn own_heads(&self, channel: &PublicKey) -> Vec<(NodeKey, Option<HeadInfo>)> {
        light::own_heads(self, channel)
    }

    /// Every receiver gets every later event (node Q2).
    pub fn events(&self) -> mpsc::Receiver<Event> {
        let (tx, rx) = mpsc::channel();
        lock(&self.core.subscribers).push(tx);
        rx
    }

    /// Sign the next list and tell the other own machines.
    fn change(
        &self,
        f: impl FnOnce(&mut DeviceList) -> Result<(), NetError>,
    ) -> Result<DeviceList, NetError> {
        let (list, _) = self.core.sign_next(f)?;
        if let Some(runtime) = self.shared.runtime.get() {
            runtime.spawn(pairing::push(self.core.clone(), list.clone(), None));
        }
        Ok(list)
    }

    #[cfg(test)]
    pub(crate) fn endpoint(&self) -> Option<Endpoint> {
        lock(&self.shared.running)
            .as_ref()
            .map(|r| r.router.endpoint().clone())
    }
    pub(crate) fn core(&self) -> &Core {
        &self.core
    }
    pub(crate) fn core_arc(&self) -> Arc<Core> {
        self.core.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Nothing;
    impl Holdings for Nothing {
        fn is_anchor(&self) -> bool {
            false
        }
        fn heads(&self, _: Option<&[PublicKey]>) -> Vec<(PublicKey, Option<HeadInfo>)> {
            Vec::new()
        }
        fn entries(&self, _: &PublicKey, _: u64, _: usize, _: usize) -> Vec<Entry> {
            Vec::new()
        }
        fn fork(&self, _: &PublicKey) -> Option<(Entry, Entry)> {
            None
        }
        fn blob(&self, _: &BlobId) -> Option<PathBuf> {
            None
        }
        fn wanted_blobs(&self, _: Role) -> Vec<(BlobId, u64)> {
            Vec::new()
        }
        fn ingest(&self, _: &PublicKey, _: Vec<Entry>, _: Why) -> Ingested {
            Ingested::NotWanted
        }
        fn store_blob(&self, _: &BlobId, _: &Path) -> Result<(), String> {
            Ok(())
        }
        fn search(&self, _: &Query) -> Vec<Hit> {
            Vec::new()
        }
    }

    /// AC9: the endpoint has no relay and no address lookup.
    #[test]
    fn no_default_services() {
        let dir = tempfile::tempdir().unwrap();
        let net = Net::open(
            &dir.path().join("net"),
            Arc::new(Nothing),
            Options {
                on: true,
                bind: Some("127.0.0.1:0".parse().unwrap()),
                ..Options::default()
            },
        )
        .unwrap();
        let endpoint = net.endpoint().expect("on");
        // in memory only: ours (addresses given by the owner) and the one
        // iroh-gossip adds; neither reaches the network
        assert_eq!(endpoint.address_lookup().unwrap().len(), 2);
        assert_eq!(endpoint.addr().relay_urls().count(), 0);
        assert!(
            endpoint
                .bound_sockets()
                .iter()
                .all(|a| a.ip().is_loopback())
        );
        net.close();
    }
}
