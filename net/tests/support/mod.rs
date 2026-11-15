#![allow(dead_code)]
//! Test support: a hand-written CBOR builder, an Ed25519 oracle
//! (`ed25519-compact`), signed fixture entries, an in-memory holder, a raw
//! QUIC client and a fake callee. Nothing here uses `watane_net`'s codecs.

use std::{
    collections::{BTreeMap, HashMap},
    net::SocketAddr,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use watane_log::{Entry, Log, PublicKey};
use watane_net::{BlobId, HeadInfo, Hit, Holdings, Ingested, NodeKey, Query, Role, Why};

pub mod cbor {
    pub fn head(major: u8, n: u64, out: &mut Vec<u8>) {
        let m = major << 5;
        if n < 24 {
            out.push(m | n as u8);
        } else if n <= 0xff {
            out.extend([m | 24, n as u8]);
        } else if n <= 0xffff {
            out.push(m | 25);
            out.extend((n as u16).to_be_bytes());
        } else if n <= 0xffff_ffff {
            out.push(m | 26);
            out.extend((n as u32).to_be_bytes());
        } else {
            out.push(m | 27);
            out.extend(n.to_be_bytes());
        }
    }
    pub fn uint(n: u64) -> Vec<u8> {
        let mut o = vec![];
        head(0, n, &mut o);
        o
    }
    pub fn bytes(b: &[u8]) -> Vec<u8> {
        let mut o = vec![];
        head(2, b.len() as u64, &mut o);
        o.extend(b);
        o
    }
    pub fn text(t: &str) -> Vec<u8> {
        let mut o = vec![];
        head(3, t.len() as u64, &mut o);
        o.extend(t.as_bytes());
        o
    }
    pub fn array(items: &[Vec<u8>]) -> Vec<u8> {
        let mut o = vec![];
        head(4, items.len() as u64, &mut o);
        for i in items {
            o.extend(i);
        }
        o
    }
    pub const NULL: u8 = 0xf6;
    pub const TRUE: u8 = 0xf5;
    pub const FALSE: u8 = 0xf4;
    /// Concatenate (a CBOR sequence).
    pub fn concat(items: &[Vec<u8>]) -> Vec<u8> {
        items.concat()
    }
}

pub fn sign(seed: [u8; 32], msg: &[u8]) -> [u8; 64] {
    *ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new(seed))
        .sk
        .sign(msg, None)
}
pub fn public_of(seed: [u8; 32]) -> [u8; 32] {
    *ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new(seed)).pk
}

/// A signed entry, by hand.
pub fn entry_bytes(
    seed: [u8; 32],
    seq: u64,
    prev: Option<&[u8; 32]>,
    time: u64,
    kind: &str,
    body: &[u8],
) -> Vec<u8> {
    use cbor::*;
    let author = public_of(seed);
    let fields = [
        bytes(&author),
        uint(seq),
        prev.map_or(vec![NULL], |p| bytes(p)),
        uint(time),
        text(kind),
        bytes(body),
    ];
    let mut signed = b"watane/entry/1\0".to_vec();
    signed.extend(array(&fields));
    let sig = sign(seed, &signed);
    let mut all = fields.to_vec();
    all.push(bytes(&sig));
    array(&all)
}

/// A chain of `count` small entries, signed by hand.
pub fn chain(seed: [u8; 32], count: u64) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for seq in 0..count {
        let prev = out.last().map(|e| *e.id().as_bytes());
        let raw = entry_bytes(seed, seq, prev.as_ref(), seq, "note", b"x");
        out.push(Entry::decode(&raw).unwrap());
    }
    out
}

pub struct Fixture {
    pub c1: PublicKey,
    pub c2: PublicKey,
    pub cf: PublicKey,
    pub cbig: PublicKey,
    pub c1200: PublicKey,
    pub absent: PublicKey,
    pub chains: BTreeMap<PublicKey, Vec<Entry>>,
    pub fork: (Entry, Entry),
}

fn log_chain(count: u64, body: &[u8]) -> (PublicKey, Vec<Entry>, [u8; 32]) {
    let dir = tempfile::tempdir().unwrap();
    let logdir = dir.path().join("log");
    let log = Log::open(&logdir).unwrap();
    for i in 0..count {
        log.append("note", &[body, &i.to_be_bytes()].concat())
            .unwrap();
    }
    let me = log.identity().unwrap();
    let entries = log.entries(&me, 0, count as usize).unwrap();
    let seed: [u8; 32] = std::fs::read(logdir.join("identity.key"))
        .unwrap()
        .try_into()
        .unwrap();
    drop(log);
    (me, entries, seed)
}

/// Signed once per test binary: appending to a log syncs to disk.
pub fn fixture() -> &'static Fixture {
    static FX: std::sync::OnceLock<Fixture> = std::sync::OnceLock::new();
    FX.get_or_init(make_fixture)
}

fn make_fixture() -> Fixture {
    let (c1, e1, _) = log_chain(5, b"one");
    let (c2, e2, _) = log_chain(3, b"two");
    let (cf, ef, seed) = log_chain(3, b"forked");
    // a second entry at seq 1, signed by the same author: the fork
    let other = Entry::decode(&entry_bytes(
        seed,
        1,
        Some(ef[0].id().as_bytes()),
        99,
        "note",
        b"the other branch",
    ))
    .unwrap();
    assert_ne!(other.id(), ef[1].id());
    let (cbig, ebig, _) = log_chain(70, &vec![7u8; 60_000]);
    let seed1200 = [9u8; 32];
    let c1200 = PublicKey::from_bytes(&public_of(seed1200)).unwrap();
    let e1200 = chain(seed1200, 1200);
    let absent = PublicKey::from_bytes(&public_of([3u8; 32])).unwrap();
    let mut chains = BTreeMap::new();
    chains.insert(c1, e1);
    chains.insert(c2, e2);
    chains.insert(cf, ef.clone());
    chains.insert(cbig, ebig);
    chains.insert(c1200, e1200);
    Fixture {
        c1,
        c2,
        cf,
        cbig,
        c1200,
        absent,
        fork: (ef[1].clone(), other),
        chains,
    }
}

/// What a faulty holder gives instead of the truth.
#[derive(Default)]
pub struct Faults {
    pub heads: Option<Vec<(PublicKey, Option<HeadInfo>)>>,
    pub entries: Option<Vec<Entry>>,
    pub fork: Option<Option<(Entry, Entry)>>,
    pub delay: Option<Duration>,
}

/// A holder in memory. It counts the calls it gets.
pub struct MemHoldings {
    pub chains: BTreeMap<PublicKey, Vec<Entry>>,
    pub fork: Option<(PublicKey, (Entry, Entry))>,
    pub calls: AtomicUsize,
    pub faults: Mutex<Faults>,
    /// This home holds the participant key.
    pub anchor: bool,
}

impl MemHoldings {
    pub fn empty() -> Arc<MemHoldings> {
        Arc::new(MemHoldings {
            chains: BTreeMap::new(),
            fork: None,
            calls: AtomicUsize::new(0),
            faults: Mutex::new(Faults::default()),
            anchor: false,
        })
    }
    pub fn new(fx: &Fixture) -> Arc<MemHoldings> {
        Arc::new(MemHoldings {
            chains: fx.chains.clone(),
            fork: Some((fx.cf, fx.fork.clone())),
            calls: AtomicUsize::new(0),
            faults: Mutex::new(Faults::default()),
            anchor: false,
        })
    }
    /// The same, in a home that holds the participant key.
    pub fn anchor(fx: &Fixture) -> Arc<MemHoldings> {
        Arc::new(MemHoldings {
            chains: fx.chains.clone(),
            fork: Some((fx.cf, fx.fork.clone())),
            calls: AtomicUsize::new(0),
            faults: Mutex::new(Faults::default()),
            anchor: true,
        })
    }
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
    fn enter(&self) -> Option<Duration> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.faults.lock().unwrap().delay
    }
    pub fn head_of(&self, c: &PublicKey) -> Option<HeadInfo> {
        let last = self.chains.get(c)?.last()?;
        Some(HeadInfo {
            seq: last.seq(),
            id: last.id(),
            forked: self.fork.as_ref().is_some_and(|(k, _)| k == c),
        })
    }
}

impl Holdings for MemHoldings {
    fn is_anchor(&self) -> bool {
        self.anchor
    }
    fn heads(&self, channels: Option<&[PublicKey]>) -> Vec<(PublicKey, Option<HeadInfo>)> {
        if let Some(d) = self.enter() {
            std::thread::sleep(d);
        }
        if let Some(h) = self.faults.lock().unwrap().heads.clone() {
            return h;
        }
        match channels {
            None => self.chains.keys().map(|c| (*c, self.head_of(c))).collect(),
            Some(list) => list.iter().map(|c| (*c, self.head_of(c))).collect(),
        }
    }
    fn entries(
        &self,
        channel: &PublicKey,
        from: u64,
        limit: usize,
        max_bytes: usize,
    ) -> Vec<Entry> {
        if let Some(d) = self.enter() {
            std::thread::sleep(d);
        }
        if let Some(e) = self.faults.lock().unwrap().entries.clone() {
            return e;
        }
        let mut total = 0;
        let mut out = Vec::new();
        for e in self.chains.get(channel).into_iter().flatten() {
            if e.seq() < from {
                continue;
            }
            if out.len() == limit || total + e.bytes().len() > max_bytes {
                break;
            }
            total += e.bytes().len();
            out.push(e.clone());
        }
        out
    }
    fn fork(&self, channel: &PublicKey) -> Option<(Entry, Entry)> {
        if let Some(d) = self.enter() {
            std::thread::sleep(d);
        }
        if let Some(f) = self.faults.lock().unwrap().fork.clone() {
            return f;
        }
        self.fork
            .as_ref()
            .filter(|(k, _)| k == channel)
            .map(|(_, p)| p.clone())
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
        Ok(())
    }
    fn search(&self, _: &Query) -> Vec<Hit> {
        Vec::new()
    }
}

pub fn lo() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

pub struct Node {
    pub dir: tempfile::TempDir,
    pub net: watane_net::Net,
    pub key: NodeKey,
    pub holdings: Arc<MemHoldings>,
}

/// A node on loopback, on, paired with nobody.
pub fn node(holdings: Arc<MemHoldings>, limits: Option<watane_net::Limits>) -> Node {
    let dir = tempfile::tempdir().unwrap();
    let net =
        watane_net::Net::open(&dir.path().join("net"), holdings.clone(), options(limits)).unwrap();
    let key = net.node_key();
    Node {
        dir,
        net,
        key,
        holdings,
    }
}

pub fn options(limits: Option<watane_net::Limits>) -> watane_net::Options {
    watane_net::Options {
        on: true,
        bind: Some(lo()),
        limits: limits.unwrap_or_default(),
        ..watane_net::Options::default()
    }
}

/// A node on loopback whose own device list (signed with its node key, as
/// its anchor) holds `trusted` as full machines. For callers that are raw
/// endpoints or fakes, which cannot pair.
pub fn node_trusting(
    holdings: Arc<MemHoldings>,
    trusted: Vec<NodeKey>,
    limits: Option<watane_net::Limits>,
) -> Node {
    let dir = tempfile::tempdir().unwrap();
    let net_dir = dir.path().join("net");
    let key = {
        let net = watane_net::Net::open(&net_dir, holdings.clone(), watane_net::Options::default())
            .unwrap();
        net.node_key()
    };
    let seed: [u8; 32] = std::fs::read(net_dir.join("node.key"))
        .unwrap()
        .try_into()
        .unwrap();
    let mut devices: Vec<_> = trusted.into_iter().map(|k| (k, Role::Full)).collect();
    devices.push((key, Role::Full));
    devices.sort_by_key(|(k, _)| *k);
    let list = watane_net::DeviceList {
        anchor: key,
        seq: 1,
        time: 1,
        expires_days: 0,
        devices,
    };
    std::fs::write(net_dir.join("devices"), list.sign(&seed).unwrap()).unwrap();
    let net = watane_net::Net::open(&net_dir, holdings.clone(), options(limits)).unwrap();
    Node {
        dir,
        net,
        key,
        holdings,
    }
}

/// Pair `joiner` to `anchor` as `role` through the code, the join question
/// and its answer. Both learn each other's addresses.
pub fn pair_into(anchor: &Node, joiner: &Node, role: Role) -> watane_net::DeviceList {
    let code = anchor.net.pair_code(role).unwrap();
    let list = try_pair(anchor, joiner, code, false, Some(true)).unwrap();
    anchor.net.add_addrs(&joiner.key, &joiner.net.addrs());
    joiner.net.add_addrs(&anchor.key, &anchor.net.addrs());
    list
}

/// Two paired nodes: A the anchor, B a full machine, each trusting the other
/// through the device list, each knowing where the other is.
pub fn pair(fx: &Fixture) -> (Node, Node) {
    let a = node(MemHoldings::anchor(fx), None);
    let b = node(MemHoldings::new(fx), None);
    pair_into(&a, &b, Role::Full);
    (a, b)
}

// ---- raw QUIC: a client and a fake callee that speak bytes only ----

pub fn builder(seed: [u8; 32]) -> iroh::endpoint::Builder {
    iroh::Endpoint::builder(iroh::endpoint::presets::Minimal)
        .secret_key(iroh::SecretKey::from_bytes(&seed))
        .relay_mode(iroh::RelayMode::Disabled)
        .clear_address_lookup()
        .portmapper_config(iroh::endpoint::PortmapperConfig::Disabled)
        .net_report_config(iroh::NetReportConfig::minimal())
        .clear_ip_transports()
        .bind_addr("127.0.0.1:0")
        .unwrap()
}

pub fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
}

pub fn node_key_of_seed(seed: [u8; 32]) -> NodeKey {
    NodeKey::from_bytes(public_of(seed))
}

/// A raw client with a chosen key: sends `request` bytes as they are on one
/// stream of `watane/1` and returns all the answer bytes.
pub fn raw_call(seed: [u8; 32], server: &NodeKey, addr: SocketAddr, request: &[u8]) -> Vec<u8> {
    let rt = runtime();
    rt.block_on(async {
        let ep = builder(seed).bind().await.unwrap();
        let id = iroh::EndpointId::from_bytes(server.as_bytes()).unwrap();
        let conn = ep
            .connect(
                iroh::EndpointAddr::from_parts(id, [iroh::TransportAddr::Ip(addr)]),
                b"watane/1",
            )
            .await
            .unwrap();
        let (mut send, mut recv) = conn.open_bi().await.unwrap();
        // the callee may answer and reset before it has read a large request
        let _ = send.write_all(request).await;
        let _ = send.finish();
        let answer = recv.read_to_end(16 << 20).await.unwrap();
        conn.close(0u32.into(), b"");
        answer
    })
}

/// A callee that answers every request with `reply(request bytes)`, or
/// never, when `reply` returns `None`.
pub struct Fake {
    pub key: NodeKey,
    pub addr: SocketAddr,
    _rt: tokio::runtime::Runtime,
}

pub fn fake_callee(
    seed: [u8; 32],
    reply: impl Fn(&[u8]) -> Option<Vec<u8>> + Send + Sync + 'static,
) -> Fake {
    let rt = runtime();
    let reply = Arc::new(reply);
    let (ep, addr) = rt.block_on(async {
        let ep = builder(seed)
            .alpns(vec![b"watane/1".to_vec()])
            .bind()
            .await
            .unwrap();
        let addr = ep.bound_sockets()[0];
        (ep, addr)
    });
    let accept = ep.clone();
    rt.spawn(async move {
        while let Some(incoming) = accept.accept().await {
            let reply = reply.clone();
            tokio::spawn(async move {
                let Ok(conn) = incoming.await else { return };
                let mut held = Vec::new();
                while let Ok((mut send, mut recv)) = conn.accept_bi().await {
                    let Ok(request) = recv.read_to_end(1 << 20).await else {
                        return;
                    };
                    if let Some(answer) = reply(&request) {
                        let _ = send.write_all(&answer).await;
                        let _ = send.finish();
                    }
                    // an unanswered stream stays open
                    held.push(send);
                }
            });
        }
    });
    Fake {
        key: node_key_of_seed(seed),
        addr,
        _rt: rt,
    }
}

pub fn map_of<K: std::hash::Hash + Eq, V>(
    items: impl IntoIterator<Item = (K, V)>,
) -> HashMap<K, V> {
    items.into_iter().collect()
}

/// `(code, message)` of an `["err", code, message]` answer, parsed by hand;
/// `None` for any other answer.
pub fn err_of(answer: &[u8]) -> Option<(String, String)> {
    fn text(b: &[u8]) -> Option<(String, &[u8])> {
        let (len, rest) = match *b.first()? {
            h @ 0x60..=0x77 => ((h & 31) as usize, &b[1..]),
            0x78 => (*b.get(1)? as usize, &b[2..]),
            _ => return None,
        };
        let s = String::from_utf8(rest.get(..len)?.to_vec()).ok()?;
        Some((s, &rest[len..]))
    }
    let rest = answer.strip_prefix(&[0x83])?;
    let (kind, rest) = text(rest)?;
    if kind != "err" {
        return None;
    }
    let (code, rest) = text(rest)?;
    let (message, _) = text(rest)?;
    Some((code, message))
}

/// A hello request, by hand.
pub fn hello_request() -> Vec<u8> {
    cbor::array(&[cbor::text("hello"), cbor::uint(1)])
}

/// Join `joiner` to `anchor` with `code`. `answer` is what the participant
/// says to the join question; `None`: nothing.
pub fn try_pair(
    anchor: &Node,
    joiner: &Node,
    code: watane_net::PairCode,
    replace: bool,
    answer: Option<bool>,
) -> Result<watane_net::DeviceList, watane_net::NetError> {
    let events = anchor.net.events();
    let net = joiner.net.clone();
    let thread = std::thread::spawn(move || net.join(&code, replace));
    if let Some(yes) = answer {
        loop {
            match events.recv_timeout(Duration::from_secs(10)).unwrap() {
                watane_net::Event::JoinAsked { node, .. } => {
                    assert_eq!(node, joiner.key);
                    anchor.net.answer_join(&node, yes);
                    break;
                }
                _ => continue,
            }
        }
    }
    thread.join().unwrap()
}

/// Wait until `f` holds, for at most `secs` seconds.
pub fn wait_for(secs: u64, mut f: impl FnMut() -> bool) -> bool {
    let end = std::time::Instant::now() + Duration::from_secs(secs);
    while std::time::Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    f()
}
