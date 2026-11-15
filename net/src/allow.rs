//! Allowances and budgets (TECHNICAL 4.2, 5.4): what each peer may use, and
//! `Gate`, the wrapper every protocol handler sits in. Unknown peers share
//! one pool: connections, streams and a byte bucket.

use crate::{Class, Core, key::NodeKey, lock, wire};
use iroh::{
    endpoint::Connection,
    protocol::{AcceptError, ProtocolHandler},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const POLL: Duration = Duration::from_millis(100);

/// Does a peer of this class have any allowance for the protocol? An
/// unknown peer has `watane/1` (its ops are limited in `serve`), and none for
/// iroh-blobs or iroh-gossip or anything else; a trusted peer has all three.
pub(crate) fn allowed(class: Class, alpn: &[u8]) -> bool {
    match class {
        Class::Trusted => true,
        Class::Unknown => alpn == wire::ALPN,
    }
}

struct Conn {
    peer: NodeKey,
    /// UDP bytes in and out already charged.
    charged: u64,
}

struct State {
    conns: HashMap<usize, Conn>,
    streams: HashMap<NodeKey, usize>,
    /// The byte bucket: it may go into debt, down to one second's worth.
    tokens: f64,
    last: Instant,
}

/// The budget of all unknown peers together.
pub(crate) struct Pool {
    state: Mutex<State>,
}

impl Default for Pool {
    fn default() -> Self {
        Pool {
            state: Mutex::new(State {
                conns: HashMap::new(),
                streams: HashMap::new(),
                tokens: f64::INFINITY,
                last: Instant::now(),
            }),
        }
    }
}

impl State {
    fn refill(&mut self, per_sec: u64) {
        let cap = per_sec as f64;
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + dt * cap).min(cap);
    }
}

pub(crate) struct ConnGuard {
    pool: Arc<Pool>,
    id: usize,
}
impl Drop for ConnGuard {
    fn drop(&mut self) {
        lock(&self.pool.state).conns.remove(&self.id);
    }
}

pub(crate) struct StreamGuard {
    pool: Arc<Pool>,
    peer: NodeKey,
}
impl Drop for StreamGuard {
    fn drop(&mut self) {
        let mut state = lock(&self.pool.state);
        if let Some(n) = state.streams.get_mut(&self.peer) {
            *n -= 1;
            if *n == 0 {
                state.streams.remove(&self.peer);
            }
        }
    }
}

fn bytes_of(connection: &Connection) -> u64 {
    let stats = connection.stats();
    stats.udp_tx.bytes + stats.udp_rx.bytes
}

impl Pool {
    /// Count a connection of an unknown peer; `None` when it is over a
    /// connection budget. Bytes before now (the handshake) are not charged.
    pub(crate) fn admit(
        self: &Arc<Self>,
        peer: NodeKey,
        connection: &Connection,
        limits: &crate::Limits,
    ) -> Option<ConnGuard> {
        let mut state = lock(&self.state);
        let mine = state.conns.values().filter(|c| c.peer == peer).count();
        if mine >= limits.unknown_conns_per_peer || state.conns.len() >= limits.unknown_conns {
            return None;
        }
        let id = connection.stable_id();
        state.conns.insert(
            id,
            Conn {
                peer,
                charged: bytes_of(connection),
            },
        );
        Some(ConnGuard {
            pool: self.clone(),
            id,
        })
    }

    /// Charge what a counted connection moved since the last poll.
    pub(crate) fn poll(&self, connection: &Connection, limits: &crate::Limits) {
        let total = bytes_of(connection);
        let mut state = lock(&self.state);
        let Some(conn) = state.conns.get_mut(&connection.stable_id()) else {
            return;
        };
        let delta = total.saturating_sub(conn.charged);
        conn.charged = total;
        state.refill(limits.unknown_bytes_per_sec);
        let floor = -(limits.unknown_bytes_per_sec as f64);
        state.tokens = (state.tokens - delta as f64).max(floor);
    }

    /// The bucket is empty.
    pub(crate) fn over_bytes(&self, limits: &crate::Limits) -> bool {
        let mut state = lock(&self.state);
        state.refill(limits.unknown_bytes_per_sec);
        state.tokens <= 0.0
    }

    /// Count a stream of an unknown peer; `None` when it is over the stream
    /// budget or the byte budget (the caller answers `over_budget`).
    pub(crate) fn stream(
        self: &Arc<Self>,
        peer: NodeKey,
        limits: &crate::Limits,
    ) -> Option<StreamGuard> {
        if self.over_bytes(limits) {
            return None;
        }
        let mut state = lock(&self.state);
        let open = state.streams.entry(peer).or_insert(0);
        if *open >= limits.unknown_streams_per_peer {
            return None;
        }
        *open += 1;
        Some(StreamGuard {
            pool: self.clone(),
            peer,
        })
    }
}

/// The wrapper of a protocol handler: allowance and connection budget
/// before the inner `accept`, and the byte budget while it runs.
pub(crate) struct Gate<P> {
    core: Arc<Core>,
    inner: P,
    alpn: &'static [u8],
}

impl<P> Gate<P> {
    pub(crate) fn new(core: Arc<Core>, inner: P, alpn: &'static [u8]) -> Self {
        Gate { core, inner, alpn }
    }
}

impl<P> std::fmt::Debug for Gate<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Gate")
    }
}

/// Poll the connection's bytes; close it when the budget is spent and the
/// protocol cannot say `over_budget`. Never returns otherwise.
async fn watch(core: &Core, connection: &Connection, close_when_spent: bool) {
    loop {
        tokio::time::sleep(POLL).await;
        let limits = core.limits();
        core.pool.poll(connection, &limits);
        if close_when_spent && core.pool.over_bytes(&limits) {
            connection.close(429u32.into(), b"over budget");
            return;
        }
    }
}

impl<P: ProtocolHandler> ProtocolHandler for Gate<P> {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        // the key comes from the handshake (NET-F-4)
        let peer = NodeKey::from_bytes(*connection.remote_id().as_bytes());
        self.core.seen(&peer);
        let class = self.core.class_of(&peer);
        if !allowed(class, self.alpn) {
            connection.close(403u32.into(), b"not allowed");
            return Ok(());
        }
        if class == Class::Trusted {
            return self.inner.accept(connection).await;
        }
        let limits = self.core.limits();
        let Some(guard) = self.core.pool.admit(peer, &connection, &limits) else {
            connection.close(429u32.into(), b"over budget");
            return Ok(());
        };
        let can_answer = self.alpn == wire::ALPN;
        let result = tokio::select! {
            r = self.inner.accept(connection.clone()) => r,
            () = watch(&self.core, &connection, !can_answer) => Ok(()),
        };
        self.core.pool.poll(&connection, &self.core.limits());
        drop(guard);
        result
    }

    async fn shutdown(&self) {
        self.inner.shutdown().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lo() -> std::net::SocketAddr {
        "127.0.0.1:0".parse().unwrap()
    }

    fn lo_services() -> crate::reach::Services {
        crate::reach::services(true, &[], false, Some(lo()))
    }
    use crate::{
        BlobId, DeviceList, HeadInfo, Hit, Holdings, Ingested, Net, Options, Query, Role, Why,
    };
    use iroh::{EndpointAddr, TransportAddr, protocol::Router};
    use std::{
        path::{Path, PathBuf},
        sync::atomic::{AtomicUsize, Ordering},
    };
    use watane_log::{Entry, PublicKey};

    struct Anchor;
    impl Holdings for Anchor {
        fn is_anchor(&self) -> bool {
            true
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

    #[derive(Debug)]
    struct Dummy(Arc<AtomicUsize>);
    impl ProtocolHandler for Dummy {
        async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            connection.closed().await;
            Ok(())
        }
    }

    const TEST_ALPN: &[u8] = b"watane/test";

    /// AC9: an unknown peer is closed before the inner handler of another
    /// protocol is called; a trusted one reaches it.
    #[tokio::test(flavor = "multi_thread")]
    async fn ac9_the_gate_closes_an_unknown_peer_before_the_inner_handler() {
        let dir = tempfile::tempdir().unwrap();
        let net = Net::open(
            &dir.path().join("net"),
            Arc::new(Anchor),
            Options::default(),
        )
        .unwrap();
        let core = net.core.clone();
        let trusted_seed = [21u8; 32];
        let trusted = crate::key::public_of(&trusted_seed);
        core.sign_next(|l| {
            l.devices.push((trusted, Role::Light));
            Ok(())
        })
        .unwrap();
        let _: DeviceList = core.devices().unwrap();

        let calls = Arc::new(AtomicUsize::new(0));
        let server = crate::endpoint_builder(&[20u8; 32], Some(lo()), &lo_services())
            .unwrap()
            .bind()
            .await
            .unwrap();
        let addr = EndpointAddr::from_parts(
            server.id(),
            server.bound_sockets().into_iter().map(TransportAddr::Ip),
        );
        let router = Router::builder(server)
            .accept(TEST_ALPN, Gate::new(core, Dummy(calls.clone()), TEST_ALPN))
            .spawn();

        for (seed, passes) in [(22u8, false), (21u8, true)] {
            let ep = crate::endpoint_builder(&[seed; 32], Some(lo()), &lo_services())
                .unwrap()
                .bind()
                .await
                .unwrap();
            let conn = ep.connect(addr.clone(), TEST_ALPN).await.unwrap();
            let closed = tokio::time::timeout(Duration::from_millis(500), conn.closed()).await;
            assert_eq!(closed.is_ok(), !passes, "seed {seed}");
            assert_eq!(calls.load(Ordering::SeqCst), usize::from(passes));
        }
        router.shutdown().await.unwrap();
    }

    #[test]
    fn an_unknown_peer_has_watane_only_a_trusted_one_has_the_other_protocols() {
        for alpn in [wire::ALPN, iroh_blobs::ALPN, iroh_gossip::ALPN] {
            assert!(allowed(Class::Trusted, alpn));
        }
        assert!(allowed(Class::Unknown, wire::ALPN));
        assert!(!allowed(Class::Unknown, iroh_blobs::ALPN));
        assert!(!allowed(Class::Unknown, iroh_gossip::ALPN));
        assert!(!allowed(Class::Unknown, b"something/else"));
    }

    #[test]
    fn the_byte_bucket_goes_into_debt_and_refills() {
        let pool = Arc::new(Pool::default());
        let limits = crate::Limits {
            unknown_bytes_per_sec: 1000,
            ..crate::Limits::default()
        };
        assert!(!pool.over_bytes(&limits));
        {
            let mut state = lock(&pool.state);
            state.refill(1000);
            state.tokens = -1000.0;
        }
        assert!(pool.over_bytes(&limits));
        std::thread::sleep(Duration::from_millis(1100));
        assert!(!pool.over_bytes(&limits));
        let peer = NodeKey::from_bytes([1; 32]);
        let one = pool.stream(peer, &limits).unwrap();
        let two = pool.stream(peer, &limits).unwrap();
        assert!(pool.stream(peer, &limits).is_none());
        drop(one);
        assert!(pool.stream(peer, &limits).is_some());
        drop(two);
    }
}
