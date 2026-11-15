//! `watane-net`: machines that connect directly (stage N1: the node key, the
//! pairing code text, the wire of `watane/1`, the signed form of the device
//! list). See `TECHNICAL.md`.

mod cbor;
mod client;
mod devices;
mod error;
mod key;
mod serve;
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
    collections::HashMap,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, OnceLock},
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
    /// N1 only: the nodes that `class_of` rates `Trusted`, standing in for the
    /// device list. N2 removes it.
    pub trusted: Vec<NodeKey>,
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
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Core {
    pub(crate) fn limits(&self) -> Limits {
        lock(&self.options).limits.clone()
    }
    pub(crate) fn class_of(&self, node: &NodeKey) -> Class {
        if *node == self.key || lock(&self.options).trusted.contains(node) {
            Class::Trusted
        } else {
            Class::Unknown
        }
    }
    pub(crate) fn addrs_of(&self, node: &NodeKey) -> Vec<SocketAddr> {
        lock(&self.addrs).get(node).cloned().unwrap_or_default()
    }
}

struct Running {
    router: Router,
    bind: Option<SocketAddr>,
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
            let router = self
                .runtime()?
                .block_on(async move {
                    let endpoint = builder.bind().await.map_err(|e| e.to_string())?;
                    Ok::<_, String>(
                        Router::builder(endpoint)
                            .accept(wire::ALPN, serve::Serve { core })
                            .spawn(),
                    )
                })
                .map_err(|cause| NetError::Io {
                    path: self.core.dir.clone(),
                    cause,
                })?;
            *running = Some(Running {
                router,
                bind: options.bind,
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
        let mut book = lock(&self.core.addrs);
        let list = book.entry(*node).or_default();
        for addr in addrs {
            if !list.contains(addr) {
                list.push(*addr);
            }
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

    pub(crate) fn endpoint(&self) -> Option<Endpoint> {
        lock(&self.shared.running)
            .as_ref()
            .map(|r| r.router.endpoint().clone())
    }
    pub(crate) fn core(&self) -> &Core {
        &self.core
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
        assert!(endpoint.address_lookup().unwrap().is_empty());
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
