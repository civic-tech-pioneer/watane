#![allow(
    dead_code,
    reason = "shared by several test binaries, each using only part of it"
)]
//! Test support: a hand-written CBOR builder, an Ed25519 oracle
//! (`ed25519-compact`), signed fixture entries, an in-memory holder, a raw
//! QUIC client and a fake callee. Nothing here uses `watane_net`'s codecs.

#![allow(clippy::unwrap_used, reason = "tests may panic")]

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
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

/// The key pair of a seed: deriving it costs a scalar multiplication, which
/// is slow in a debug build, and a chain signs with one seed many times.
fn keypair(seed: [u8; 32]) -> ed25519_compact::KeyPair {
    static PAIRS: Mutex<BTreeMap<[u8; 32], ed25519_compact::KeyPair>> = Mutex::new(BTreeMap::new());
    PAIRS
        .lock()
        .unwrap()
        .entry(seed)
        .or_insert_with(|| ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new(seed)))
        .clone()
}

pub fn sign(seed: [u8; 32], msg: &[u8]) -> [u8; 64] {
    *keypair(seed).sk.sign(msg, None)
}
pub fn public_of(seed: [u8; 32]) -> [u8; 32] {
    *keypair(seed).pk
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

/// A chain of `count` small entries, signed by hand. An entry's id is the
/// BLAKE3 hash of its bytes, so the chain is signed in a row and decoded (which
/// verifies each signature, slow in a debug build) on all cores after.
pub fn chain(seed: [u8; 32], count: u64) -> Vec<Entry> {
    chain_of(seed, count, b"x")
}

/// `chain` with a body of each entry's own.
pub fn chain_of(seed: [u8; 32], count: u64, body: &[u8]) -> Vec<Entry> {
    let mut raws: Vec<Vec<u8>> = Vec::new();
    let mut prev: Option<[u8; 32]> = None;
    for seq in 0..count {
        let raw = entry_bytes(seed, seq, prev.as_ref(), seq, "note", body);
        prev = Some(*blake3::hash(&raw).as_bytes());
        raws.push(raw);
    }
    let per = raws.len().div_ceil(4).max(1);
    std::thread::scope(|scope| {
        let parts: Vec<_> = raws
            .chunks(per)
            .map(|part| {
                scope.spawn(move || {
                    part.iter()
                        .map(|raw| Entry::decode(raw).unwrap())
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        parts.into_iter().flat_map(|h| h.join().unwrap()).collect()
    })
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

/// Signed once per test binary: appending to a log syncs to disk. The two
/// big chains (`cbig`, `c1200`) are in `chains` of `fixture_big` only: they
/// cost seconds to sign in a debug build, and few tests need them.
pub fn fixture() -> &'static Fixture {
    static FX: std::sync::OnceLock<Fixture> = std::sync::OnceLock::new();
    FX.get_or_init(|| make_fixture(false))
}

/// `fixture` with the two big chains.
pub fn fixture_big() -> &'static Fixture {
    static FX: std::sync::OnceLock<Fixture> = std::sync::OnceLock::new();
    FX.get_or_init(|| make_fixture(true))
}

fn make_fixture(with_big: bool) -> Fixture {
    // the two long chains take most of the time: build them on their own threads
    let big = std::thread::spawn(move || {
        if with_big {
            chain_of([8u8; 32], 70, &vec![7u8; 60_000])
        } else {
            vec![]
        }
    });
    let long = std::thread::spawn(move || {
        if with_big {
            chain([9u8; 32], 1200)
        } else {
            vec![]
        }
    });
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
    let ebig = big.join().unwrap();
    let cbig = PublicKey::from_bytes(&public_of([8u8; 32])).unwrap();
    let seed1200 = [9u8; 32];
    let c1200 = PublicKey::from_bytes(&public_of(seed1200)).unwrap();
    let e1200 = long.join().unwrap();
    let absent = PublicKey::from_bytes(&public_of([3u8; 32])).unwrap();
    let mut chains = BTreeMap::new();
    chains.insert(c1, e1);
    chains.insert(c2, e2);
    chains.insert(cf, ef.clone());
    if with_big {
        chains.insert(cbig, ebig);
        chains.insert(c1200, e1200);
    }
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
    /// `store_blob` fails with this text.
    pub store_blob_fails: Option<String>,
}

/// A holder in memory. It counts the calls it gets. Its chains and packages
/// change as `ingest` and `store_blob` are called, so they sit behind locks.
pub struct MemHoldings {
    chains: Mutex<BTreeMap<PublicKey, Vec<Entry>>>,
    pub fork: Option<(PublicKey, (Entry, Entry))>,
    pub calls: AtomicUsize,
    pub faults: Mutex<Faults>,
    /// This home holds the participant key.
    pub anchor: bool,
    /// Every `ingest` call with entries: channel, how many, why.
    pub ingests: Mutex<Vec<(PublicKey, usize, Why)>>,
    /// Every `store_blob` call.
    pub stored_blobs: Mutex<Vec<BlobId>>,
    /// The packages this holder holds: id to the file `blob(id)` gives.
    files: Mutex<HashMap<[u8; 32], std::path::PathBuf>>,
    /// The packages a test wants: `wanted_blobs` lists those not yet stored.
    wanted: Mutex<Vec<(BlobId, u64)>>,
    /// The bytes `store_blob` took.
    taken: Mutex<HashMap<[u8; 32], Vec<u8>>>,
    /// `ingest` answers `NotWanted` while set.
    pub not_wanted: std::sync::atomic::AtomicBool,
    /// `ingest` answers `Compromised` while set.
    pub compromised: std::sync::atomic::AtomicBool,
    /// What `search` answers, whatever the query.
    pub hits: Mutex<Vec<Hit>>,
    /// How many `entries` calls came for each channel.
    pub entries_calls: AtomicUsize,
    dir: tempfile::TempDir,
}

impl MemHoldings {
    /// A holder with these chains (and a forked channel), anchor or not.
    pub fn custom(
        chains: BTreeMap<PublicKey, Vec<Entry>>,
        fork: Option<(PublicKey, (Entry, Entry))>,
        anchor: bool,
    ) -> Arc<MemHoldings> {
        Arc::new(MemHoldings {
            chains: Mutex::new(chains),
            fork,
            calls: AtomicUsize::new(0),
            faults: Mutex::new(Faults::default()),
            anchor,
            ingests: Mutex::new(Vec::new()),
            stored_blobs: Mutex::new(Vec::new()),
            files: Mutex::new(HashMap::new()),
            wanted: Mutex::new(Vec::new()),
            taken: Mutex::new(HashMap::new()),
            not_wanted: std::sync::atomic::AtomicBool::new(false),
            compromised: std::sync::atomic::AtomicBool::new(false),
            hits: Mutex::new(Vec::new()),
            entries_calls: AtomicUsize::new(0),
            dir: tempfile::tempdir().unwrap(),
        })
    }
    pub fn empty() -> Arc<MemHoldings> {
        Self::custom(BTreeMap::new(), None, false)
    }
    pub fn new(fx: &Fixture) -> Arc<MemHoldings> {
        Self::custom(fx.chains.clone(), Some((fx.cf, fx.fork.clone())), false)
    }
    /// The same, in a home that holds the participant key.
    pub fn anchor(fx: &Fixture) -> Arc<MemHoldings> {
        Self::custom(fx.chains.clone(), Some((fx.cf, fx.fork.clone())), true)
    }
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
    /// A copy of the chains now held.
    pub fn chains(&self) -> BTreeMap<PublicKey, Vec<Entry>> {
        self.chains.lock().unwrap().clone()
    }
    /// The raw bytes of every entry of a channel held.
    pub fn raws(&self, c: &PublicKey) -> Vec<Vec<u8>> {
        self.chains()
            .get(c)
            .map(|v| v.iter().map(|e| e.bytes().to_vec()).collect())
            .unwrap_or_default()
    }
    /// An own append: the entry goes on the end of its channel's chain.
    pub fn append(&self, entry: Entry) {
        self.chains
            .lock()
            .unwrap()
            .entry(entry.author())
            .or_default()
            .push(entry);
    }
    /// A package this holder holds and a live release names: `blob(id)`
    /// gives its file and `wanted_blobs` of others lists it when they ask
    /// through `want_blob`.
    pub fn add_blob(&self, bytes: &[u8]) -> BlobId {
        let id = BlobId(*blake3::hash(bytes).as_bytes());
        let path = self.dir.path().join(format!("{}.pkg", hex(&id.0)));
        std::fs::write(&path, bytes).unwrap();
        self.files.lock().unwrap().insert(id.0, path);
        id
    }
    /// The holder no longer gives this package (a withdrawn release).
    pub fn remove_blob(&self, id: &BlobId) {
        self.files.lock().unwrap().remove(&id.0);
    }
    /// A package this holder wants (`wanted_blobs` lists it until stored).
    pub fn want_blob(&self, id: BlobId, size: u64) {
        self.wanted.lock().unwrap().push((id, size));
    }
    pub fn taken_blob(&self, id: &BlobId) -> Option<Vec<u8>> {
        self.taken.lock().unwrap().get(&id.0).cloned()
    }
    /// The hits `search` answers from now on.
    pub fn set_hits(&self, hits: Vec<Hit>) {
        *self.hits.lock().unwrap() = hits;
    }
    pub fn ingested_entries(&self) -> usize {
        self.ingests.lock().unwrap().iter().map(|(_, n, _)| n).sum()
    }
    fn enter(&self) -> Option<Duration> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.faults.lock().unwrap().delay
    }
    pub fn head_of(&self, c: &PublicKey) -> Option<HeadInfo> {
        let chains = self.chains.lock().unwrap();
        let last = chains.get(c)?.last()?;
        Some(HeadInfo {
            seq: last.seq(),
            id: last.id(),
            forked: self.fork.as_ref().is_some_and(|(k, _)| k == c),
        })
    }
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
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
            None => {
                let keys: Vec<_> = self.chains.lock().unwrap().keys().copied().collect();
                keys.iter().map(|c| (*c, self.head_of(c))).collect()
            }
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
        self.entries_calls.fetch_add(1, Ordering::SeqCst);
        if let Some(d) = self.enter() {
            std::thread::sleep(d);
        }
        if let Some(e) = self.faults.lock().unwrap().entries.clone() {
            return e;
        }
        let mut total = 0;
        let mut out = Vec::new();
        let chains = self.chains.lock().unwrap();
        for e in chains.get(channel).into_iter().flatten() {
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
    fn blob(&self, id: &BlobId) -> Option<std::path::PathBuf> {
        self.files.lock().unwrap().get(&id.0).cloned()
    }
    fn wanted_blobs(&self, _: Role) -> Vec<(BlobId, u64)> {
        let taken = self.taken.lock().unwrap();
        self.wanted
            .lock()
            .unwrap()
            .iter()
            .filter(|(id, _)| !taken.contains_key(&id.0))
            .cloned()
            .collect()
    }
    fn ingest(&self, channel: &PublicKey, entries: Vec<Entry>, why: Why) -> Ingested {
        self.ingests
            .lock()
            .unwrap()
            .push((*channel, entries.len(), why));
        if self.not_wanted.load(Ordering::SeqCst) {
            return Ingested::NotWanted;
        }
        if self.compromised.load(Ordering::SeqCst) {
            return Ingested::Compromised;
        }
        let mut chains = self.chains.lock().unwrap();
        let chain = chains.entry(*channel).or_default();
        for e in entries {
            let ok = e.author() == *channel
                && e.seq() == chain.len() as u64
                && e.prev() == chain.last().map(|l| l.id());
            if !ok {
                // a second entry at a seq already held is a fork: not chained
                if chain.get(e.seq() as usize).is_some() {
                    continue;
                }
                return Ingested::Failed("out of order".into());
            }
            chain.push(e);
        }
        Ingested::Stored(chain.len() as u64 - 1)
    }
    fn store_blob(&self, id: &BlobId, file: &Path) -> Result<(), String> {
        self.stored_blobs.lock().unwrap().push(*id);
        if let Some(cause) = self.faults.lock().unwrap().store_blob_fails.clone() {
            return Err(cause);
        }
        let bytes = std::fs::read(file).map_err(|e| e.to_string())?;
        self.taken.lock().unwrap().insert(id.0, bytes);
        Ok(())
    }
    fn search(&self, _: &Query) -> Vec<Hit> {
        self.enter();
        self.hits.lock().unwrap().clone()
    }
}

pub fn lo() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

/// Open a home and turn net on as `options` say: a new home opens with the
/// switches off (5.1), so `set_options` follows `open`.
pub fn open_on(
    net_dir: &std::path::Path,
    holdings: Arc<MemHoldings>,
    options: watane_net::Options,
) -> watane_net::Net {
    slot();
    let net = watane_net::Net::open(net_dir, holdings, options.clone()).unwrap();
    net.set_options(options).unwrap();
    net
}

pub struct Node {
    pub dir: tempfile::TempDir,
    pub net: watane_net::Net,
    pub key: NodeKey,
    pub holdings: Arc<MemHoldings>,
}

/// Limits whose budget of unknown peers cannot race a test: QUIC overhead
/// can drain the default 64 KiB/s bucket, and one unknown key may hold only
/// two connections. For a test that sends several requests from one unknown
/// key and asserts the code of each; the production budget is shown by
/// `budget::s9_the_default_budget_refuses_an_unknown_key_beyond_it`.
pub fn roomy_limits() -> watane_net::Limits {
    watane_net::Limits {
        unknown_bytes_per_sec: 16 << 20,
        unknown_conns_per_peer: 16,
        unknown_streams_per_peer: 16,
        ..watane_net::Limits::default()
    }
}

/// A node on loopback, on, paired with nobody.
pub fn node(holdings: Arc<MemHoldings>, limits: Option<watane_net::Limits>) -> Node {
    let dir = tempfile::tempdir().unwrap();
    let net = open_on(
        &dir.path().join("net"),
        Arc::clone(&holdings),
        options(limits),
    );
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
    slot();
    let dir = tempfile::tempdir().unwrap();
    let net_dir = dir.path().join("net");
    let key = {
        let net = watane_net::Net::open(
            &net_dir,
            Arc::<MemHoldings>::clone(&holdings),
            watane_net::Options::default(),
        )
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
    let net = open_on(&net_dir, Arc::clone(&holdings), options(limits));
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
    pair_with(anchor, joiner, role, false)
}

/// `pair_into`, with `replace` as the joiner says it.
pub fn pair_with(
    anchor: &Node,
    joiner: &Node,
    role: Role,
    replace: bool,
) -> watane_net::DeviceList {
    let list = pair_unsettled(anchor, joiner, role, replace);
    settle(&[anchor, joiner]);
    list
}

/// `pair_with` without the final settle, for a test that settles once after
/// several pairings.
pub fn pair_unsettled(
    anchor: &Node,
    joiner: &Node,
    role: Role,
    replace: bool,
) -> watane_net::DeviceList {
    let (ea, ej) = (anchor.net.events(), joiner.net.events());
    let code = anchor.net.pair_code(role).unwrap();
    let list = try_pair(anchor, joiner, code, replace, Some(true)).unwrap();
    anchor.net.add_addrs(&joiner.key, &joiner.net.addrs());
    joiner.net.add_addrs(&anchor.key, &anchor.net.addrs());
    // the machines connect and sync now: wait until that has settled
    for (events, peer) in [(ea, joiner.key), (ej, anchor.key)] {
        loop {
            match events.recv_timeout(Duration::from_secs(10)).unwrap() {
                watane_net::Event::PeerUp(k) if k == peer => break,
                _ => {}
            }
        }
    }
    list
}

/// Wait until the holders of these nodes have not been called for a while.
pub fn settle(nodes: &[&Node]) {
    // quiet: the holders' call counts did not change over 300 ms (6 readings 50 ms apart)
    let mut last: Vec<usize> = vec![];
    let mut same = 0;
    for _ in 0..600 {
        std::thread::sleep(Duration::from_millis(50));
        let now: Vec<usize> = nodes.iter().map(|n| n.holdings.calls()).collect();
        if now == last {
            same += 1;
            if same >= 6 {
                return;
            }
        } else {
            same = 0;
            last = now;
        }
    }
    panic!("the holders never settled");
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
    slot();
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
    fake_machine(seed, reply, None)
}

/// A callee of `watane/1` that also answers every request on iroh-blobs'
/// ALPN with `blob` (raw bytes: the size header and data of a small blob).
pub fn fake_machine(
    seed: [u8; 32],
    reply: impl Fn(&[u8]) -> Option<Vec<u8>> + Send + Sync + 'static,
    blob: Option<Vec<u8>>,
) -> Fake {
    let rt = runtime();
    let reply = Arc::new(reply);
    let (ep, addr) = rt.block_on(async {
        let ep = builder(seed)
            .alpns(vec![b"watane/1".to_vec(), iroh_blobs::ALPN.to_vec()])
            .bind()
            .await
            .unwrap();
        let addr = ep.bound_sockets()[0];
        (ep, addr)
    });
    let accept = ep.clone();
    rt.spawn(async move {
        while let Some(incoming) = accept.accept().await {
            let reply = Arc::clone(&reply);
            let blob = blob.clone();
            tokio::spawn(async move {
                let Ok(conn) = incoming.await else { return };
                let mut held = Vec::new();
                if conn.alpn() == iroh_blobs::ALPN {
                    while let Ok((mut send, mut recv)) = conn.accept_bi().await {
                        let mut buf = [0u8; 4096];
                        let _ = recv.read(&mut buf).await;
                        if let Some(bytes) = &blob {
                            let _ = send.write_all(bytes).await;
                            let _ = send.finish();
                        }
                        held.push(send);
                    }
                    return;
                }
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

/// A hello answer, by hand: version 1, the ops, ten limits, no list seq.
pub fn hello_answer() -> Vec<u8> {
    use cbor::*;
    let ops: Vec<Vec<u8>> = ["hello", "heads", "entries", "fork", "devices", "pair"]
        .iter()
        .map(|o| text(o))
        .collect();
    let limits: Vec<Vec<u8>> = [
        262144u64, 4194304, 1000, 65536, 67108864, 200, 10000, 5000, 600000, 120000,
    ]
    .iter()
    .map(|n| uint(*n))
    .collect();
    concat(&[
        array(&[text("ok")]),
        array(&[uint(1), array(&ops), array(&limits), vec![NULL]]),
    ])
}

/// A heads answer with one channel, by hand.
pub fn heads_answer_of(channel: &PublicKey, seq: u64, id: &[u8; 32]) -> Vec<u8> {
    use cbor::*;
    concat(&[
        array(&[text("ok")]),
        array(&[bytes(channel.as_bytes()), uint(seq), bytes(id), vec![FALSE]]),
    ])
}

/// An entries answer, by hand.
pub fn entries_answer_of(entries: &[Vec<u8>]) -> Vec<u8> {
    use cbor::*;
    let mut items = vec![array(&[text("ok")])];
    items.extend(entries.iter().map(|e| bytes(e)));
    concat(&items)
}

/// What op a request names (its first text), by hand.
pub fn op_of(request: &[u8]) -> &'static str {
    for op in ["hello", "heads", "entries", "fork", "devices"] {
        if request
            .windows(op.len())
            .take(12)
            .any(|w| w == op.as_bytes())
        {
            return op;
        }
    }
    "other"
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

/// A negative check: `f` stays true for `millis` ms, looked at every 50 ms
/// (the window is what is tested: something that must not happen).
pub fn stays(millis: u64, mut f: impl FnMut() -> bool) -> bool {
    let end = std::time::Instant::now() + Duration::from_millis(millis);
    while std::time::Instant::now() < end {
        if !f() {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    f()
}

/// Runs the test `name` (its full path in the `it` binary) alone in a child process and returns
/// true; in that child it returns false and the test goes on. For a test that counts the
/// process's sockets, which every test of the binary shares.
pub fn rerun_alone(name: &str) -> bool {
    if std::env::var_os("WATANE_RERUN_ALONE").is_some() {
        return false;
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--test-threads=1"])
        .env("WATANE_RERUN_ALONE", "1")
        .output()
        .unwrap();
    let text =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success() && text.contains("1 passed"), "{text}");
    true
}

/// At most `SLOTS` tests run machines at once. Each file was its own binary, so only a file's own
/// tests ran together; now all of net's do, and tests with 1 s deadlines or a 5 s bound failed
/// under that load (3 runs in 10 at 16 threads, none in 8 at 6). The test's own thread takes a
/// slot when it opens its first machine and gives it back when the test ends. Only libtest's test
/// threads, named `module::test`, take one: a runtime's worker threads outlive the test.
// ponytail: a fixed count; make it a share of the cores if machines change a lot
pub fn slot() {
    use std::sync::Condvar;
    const SLOTS: usize = 6;
    static TAKEN: (Mutex<usize>, Condvar) = (Mutex::new(0), Condvar::new());
    struct Slot;
    impl Drop for Slot {
        fn drop(&mut self) {
            *TAKEN.0.lock().unwrap() -= 1;
            TAKEN.1.notify_one();
        }
    }
    thread_local!(static HELD: std::cell::RefCell<Option<Slot>> = const { std::cell::RefCell::new(None) });
    if !std::thread::current()
        .name()
        .is_some_and(|n| n.contains("::"))
        || HELD.with(|h| h.borrow().is_some())
    {
        return;
    }
    let mut taken = TAKEN.0.lock().unwrap();
    while *taken >= SLOTS {
        taken = TAKEN.1.wait(taken).unwrap();
    }
    *taken += 1;
    HELD.with(|h| *h.borrow_mut() = Some(Slot));
}
