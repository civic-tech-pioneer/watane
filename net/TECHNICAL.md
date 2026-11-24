# net: technical specification

How `net` does what `FUNCTIONAL.md` says, for stage D1 of `spec/PEERS.md`
(step 8, own machines). Requirements are cited as `NET-F-<n>`, the
architecture as PEERS sections. Section 13 lists the decisions made here;
section 12 the questions left for the step 8 design slice.

Stage N1 (section 9) created the crate and added it to the workspace.

## 1. Crate

| Item | Value |
|---|---|
| Package | `watane-net`, lib `watane_net`, no binary |
| Edition, toolchain | 2024, stable, `rust-version = "1.91"` |
| Lints | Workspace `unsafe_code = "deny"`. Only `watane-log-windows-fs` may use `#[allow(unsafe_code)]` (spec/TECHNICAL 2). Review enforces this rule. `cargo clippy --all-targets -- -D warnings` is clean. |
| Threads | `net` owns one multi-thread tokio runtime. The public API is blocking: each call runs on that runtime and waits. A caller must not call it from inside a tokio runtime. `net` calls the holder only from blocking threads (`spawn_blocking`). |
| Network | QUIC through iroh (UDP), and nothing while off (NET-F-3). |
| Opens | `<home>/net/` only. Never `<home>/log/` or `<home>/dist/`. |

```text
net/
  Cargo.toml
  src/
    lib.rs        Net, Options; re-exports; the only `pub` items (section 3)
    error.rs      NetError, the wire error codes (4.2)
    key.rs        NodeKey, the node key file (4.5)
    ticket.rs     Ticket, PairCode: parse and format (4.1)
    wire.rs       requests, answers, limits, the codecs of 4.2 (N1: hello, heads, entries, fork)
    cbor.rs       the strict CBOR reader and the writer helpers that wire.rs and devices.rs share
    client.rs     the caller of watane/1: send, read within the byte limit and deadline, check the answer (5.2, NET-F-23)
    devices.rs    DeviceList: encode, sign, verify (4.3); pairing (5.5)
    serve.rs      the handler of `watane/1`; the wrappers of the iroh-blobs and iroh-gossip handlers (5.4)
    allow.rs      allowances: what each peer may use, and its byte, connection and stream budget (4.2)
    sync.rs       kept connections, the keeper, pulls (5.2, 5.6)
    gossip.rs     the own-machines topic, the hint codec and rule (5.7)
    blobs.rs      the iroh-blobs store, the serve hook, fetch (5.8)
    reach.rs      services by configuration, the switches and addresses of book.json, the cause of a failed dial, the relay server (5.10)
  tests/
    support/mod.rs   MemHoldings (an in-memory holder), signed fixture entries, local nodes
    node.rs  pair.rs  serve.rs  sync.rs  light.rs  reach.rs  restore.rs
```

Inner modules are private. Only `lib.rs` exports.

## 2. Dependencies

Versions as on crates.io on 2026-11-14. Stage N1 pinned the three iroh
crates to the exact releases that resolve together (2026-11-15). The
versions of the others are the minimum; `Cargo.lock` holds the rest.

```toml
[dependencies]
watane-log = { path = "../log" }   # PublicKey, Entry, EntryId: decode and verify only
iroh = "=1.4.1"                    # QUIC dialed by Ed25519 key, hole punching, relays, mDNS, pkarr
iroh-blobs = "=0.105.0"            # BLAKE3 verified streaming
iroh-gossip = "=0.102.0"           # HyParView and Plumtree
iroh-relay = { version = "=1.4.1", features = ["server"] }  # the relay server of `relay_server` (5.10); a product option, so a normal dependency
iroh-mdns-address-lookup = "=0.6.1"    # mDNS on the local network (5.10)
iroh-mainline-address-lookup = "=0.6.1" # the Mainline DHT, when `dht` is on (5.10)
tokio = { version = "1.54", features = ["rt-multi-thread", "sync", "time", "macros"] }
ed25519-dalek = "3.0"              # node-key signatures of 4.3, as log uses it
blake3 = "1.8"
minicbor = { version = "2.3", default-features = false, features = ["alloc"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
getrandom = "0.4"
zeroize = "1.9"
url = "2.5"
thiserror = "2.0"

[dev-dependencies]
tempfile = "3.27"
ed25519-compact = "2.6"            # oracle for the vectors of 4.3
```

Why iroh: it dials a node by its Ed25519 key, punches holes through NATs,
falls back to a relay that forwards only encrypted QUIC, finds nodes on the
local network (mDNS), and can publish addresses in the Mainline DHT
(pkarr). Its relay server ships as a crate, so `watane-node` can run one.
iroh-blobs moves BLAKE3-named blobs with verified streaming, and its hash
is the plain BLAKE3 hash, so package ids work unchanged. iroh-gossip
spreads short messages to the members of a topic. The node key is a plain
Ed25519 key, so another transport can take it over without a new key.

## 3. Public API

```rust
/// The Ed25519 key of a node. Text form: 52 characters of z-base-32,
/// the same rules as log's PublicKey (log TECHNICAL 4.1). Short form: 8.
pub struct NodeKey(/* 32 bytes */);
pub struct BlobId(pub [u8; 32]);            // a BLAKE3 hash

pub enum Class { Unknown, Trusted }          // D2 adds Friendly, D4 Public
pub enum Role { Light, Full }

#[derive(Clone, Debug, Default)]      // node Q4; Limits derives the same, its Default is 4.2's
pub struct Options {
    pub on: bool,                     // false: no socket at all (NET-F-3)
    pub relays: Vec<url::Url>,        // chosen by the participant; empty by default
    pub dht: bool,                    // publish and look up addresses in the Mainline DHT; false by default
    pub relay_server: Option<std::net::SocketAddr>, // watane-node only: run iroh-relay here (NET-F-29)
    pub limits: Limits,               // section 4.2 defaults, budgets included
    pub bind: Option<std::net::SocketAddr>, // where to bind; default 0.0.0.0:0
}

pub struct Net { /* Send + Sync; cheap to clone */ }

impl Net {
    pub fn open(home_net: &Path, holdings: Arc<dyn Holdings>, options: Options) -> Result<Net, NetError>;
    pub fn set_options(&self, options: Options) -> Result<(), NetError>; // on/off, relays, dht: written to book.json (5.1)
    pub fn options(&self) -> Options;                                    // the switches as open merged them with book.json, or as set_options last stored them
    pub fn close(self);                                                  // writes no switch: a closed node reopens as it was
    pub fn relay_url(&self) -> Option<url::Url>;                         // `http://<bound addr>/` while the relay server runs, else None

    pub fn node_key(&self) -> NodeKey;
    pub fn class_of(&self, node: &NodeKey) -> Class;
    pub fn addrs(&self) -> Vec<std::net::SocketAddr>;                        // bound local addresses; empty while off
    pub fn add_addrs(&self, node: &NodeKey, addrs: &[std::net::SocketAddr]); // where to dial a node, after the stored ones (4.5)

    // the callers of `watane/1` (N1): each checks the answer before it returns it (4.2, NET-F-23)
    pub fn hello(&self, node: &NodeKey) -> Result<Hello, NetError>;
    pub fn heads_of(&self, node: &NodeKey, channels: &[PublicKey]) -> Result<Vec<(PublicKey, Option<HeadInfo>)>, NetError>;
    pub fn entries_of(&self, node: &NodeKey, channel: &PublicKey, from: u64, limit: u64) -> Result<Vec<Entry>, NetError>;
    pub fn fork_of(&self, node: &NodeKey, channel: &PublicKey) -> Result<Option<(Entry, Entry)>, NetError>;

    // own machines (5.5)
    pub fn pair_code(&self, role: Role) -> Result<PairCode, NetError>;      // anchor only
    pub fn answer_join(&self, node: &NodeKey, yes: bool);                   // after Event::JoinAsked
    pub fn join(&self, code: &PairCode, replace: bool) -> Result<DeviceList, NetError>;
    pub fn devices(&self) -> Option<DeviceList>;
    pub fn remove_device(&self, node: &NodeKey) -> Result<DeviceList, NetError>;   // anchor only
    pub fn set_role(&self, node: &NodeKey, role: Role) -> Result<DeviceList, NetError>; // anchor only
    pub fn set_expiry(&self, days: u32) -> Result<DeviceList, NetError>;     // anchor only; 0: never

    // for the holder's callers (5.6 to 5.9, 5.11)
    pub fn told(&self, channel: &PublicKey, seq: u64, id: &EntryId);  // a new head here: tell own machines
    pub fn need(&self, channel: &PublicKey) -> Result<(), NetError>;   // light: pull it from a full machine
    pub fn need_blob(&self, id: &BlobId, size: u64) -> Result<(), NetError>;
    pub fn query(&self, query: &Query) -> Vec<(NodeKey, Vec<Hit>)>;   // light: ask a full machine
    pub fn own_heads(&self, channel: &PublicKey) -> Vec<(NodeKey, Option<HeadInfo>)>; // restore hints
    pub fn events(&self) -> std::sync::mpsc::Receiver<Event>;
}

pub struct Hello { pub version: u64, pub ops: Vec<String>, pub limits: Limits, pub devices_seq: Option<u64> }
pub struct DeviceList { pub anchor: NodeKey, pub seq: u64, pub time: u64,
                        pub expires_days: u32, pub devices: Vec<(NodeKey, Role)> }
pub struct HeadInfo { pub seq: u64, pub id: EntryId, pub forked: bool }

pub enum Event {
    JoinAsked { node: NodeKey, role: Role },          // the app shows the short key and asks
    DevicesChanged(DeviceList),
    Pulled { channel: PublicKey, seq: u64, from: NodeKey },
    Refused { node: NodeKey, cause: String },         // a bad object (NET-F-24)
    Unreachable { node: NodeKey, cause: String },     // names "a relay or the DHT is needed" when so (NET-F-28)
    PeerUp(NodeKey), PeerDown(NodeKey),
}
```

### 3.1 The holder

`net` stores no channel. It reads and writes them through the holder,
which `node` implements over `dist` (section 8). `net` defines the trait,
and no method calls back into `net`.

```rust
pub trait Holdings: Send + Sync + 'static {
    fn is_anchor(&self) -> bool;                                   // this home holds the participant key
    fn heads(&self, channels: Option<&[PublicKey]>) -> Vec<(PublicKey, Option<HeadInfo>)>; // None: all held
    fn entries(&self, channel: &PublicKey, from: u64, limit: usize, max_bytes: usize) -> Vec<Entry>;
    fn fork(&self, channel: &PublicKey) -> Option<(Entry, Entry)>;
    fn blob(&self, id: &BlobId) -> Option<PathBuf>;                // a verified package a held live release names
    fn wanted_blobs(&self, role: Role) -> Vec<(BlobId, u64)>;      // Full: every live release's package it lacks; Light: those asked for
    fn ingest(&self, channel: &PublicKey, entries: Vec<Entry>, why: Why) -> Ingested;
    fn store_blob(&self, id: &BlobId, file: &Path) -> Result<(), String>;
    fn search(&self, query: &Query) -> Vec<Hit>;
}
pub enum Why { Kept, Asked }                     // Kept: a full machine's copy; Asked: a light machine's need or a hit
pub enum Ingested { Stored(u64), NotWanted, Compromised, Failed(String) }
```

The holder verifies everything it ingests (`dist` AGENTS rule 2). `net`
checks only what it must before it bothers the holder (NET-F-23).

### 3.2 Errors

```rust
pub enum NetError {
    Off,                                            // net is turned off
    Unreachable { node: NodeKey, cause: String },   // no path; names the missing relay or DHT
    Refused { node: NodeKey, code: Code },          // the other node said no (4.2)
    Timeout { node: NodeKey },
    TooLarge { node: NodeKey },
    BadObject { node: NodeKey, cause: String },     // NET-F-24
    NotAnchor,                                      // pair_code, remove_device on a replica
    NoFullMachine,                                  // need, query on a light machine with no full one reachable
    BadCode, CodeSpent, Declined,                   // pairing (NET-F-8)
    BadList { cause: String },                      // a device list that does not decode or verify (4.3)
    HasAnchor,                                      // join without replace (NET-F-9)
    Corrupt { path: PathBuf, cause: String },       // a file of <home>/net/
    Io { path: PathBuf, cause: String },
}
```

Every message names the node in its short form (NET-F-32). `Unreachable`
also covers a connection that failed or broke, with the cause. A cause that
nests another error never repeats the node's short key, or a phrase the outer
text has (`NetError::cause_text` gives an error without its `node <short>`).
`CodeSpent` reads "the pairing code was used or has expired; make a new one
on the anchor"; `BadCode` ("not a pairing code") is only a text that does not
parse as a code. A `Corrupt`
`node.key` is a wrong length, a mode that lets others in, or a link.

## 4. Formats

### 4.1 Text forms

```abnf
pair    = "watane-pair:" key ":" secret *( ";" hint )
ticket  = "watane-node:" key *( ";" hint )          ; D2 and later
hint    = "relay=" https-url / "addr=" socket-address
key     = 52zbase32            ; a node key, as log 4.1
secret  = 26zbase32            ; 16 random bytes
```

At most 8 hints and 1024 bytes in all. A hint is used for the first
connection only (PEERS 4.3). Leading and trailing ASCII whitespace of a pasted
code is trimmed; whitespace inside it is a bad code.

### 4.2 Wire

- ALPN `watane/1`. One QUIC bidirectional stream per request.
- The caller writes one CBOR request and finishes the stream. The callee
  writes a CBOR sequence (RFC 8742) and finishes.
- CBOR is deterministic (RFC 8949 4.2.1), arrays only, as in `dist` 4.1. A
  decoder refuses trailing bytes, other major types, and input that does not
  encode back to itself.
- Request: `[op: text, args...]`. Answer: first item `["ok", ...]` or
  `["err", code: text, message: text]`.

| Op | Args | Answer after `"ok"` | Who may call |
|---|---|---|---|
| `hello` | `version: uint` | one item `[version, ops: [text], limits: [uint], devices_seq: uint or null]` | anyone; `devices_seq` only for trusted |
| `heads` | `[channel: bytes(32)]`; empty: all | then one item per channel: `[channel, seq, id: bytes(32), forked: bool]` or `[channel, null]` | trusted |
| `entries` | `channel, from: uint, limit: uint` | then one item per entry: `bytes`, at most 4 MiB in all | trusted |
| `fork` | `channel` | one item: `[a: bytes, b: bytes]` or `null` | trusted |
| `query` | `query` (4.4) | `[hits: [hit]]` | trusted |
| `devices` | `seq: uint` | `[list: bytes or null, addrs: [[node: bytes(32), [text]]]]` | trusted |
| `pair` | `secret: bytes(16)` | `[list: bytes]`, after the participant's answer | anyone (the code decides) |

`query` is decoded strictly (4.4: a `limit` above 200, a `mode` other than
`offers`, `uses` or `sends`, or text that is not text is `bad_request`); the
answer is one item `[hits]` of at most `min(limit, hits per answer)` hits, from
`holdings.search` only, within the request deadline; its answer limit is
32 KiB. The caller refuses more hits than it asked for, or a hit whose
channel or id is not 32 bytes (`BadObject`). `hello` lists the ops this node
answers, `query` included since N4; no caller reads the list to choose an op.

The other two ALPNs are iroh-blobs' and iroh-gossip's own. `serve.rs`
accepts a connection on them only from a trusted node (5.4).

"After `ok`" means: the first item is exactly `["ok"]`, and the items listed
follow it as items of the sequence. A callee never sends more than the answer
byte limit of the op, framing included; a caller reads at most that, and a
longer stream is `TooLarge`. The callee checks, in this order (5.4): the
request length (`too_large`, before it decodes), the encoding
(`bad_request`), an op it does not have (`unknown_op`, also the ops of later
stages and made-up ones), the class (`not_allowed`), then the op. A request
decodes only when it is the deterministic encoding of an array `[op, args...]`
with the args of the table: the reader accepts the shortest form of
unsigned integers, definite byte strings, text, arrays, null and booleans, and
nothing else. A caller of `heads` with a non-empty list expects exactly
those channels back, each once.

The role of a joiner is the role its code was made for, kept on the anchor;
the joiner does not name one (decided in N2: the code text of 4.1 holds no
role, so the joiner cannot know it). `devices` with a `seq` higher than the
callee's own tells the callee that the caller holds a newer list: the callee
answers, then asks the caller `devices` itself (5.5), so a list still moves
by pull only.

Error codes: `not_allowed`, `over_budget`, `too_large`, `bad_request`,
`unknown_op`, `not_found`, `busy`, `bad_code`, `declined`. An unknown op is
`unknown_op`, so older nodes stay compatible. A change to an op is a new op.

**Limits** (`Limits`, defaults). `hello` gives them as `limits: [uint]` in
the order of this table, 10 items: request, answer of `entries`, entries
per answer, entry size, blob size, hits per answer, connect timeout (ms),
request deadline (ms), `pair` deadline (ms), blob deadline (ms).

**Answer byte limits** (the whole stream, CBOR framing included):
`hello` 1 KiB; `heads` 1 MiB; `entries` 4 MiB (`Limits::answer_entries`;
the callee gives the holder `max_bytes` = that minus 4 bytes for `["ok"]`
and 5 per entry asked for, and cuts what the holder returns to fit); `fork`
2 × `MAX_ENTRY` + 64. The error item is at most about 220 bytes and always
fits.

| Limit | Default |
|---|---|
| request size | 256 KiB (a `heads` of 4096 channels is about 170 KiB) |
| answer of `entries` | 4 MiB, at most 1000 entries |
| entry size | `log`'s `MAX_ENTRY` |
| blob size | 64 MiB (spec TECHNICAL 3) |
| hits per answer | 200 |
| connect timeout | 10 s |
| request deadline | 5 s; `pair` 10 min; a blob 120 s |
| sync interval | 10 min (`Limits::sync_interval`, not sent by `hello`) |

The caller refuses to send a request over its own request limit
(`TooLarge`) and applies the connect timeout and the deadline to the whole
call (`Unreachable`, `Timeout`). The callee applies the deadline to the
holder's work too (`busy`).

**Allowances.** Every peer has an allowance: which protocols and ops it may
use, and its budget in bytes, connections and streams. An allowance of zero
for a protocol means the peer may not use it at all. The allowance comes
from the peer's class now, and from a grant from D2 on (a friend gets the
granted channels, their packages, and a byte quota). There is no separate
class check: a class only chooses an allowance.

| Peer | `watane/1` | iroh-blobs | iroh-gossip | Budget |
|---|---|---|---|---|
| trusted | every op | every held package | the own-machines topic | none (an own machine) |
| unknown | `hello`, `pair` | none | none | 2 connections and 2 streams each; 16 connections and 64 KiB/s in and out for all unknown peers together |

Budgets are checked before any work (PEERS A6). Unknown peers share one
pool, so free keys starve only each other. The budget numbers are fields
of `Limits` that `hello` does not send: `unknown_conns_per_peer` 2,
`unknown_streams_per_peer` 2 (open at once), `unknown_conns` 16 (open at
once, all unknown peers), `unknown_bytes_per_sec` 65536 (in and out
together, all unknown peers: a bucket of that many bytes that refills at
that rate; a peer of the pool is over budget while the bucket is empty).
A connection over a connection budget is closed at once; a stream over a
stream budget or the byte budget is answered `over_budget`. Bytes are the
connection's UDP payload bytes in and out from `Connection::stats()`,
polled while the connection lives (5.4).

### 4.3 Signed objects

Signed with the node key: Ed25519 of RFC 8032 5.1.6 (pure), verified with
`verify_strict`, over a domain prefix, a zero byte, and the CBOR of the
unsigned items. The object is the CBOR array of the unsigned items with the
64-byte signature last. The prefixes differ from `log`'s `watane/entry/1`,
so no signature of one kind passes for another.

Besides these forms, the node key signs only what iroh signs with it: the
QUIC handshake and, when the DHT is on, the pkarr record of its addresses.

| Object | Prefix | Items before `sig` |
|---|---|---|
| device list | `watane/devices/1` | `anchor: bytes(32), seq: uint, time: uint, expires: uint, devices: [[node: bytes(32), role: uint]]` |

`devices` is sorted by node key, with no duplicate, and holds the anchor.
`role` is 0 (light) or 1 (full). `expires` is in days, 0 for never. Times
are milliseconds since the Unix epoch. Later stages add the heads file
(`watane/heads/1`, D4) as a second form, and no other.

Stage N1 added golden vectors made with `ed25519-compact` from the seed of
RFC 8032 test 1, as `log` 4.4 does: `tests/it/vectors.rs` holds the bytes of a
list of the anchor (full) and a second key (light), seq 1, time
1767225600000, expires 0, built by hand, and its signature from the oracle.
The signed bytes are the prefix (with its zero byte), then the CBOR array of
the five unsigned items (`85 ...`), as `log` does for entries; the object
is the array of six (`86 ...`). In N1 the code has no list behaviour:
`DeviceList::sign(&seed)` (the anchor's seed, refusing a list that is not
sorted or lacks the anchor) and `DeviceList::verify(bytes)` (strict
decode, the same checks, `verify_strict`); neither signs anything else.

### 4.4 Query and hit

```text
query = [text: text or null, publisher: bytes(32) or null,
         interface: [mode: text, name: text] or null,
         origin: [publisher: bytes(32), slug: text] or null, limit: uint]
hit   = [channel: bytes(32), seq: uint, entry: bytes(32)]
```

The fields are those of `dist` 5.10; `mode` is `offers`, `uses` or
`sends`. `limit` is at most 200. A hit is a hint: the asker pulls its
channel (5.9) and the holder verifies it.

### 4.5 Storage in the home

`net` owns `<home>/net/`: directory 0700, files 0600, every write a temp
file, fsync, rename.

```text
<home>/net/
  node.key     the 32-byte seed of the node key; the rules of log's key file (log 4.7)
  book.json    the switches, relays, last good addresses (at most 4 a machine), last time the anchor was reached
  devices      the newest verified device list (4.3), as bytes
  anchor       on a replica: the anchor's node key (52 characters)
  blobs/       iroh-blobs' store: the packages being moved (5.8)
```

```json
{
  "version": 1,
  "on": false,
  "dht": false,
  "relays": [],
  "addrs": { "<node key>": ["192.168.1.20:4433"] },
  "anchor_seen": 1762780000000
}
```

`on`, `relays` and `dht` are the participant's switches: `set_options` writes
all three (temp, fsync, rename, the other fields kept) and `close` writes none.
`addrs` holds, under the node key in text form, the addresses where a machine
of the device list last answered or called, newest first, at most 4; only
machines of the list have an entry. Measurements and gossip state live in
memory only.

Windows uses the private Windows file operations in log TECHNICAL 4.7
for the directory, node key and pairing records. It validates the node
key's owner, DACL, link count and file type on the opened handle before
reading it. The first creator publishes with a write-through rename that
cannot overwrite a concurrent winner. The blob store inherits the
private directory's access control and staging files have an owner-only
DACL.

## 5. Behaviour

### 5.1 Open

Create `<home>/net/` and the node key if missing (NET-F-1). Read the
files of 4.5; a bad file is `Corrupt`. With `on` false, stop there: no
socket (NET-F-3). Otherwise start the iroh endpoint with the node key,
the three ALPNs, the relays of `book.json` (or none), the DHT only
when `dht` is true, mDNS when the bind address is not loopback, and the relay
server when `relay_server` is set. `reach::services(on, relays, dht, bind)`
decides the services from the effective switches: mDNS when on and the bind
address is not loopback (a loopback node has no local network); exactly the
relays chosen (`RelayMode::Custom`, else `Disabled`: no default relay); the
DHT lookup (publish and resolve) exactly when `dht`. No port mapping, no
probes, no n0 DNS or pkarr relay, and no proxy from the environment.

Which switches win at open (node Q1): `Options.on` false keeps the
endpoint off for that open, whatever `book.json` says. With `Options.on`
true, `book.json` decides `on`, the relays and the DHT; a new home's
`book.json` has all of them off, so a fresh home opened with `on: true` stays
off until `set_options` turns it on. `set_options` writes `on`, `relays` and
`dht` to `book.json`; `close` writes nothing. `limits`, `bind` and
`relay_server` come from `Options` only. A change of `relays`, `dht`, `bind`
or `relay_server` while on rebuilds the endpoint (a running relay server of
the same address keeps running).

### 5.2 Connect and hello

The node key of the other side comes from the QUIC handshake (NET-F-4).
A node keeps one connection to each own machine whose address it knows
(`add_addrs`, the hints of a code, or an address the machine called from),
calls `hello` on it, and syncs (5.6). Other calls use that connection, and
a new one only when none is kept. When it ends the node redials with a
bounded backoff (0.5 s, doubling to 5 s); it never dials an unknown node. Of
two connections to one machine the one the smaller key opened is kept (a
connection made within 2 s of the kept one may replace it; a later one never
does). A node keeps the answer of `hello` for the connection. A trusted caller with a lower `devices_seq` is sent the newer
list (`devices`); a node that sees a higher one asks for it.

### 5.3 Classes

`class_of(node)` is `Trusted` when the node is in the newest verified
device list and the list has not expired (5.5), else `Unknown`. The own
node is trusted. Measurements never change a class.

### 5.4 Serving

Every protocol handler sits in `Gate` (allow.rs): it takes the remote node
key from the connection, rates it (5.3), and checks the peer's allowance for
the connection's ALPN and its connection budget before the inner handler
sees the connection; it counts the connection's bytes (polled from
`Connection::stats()` about every 100 ms while the inner handler runs)
against the peer's byte budget, and closes the connection when the budget
is spent on a protocol that cannot answer `over_budget` (iroh-blobs,
iroh-gossip). N2 builds `Gate` around `watane/1`; N3 puts the iroh-blobs
and iroh-gossip handlers in it.

Every connection and request goes through, in order: the peer's
allowance for its ALPN, the peer's budget (`over_budget`), size
(`too_large`, before decoding), decode (`bad_request`), the allowance for the op
(`not_allowed`), then the op. iroh's router gives each protocol handler
the connection with its remote node key; `net` wraps the iroh-blobs and
iroh-gossip handlers, so a connection whose allowance for that protocol is
zero is closed before they see it, and their bytes count against the
budget. The budget of a protocol that iroh-blobs or iroh-gossip serves cannot be
counted in the handler: the wrapper polls `Connection::stats()` while the
inner handler runs (N2 builds it) and closes the connection when its bytes
exceed the peer's budget. Reads answer only what the holder holds; `net` never fetches for a
caller and never forwards (NET-F-17).

### 5.5 Pairing and the device list

On the anchor (`holdings.is_anchor()`):

1. `pair_code(role)`: 16 random bytes, valid for 10 minutes and one use,
   kept in memory with the role, and the endpoint's current addresses as
   hints. Without a device list, the anchor first signs one with itself
   only, as full.
2. `pair` from a node: compare the secret in constant time. A bad, spent
   or expired code is `bad_code`, and changes nothing; the joiner maps that
   answer to `CodeSpent`, not `BadCode`. A good code is spent at once. Emit `JoinAsked`, and wait for `answer_join` up to the code's
   deadline; no answer is a no (`declined`).
3. On yes: add the node with the code's role, seq + 1, sign, store, answer
   the `pair` request with the list, and send it with `devices` to every
   reachable own machine. `remove_device`, `set_role` and `set_expiry` sign
   a new list the same way.

On the joining machine, `join(code, replace)`: refuse with `HasAnchor`
when an anchor is stored and `replace` is false. Dial the code's key with
its hints, call `pair`, verify the list (signed by that key, names this
node), and store it and `anchor`, and store the address at which the anchor
answered as `learn_addr` would for a listed machine (4.5), so a reopened home
reaches it with no hint; nothing is written to `book.json` for other machines.
With `replace`, the new anchor's list replaces the old one whatever its seq.
After `pair`, the joiner pulls `devices` once from the anchor and keeps the
addresses it passes on (5.10) before `join` returns; a failed pull does not
fail the join (the next newer hello or list push retries).

A machine stores a list from any node only when its stored anchor signed
it, it verifies, and its seq is higher (NET-F-11). Each time this machine
reaches the anchor, it writes `anchor_seen`: a call to the anchor that got
an answer, or a connection from the anchor (its key comes from the
handshake, NET-F-4), so the anchor can always reach an expired machine and
renew it. N2 writes `anchor_seen` into `book.json` and keeps any other field
there as it found it; a new `book.json` has the defaults of 4.5. A list or
`anchor` file that does not verify at open is `Corrupt`.

Who signs: the home whose holder `is_anchor()`. Its list's anchor is its own
node key, it stores no `anchor` file, and its list never expires for it.
A replica rates the anchor like any other machine of its list.

`events()` gives a new receiver on each call; every receiver gets every
later event (node Q2). A receiver that was dropped is forgotten. With `expires` > 0, other
machines stop being trusted when now is later than
`max(list.time, anchor_seen) + expires days` (NET-F-12); the anchor itself
never expires for itself.

A lost anchor cannot be replaced from a replica. The participant restores
the identity on a new home, which signs a new list as a new anchor, and
pairs each machine with `replace` (PEERS 6.1).

### 5.6 Sync

For each own machine that is online, on connect, on a hint (5.7), and every
10 minutes (NET-F-18):

1. Ask `heads`: a full replica asks with an empty list (all); a light
   machine, and the anchor of the list (or a holder with `is_anchor`), ask
   for the channels `holdings.heads(None)` lists (NET-F-21, node Q5).
2. For each channel where the other's seq is higher: `entries` from the
   own seq + 1, in pages, until the other's seq. Check each entry
   (NET-F-23), then `holdings.ingest(channel, entries, Kept)` (a full
   replica) or `Asked` (a light machine and the anchor). `NotWanted` stops that channel. Emit `Pulled`.
3. A `forked` channel: ask `fork` and ingest both entries; `log` marks it.
4. Then fetch the packages of `holdings.wanted_blobs(role)` (5.8).

`net` never takes entries it did not ask for (NET-F-22). One sync per pair
of machines runs at a time; what is asked while one runs is done after it.
A page is asked for at most `max_entries` entries up to the other's seq, and
each page passes ND-14 before the holder sees it. A page whose `ingest` is
anything but `Stored` up to its last entry stops that channel. `Pulled` is
emitted once per channel, with the head the holder stored. A `forked`
channel the holder does not yet know as forked gets `fork`, and each of the
two entries is ingested on its own. A machine that sends a bad object
(`BadObject`: an entry, a head list, a package) is reported with
`Event::Refused` and dropped for the session: it gets no connection, no
sync, and its hints are ignored, until this `Net` is opened again (a turn-off
and on does not clear it). Its connection is closed; what it asks of this
machine is still answered.

### 5.7 Hints over gossip

Every own machine joins one iroh-gossip topic: `BLAKE3("watane/own/1\0" ‖
anchor node key)`, bootstrapped from the reachable machines of the device
list. `told(channel, seq, id)` broadcasts `[channel, seq, id]` (CBOR, 72
bytes). The holder's caller (`node`) calls `told` after an own append and
after an ingest that raised a head. A received message is a hint: when it
decodes exactly (a message of any other shape is dropped), comes from a
trusted machine, and its seq is above the holder's head for that channel,
run step 2 of 5.6 for that channel with the machine that delivered it, else
drop it (NET-F-20). The rule by role (answers node Q3): a full machine acts
on a hint for any channel; a light machine and the anchor only for a channel in
`holdings.heads(None)`, so a hint never makes it hold a channel it does
not. Acting asks the sender for the channel's head first: when the head's
seq is the hint's and its id is not, the hint was forged and nothing is
pulled; when it is not above the holder's, nothing is pulled. The seq and id
of a hint are never trusted.

The machine that delivered a hint is not always the one that told it:
gossip relays, and a neighbour may relay a hint before it has pulled the
head itself. The teller is not on the wire and is never taken from a field.
So when the delivering machine's head is below the hint's seq (or it has
no head of the channel) and the holder's head is too, a machine is asked
again 250 ms later, up to 20 times (5 s in all), as it is pulling the same
head for the same hint. The machine asked again is the next one, by node
key, of the trusted machines this machine has a live link to (the
delivering machine again when it is the only one): the delivering machine
may be one that never holds the channel (a light machine), and each machine
is asked by the same rules, so a forged id or seq is judged as before. The
retries are counted per hint, not per machine: 21 asks in all. When the
head of the machine asked reaches the hint's seq the pull is as in 5.6
step 2. What it has below the hint's seq is pulled on the first and the
last try only, so a forged seq does not keep a pull open for 5 s. After
the 20th retry the hint is dropped (a forged seq ends here, with no ingest
beyond what the machines asked really have); the connect sync and the 10-minute timer still deliver. A newer hint for the
channel replaces the one waiting. Only peers whose
allowance includes the topic can connect on the gossip ALPN, so a stranger
cannot join it.

### 5.8 Blobs

Packages move with iroh-blobs. Its store in `<home>/net/blobs/` holds no
second copy: it adds each package **by reference** to the holder's file,
and keeps only the BLAKE3 tree for verified streaming (about 0.4 % of the
size). The holder's package files are immutable and named by their hash,
which is what a reference needs.

- **Serve:** `net` adds by reference each package that `holdings.blob`
  gives for the held live releases, and removes the reference when the
  holder no longer gives it (a withdrawn package, `dist` DD-12). Only peers
  whose allowance includes the package can fetch it.
- **Fetch:** for each `(id, size)` wanted, download by hash from the own
  machine being synced, verified by iroh-blobs as it streams; the verified
  size must be `size` (and at most `limits.blob`), else the package is a bad
  object; write to a temp file under `<home>/net/`; check BLAKE3 and size
  again; `holdings.store_blob` (a holder that does not keep it gives `Io`,
  `package <hex8> was not stored: <cause>`); the temp file is removed in every case. The
  fetch does not use the store, so it keeps no second copy.

### 5.9 A light machine

A light own machine, and any machine that needs what it does not hold, asks
a full machine.

- **The choice.** The candidates are the machines of the newest device list
  with role `Full` other than this node, minus any dropped for the session
  (5.6) and any with neither a kept connection nor a known address. In
  order: full replicas (not the list's anchor) with a kept connection, the
  other full replicas (dialed), then the anchor. `need`, `need_blob` and
  `query` use the first that answers (node Q5). With no list, or none
  reachable, `need` and `need_blob` give `NoFullMachine`; `query` returns the
  own hits only. A full machine that answers `not_allowed` (this machine
  was removed from the list) gives `Refused { node, code: NotAllowed }` from
  `need` and `need_blob` (the last such answer when several refused and none
  succeeded; for a package, the machine closing the connection as "not
  allowed" counts), not `Unreachable`; a bad object keeps priority. With net
  off, `need` and `need_blob` give `Off`; `query`
  returns the own hits only and `own_heads` is empty.
- `query(q)`: run `holdings.search(q)` and ask `query` (4.2) of one full
  machine (5 s). Return `(own node, own hits)` first, then `(machine, its
  hits)` when one answered. The callee answers from its holder only (ND-6).
  For each remote hit whose channel the holder lacks, or holds at a lower
  seq than the hit, pull it from that machine as `need` does, `Asked`. At
  most `QUERY_PULLS` = 32 distinct channels are pulled per `QUERY_WINDOW` =
  5 s, across all `query` calls of one `Net` (a counter and the time of the
  first pull of the window), so the three calls of one participant query pull
  at most 32 (node Q6). A hit beyond the cap is returned and not pulled; a
  hit is returned whether or not its pull worked, and the holder verifies it
  (node 5.4). `limit` above 200 is cut to 200.
- `need(channel)`: ask the chosen machine `heads` for the channel, then
  `entries` from the holder's seq + 1 in pages to its head (ND-14 on each
  page), `Asked`; a forked head as 5.6 step 3; `Pulled` once. `Ok` when the
  holder's head is the remote's or the remote has nothing newer. A page the
  holder does not take gives an error, not `Ok`: `NotWanted`, `Compromised`
  and `Failed` are `Unreachable { node, cause }` with the holder's cause, no
  drop (a compromised channel is its author's fork, not the machine's lie);
  no other machine is asked then. A bad
  object is `BadObject`, with 5.6's drop and `Refused`. A full machine may
  call it too (it asks another full machine).
- `need_blob(id, size)`: fetch as 5.8 (a package over `limits.blob` is
  `TooLarge` with no transfer; a hash mismatch is `BadObject`) and call
  `holdings.store_blob`. A machine that is not reached, or that does not
  have it, gives way to the next candidate; none connected is
  `NoFullMachine`; some connected and none had it is `Unreachable` naming the
  last of them and why, and the same `Event::Unreachable { node, cause }` (the
  error's node and cause) is emitted, so Machines shows it: at most once for a
  machine until a package fetch from it succeeds, like the keeper's (5.10), so
  failed fetches cannot push a refusal out of Machines' 20 problems.
  A full machine whose own file no longer matches the hash never sends it:
  iroh-blobs checks the file before sending and resets the stream, so the
  light machine sees "not served" (this event), or, when the reset reaches it
  as a cut stream, `BadObject` and `Refused`; which one is timing (S87).
  A local failure (this machine's temp file, or the holder not keeping the
  package) is `Io`, returned as it is: it names no machine, emits no event,
  marks none, and ends the tries.
  `node` calls these before `dist` reads, so a `dist`
  fetch then finds the bytes in its own store.

A light machine with no full machine reachable gives `NoFullMachine`, and
the holder reads from link locations as today.

### 5.10 Reaching machines

In this order (PEERS 8.1): mDNS on the local network; the `addrs` of
`book.json`; addresses passed on in `devices` answers; the hints of a code;
a relay of `relays`; the DHT when `dht` is on. A machine stores each
address that worked: after a connection to, or from, a machine of the list,
the IP address it ran over goes first into `book.json` `addrs` (at most 4),
and at open these are the first known addresses (`add_addrs` adds after
them). A `devices` answer passes on the callee's addresses of the other
machines of its list (at most 32 machines, 4 each), and none when its list
is older than the caller's `seq` (it may name a machine the caller removed);
the caller checks them against its list after taking a newer list of the same
answer (a machine of its list, each text a socket address, else `BadObject`)
and adds them after the ones it knows. The dial adds the chosen relays to the address
it dials, so two machines that share a relay meet there.

With no relay and no DHT, a failure to connect to a machine whose direct
address is not known is `Unreachable` with the cause "no direct path; choose
a relay or turn on the DHT" (NET-F-28); the error and the keeper's
`Event::Unreachable` (once for a machine until it connects again; the event's
cause starts "node <short>: ") carry it. With only a relay chosen the cause is
"no direct path and no answer through the relay; turn on the DHT"; with only
the DHT on, "no direct path and no answer through the DHT; choose a relay";
with both, "no direct path and no answer by the relay or by the DHT". When
this machine knows `n` direct addresses of the machine and none answered, the
cause says that instead and never "no direct path": "did not answer at its
`n` known address(es)" followed by the same advice, once (for example "...;
choose a relay or turn on the DHT", "... or through the relay; turn on the
DHT", "..., by the relay or by the DHT"). A dial gives up
within the request deadline.

`watane-node` with `relay_server` runs iroh-relay for the participant's own
machines (`iroh_relay::server`, HTTP and no TLS: the relay forwards only QUIC
that is encrypted end to end, and a relay on an IP has no certificate a client
would accept; no QUIC address discovery, no metrics server, no ACME), while
net is on. `Net::relay_url()` is `http://<bound address>/` (with port 0 the
bound port); the participant adds exactly this URL to `relays` on the others.
The relay admits only the machines of the newest device list of the serving
node (`class_of` is `Trusted`, so the list's expiry applies); any other
endpoint is denied (an `iroh_relay::server::AccessControl`), so the server
must be one of her machines, paired like the others.

### 5.11 Restore

`own_heads(channel)`: ask `heads` for the own channel of every candidate of
5.9 that answers (in parallel, each within the request deadline) and return
`(node, head)` for each. It ingests nothing and emits no `Pulled`. The caller (`app` through `node`)
shows them as hints; `dist` reads the entries with `need` and confirms the
head only after the participant does (F-APP-6, NET-F-30).

## 6. Security properties

| Property | Enforced by |
|---|---|
| The caller's identity comes from the transport | QUIC handshake; no request field names the caller |
| No unverified object reaches the holder | decode, author, seq order (5.6); BLAKE3 and size (5.8); the holder verifies again |
| No push | pull only (5.6); gossip messages are hints (5.7) |
| No forwarding, no amplification | 5.4 |
| No signing oracle | the node key signs only the form of 4.3 and iroh's own handshake and pkarr record; the participant key is never in `net` |
| Bounded work | byte and connection budgets and size limits before work (4.2, 5.4) |
| Strangers get nothing | the unknown allowance: `hello` and `pair` only (NET-F-14) |
| A stolen code is useless | 128 bits, 10 minutes, one use, and the participant confirms the joiner's short key |
| A replica cannot sign | it holds only its node key; its home is keyless (spec W33) |
| Removed or lost machines lose trust | a newer device list; `expires` when set |
| Nothing ties the participant to a node | no participant key on the wire; no public record unless the DHT is on |

Not protected: a compromised own machine within its trust (it can withhold
and lie about completeness to its own participant), traffic analysis by a
chosen relay, and another program of the user that reads `node.key`.

## 7. Test plan

- No test reaches the internet. Nodes bind to loopback, with mDNS off and
  addresses given directly. S11 starts iroh-relay in the test. The DHT
  switch is tested only as "off sends nothing"; a test of the DHT itself is
  `#[ignore]`.
- `tests/support::MemHoldings` is a holder in memory, filled with entries
  that `watane_log` signs in a temporary directory before the test. A test
  computes its expected answer from those entries, not from `watane_net`.
- The vectors of 4.3 come from `ed25519-compact` (the oracle), as `log`
  8.1 does.
- Each scenario of `FUNCTIONAL.md` 3 is one test, named `s<n>_...`.
- Fault injection: a node that serves a changed byte, an entry of another
  author, a lower head, an oversized answer, unasked entries, a forged
  hint, and a flood of fresh keys.

## 8. Integration

The crate `node` (spec TECHNICAL 2) joins `dist` and `net`; neither knows
the other. It implements `Holdings` over a `Dist` handle, opens `Net` with
it, calls `told` after an own append and a raised head, and calls `need`
and `need_blob` on a light machine before a `dist` read. It owns the
program `watane-node`: a keyless home (spec W33) that runs as a full or
light machine and, with a flag, as a relay. `app` opens `node` in the
participant's home. The step 8 design slice writes `node`'s own documents.

## 9. Build plan

| Stage | Build | Done when |
|---|---|---|
| N1 | crate, node key, pairing code text, wire codecs and limits, `hello`, `heads`, `entries`, `fork` between two local nodes, the 4.3 vectors; pin iroh, iroh-blobs and iroh-gossip | S1, and `heads` and `entries` between two trusted test nodes |
| N2 | device list, pairing, classes, allowances and budgets | S2, S3, S4, S9 (budget part), S10 |
| N3 | sync, gossip hints, blobs | S5, S6, S7, S9 |
| N4 | light machines, restore, reach (mDNS, addresses, relay, the DHT switch) | S8, S11, S12 |

N1 notes (built):

- N1 builds the endpoint with `presets::Minimal`, `RelayMode::Disabled`,
  `clear_address_lookup`, `PortmapperConfig::Disabled`,
  `NetReportConfig::minimal()` (no HTTPS or captive-portal probes) and one
  IP socket at `Options::bind`. A node dials the addresses of `add_addrs`
  and nothing else. `Options::relays`, `dht` and `relay_server` exist and N1
  ignores them until N4 (wired in part 2, below). iroh-gossip adds its own in-memory address lookup
  to an endpoint at spawn (N3); that is no network service.
- `Net::open` makes no runtime and no socket while `on` is false; the
  runtime is made on the first turn-on. `set_options` with `on` false
  shuts the router and the endpoint down.
- A trusted node is one of `Options::trusted` (and the own node). N2
  replaces it with the device list.
- The pairing code text (4.1) is built and parsed (`PairCode`, `Hint`);
  nothing uses it before N2. `Ticket` is D2.
- Tests: `node.rs` (S1), `code.rs`, `vectors.rs`, `serve.rs` (hello, heads,
  entries, fork, unknown caller, ops of later stages, limits), `faults.rs`
  (the caller's checks, deadlines), `keyfile.rs`, `q1.rs`, and
  `no_default_services` in `lib.rs`.

N2 notes (built):

- `allow.rs`: `Gate<P>` wraps `Serve` for `watane/1`. It renews `anchor_seen`
  when the peer is the anchor (ND-17), rates the peer, closes a connection
  whose class has no allowance for its ALPN or that is over a connection
  budget, and polls `Connection::stats()` every 100 ms for an unknown peer.
  Streams of an unknown peer are counted in `Serve`, which polls the
  connection's bytes first, so a stream over the stream or byte budget gets
  `over_budget`. The byte bucket may run into debt of one second's worth, so a
  burst keeps an unknown peer out for a while; bytes of the handshake (before
  the Gate counts the connection) are not charged. A request of 16 KiB on a
  4 KiB/s bucket may itself be refused, because most of it has arrived by the
  time its stream is accepted.
- `pairing.rs`: the stored list (`devices`, `anchor`, `anchor_seen` in
  `book.json`, every write temp, fsync, rename, 0600), codes, `sign_next`
  (one function signs every new list), `accept_list` (forward only, the
  stored anchor's only), `join`'s `install`. The code lives as long as
  `pair_deadline` of the anchor at the time it was made; the same limit is
  the wait for `answer_join`. `pair` has its own stream deadline,
  `pair_deadline` plus a second.
- `devices(seq)` answers the list only when the callee's seq is higher; a
  higher seq in the request makes the callee pull from the caller. A new list
  is pushed this way to every other machine whose address is known
  (`add_addrs`, and the hints of a code for the anchor); N2 learns no address
  from a connection. `hello` sends no list, but a caller that sees a higher
  `devices_seq` of a trusted node asks that node for it and stores it under
  5.5 (a failed pull changes nothing and does not fail `hello`); N3 still syncs
  a machine that was offline on connect.
- Tests: `pair.rs` (S2, S3, S4, S10), `budget.rs` (S9), unit tests in
  `allow.rs` and `pairing.rs` (AC7, AC9, storage). Raw or fake callers are
  made trusted with `support::node_trusting`, which signs a list with the
  node's own key before it opens. A raw client that does not close its
  connection holds one of the two connections of its key until the idle
  timeout, so a test uses a new key for each call.

N3 notes (built):

- Kept connections (5.2): `sync.rs` runs a keeper task while on, ticking every
  250 ms. For each trusted machine of the list it redials when no connection is
  kept and an address is known (the machine of the larger key waits 1 s after it
  first knows the address, so that two dials rarely cross), and asks for a
  sync when the last full one is older than `Limits::sync_interval` (default
  10 min; a field `hello` does not send, so a test can shorten it). A connection
  that a trusted machine opened is kept the same way (`Serve::accept` hands it
  over) and its address is learned (first in line). Before it is kept a
  connection answers `hello`; `PeerUp` follows, then the sync; `PeerDown`
  when it ends. The machine that dialed also serves the streams the other
  opens on it.
- Gossip (5.7): `gossip.rs` joins the topic when the list names an anchor and
  rejoins when the list's seq changes, with the machines whose address is
  known as bootstrap, and joins machines that become reachable later. An
  in-memory address lookup (`MemoryLookup`) feeds `add_addrs` and learned
  addresses to gossip; with iroh-gossip's own it makes two in-memory
  services on the endpoint, still no network service (`no_default_services`).
  `told` waits up to 20 s for a neighbor on the topic and then broadcasts the
  hint; a hint lost before that is covered by the sync on connect and the timer.
  The CBOR of a hint is 70 to 72 bytes by the size of the seq.
- Blobs (5.8): the iroh-blobs store is opened in `<home>/net/blobs/` (0700)
  when net turns on. The ids net may serve are learned on request: the
  provider events of iroh-blobs are intercepted for `get` and `get_many`; for
  each hash `holdings.blob` must give a path, which is added by reference
  (`ImportMode::TryReference`) under the tag `package-<hex>`; the hash of
  that file must be the one asked. A hash the holder does not give (any more)
  is refused and its tag deleted (the store's garbage collector is not public
  in iroh-blobs 0.105, so the tree data of a withdrawn package stays until the
  store is rebuilt; nothing is served from it). Push and observe requests
  are off. A fetch asks the verified size first (a lie about the size fails
  the proof), then the blob; it holds at most `size` (at most 64 MiB) bytes in
  memory before the temp file.
- Tests: `sync.rs` (S5, S6, S7, S9, the timer, light and fork rules, the gate
  around blobs and gossip, junk on the topic), unit tests in `gossip.rs` (hint
  bytes, topic id, hint rule) and `allow.rs` (the allowances by ALPN).
  `MemHoldings` keeps its chains and packages behind locks; `pair_into` now
  waits for `PeerUp` on both machines and for the holders to go quiet.
- S9 split: unasked entries and the answer from the holder only are here;
  "a light machine's query reaches the server and no third node" waits for
  `query` in N4.
- `n0-future` (already in the lock file through iroh) is a dependency of this
  crate only to read the gossip receiver, which is a `Stream`; iroh-gossip
  re-exports nothing to name that trait.

N4 part 1 notes (built, S67):

- A fetch that fails gives a cause in watane's words (NET-F-32), never iroh's
  text: a request the serving machine cut or refused reads "not served: it
  does not have it, or its copy failed its check"; any other transport
  failure reads "could not be read: the connection failed" (`failed()` in
  `blobs.rs`); `need_blob` puts "package <hex8> was not fetched: " before
  it, and the error names the machine (S80). A connection the machine closed
  as "not allowed" is `Refused { code: NotAllowed }`.

- Light machines (5.9): `light.rs` holds the choice of a full machine (`order`,
  a pure function over the list, this node and the set of connected machines),
  `need`, `need_blob`, `query`, `own_heads` and the window counter (`Window`,
  `QUERY_PULLS` 32, `QUERY_WINDOW` 5 s, in `Core`, tested with an injected
  clock). A machine with neither a kept connection nor a known address is not
  a candidate: reach by mDNS, `book.json` addresses, relay, the DHT and S11 is
  S68, so a test gives the light machine the server's address with
  `add_addrs`.
- The `query` op (4.2): `Request::Query`, codec unit tests in `wire.rs`; a
  trusted caller only; an unknown caller gets `not_allowed` before the holder.
- The anchor (5.6, 5.7): a machine whose list names it as anchor, or whose
  holder says `is_anchor`, syncs as a light machine: `heads(None)` of its
  holder only, `Asked`, hints for held channels only, `wanted_blobs(Light)`.
  Only a full replica asks for all and ingests `Kept`.
- `own_heads` hints only; it calls no `ingest` and emits no `Pulled`.
- Follow-ups (NET-F-32): a wanted package over `limits.blob` in a sync is
  skipped with no transfer and `Event::Unreachable` ("package <hex8> is <n>
  bytes, over this machine's limit of <m>"; hex8 is the first four bytes of
  the hash); a hash mismatch says "package <hex8> from <short key> does not
  match its hash"; every `Refused` cause contains the node's short key (the
  event adds "node <short>: " when the cause lacks it).
- No `Limits` field is new; `hello`'s ops now name `query`.
- Tests: `tests/it/light.rs` (S8, S9 query part, S12, the anchor, the causes),
  unit tests in `light.rs` and `wire.rs`.
- Turning net off (`set_options` with `on` false, `close`) ends every kept
  connection with an application close code, then waits at most `CLOSE_WAIT`
  (500 ms, `lib.rs`) for the router and endpoint to finish closing; the keeper
  and the blobs hook are aborted first, the router is told to stop at once and
  ends by itself on the runtime if the wait runs out. Then the blob store is
  shut down (`Store::shutdown`, which closes its redb database) and awaited at
  most `STORE_WAIT` (5 s, `lib.rs`): the blob store is closed before a rebuild
  or `Net::open` reopens its folder, because redb locks the file exclusively
  (on Windows the late close failed the reopen). A timeout there is ignored;
  the reopen reports its own error.

N4 part 2 notes (built, S68):

- Services by configuration (5.1): `reach::services` is a pure function,
  unit tested for each case; the endpoint builder applies it. mDNS is off on
  a loopback bind, so no test builds an endpoint with mDNS or the DHT on:
  they are tested by configuration only. The DHT lookup publishes unfiltered
  (the participant chose it, and a machine with no relay is found only by its
  addresses); the lookup's default would publish a relay address only.
- The book rules (4.5, 5.1): `open` takes `on`, relays and DHT from
  `book.json` when `Options.on` is true; `set_options` writes them and
  `close` does not. `book.json` is written through `reach::update_book`
  (one lock, the other fields kept), also for `anchor_seen`. Test helpers
  open a home and then call `set_options`.
- The relay server (5.10) admits own machines only (W32, NET-F-29); S11 pairs the
  server, and a stranger on the same relay is refused.
- The relay server (5.10) is HTTP; `relay_url` is `http://<addr>/`. node's
  text says `https://<addr>/` (a question for node).
- The cause (5.10, `reach::no_path_cause`), the deadline: a dial gives up at
  `min(connect_timeout, deadline)`; a call whose deadline ends while it is
  connecting is `Unreachable`, not `Timeout`.
- Stored addresses are written when a connection to a machine of the list is
  made or accepted, with the address of the selected IP path (none for a
  relay path), and only when they change.
- `devices` answers carry addresses (4.2); `pairing::take_answer` checks and
  adds them. They are asked for when the list changes (a push, a pull), not
  at every connect.
- Tests: `tests/it/reach.rs` (S11 with an in-process relay on 127.0.0.1, the
  redial from the book, the addresses of a `devices` answer), unit tests in
  `reach.rs` (services, cause, book), `lib.rs` (the switch rules,
  `no_default_services`) and `wire.rs` (the codec).

S13 runs from N1 on. `node` and the `app` views follow N4 in step 8; J8
closes the step.

## 10. Provides to other subsystems

| To | What |
|---|---|
| `node` | `Net`, `Holdings`, `Why`, `Ingested`, `Role`, `Query`, `Hit`, `HeadInfo`, `Event`, `DeviceList` |
| `app` (through `node`) | turn on and off, pairing code and join, `answer_join`, devices, roles, expiry, relays, the DHT switch; `Event` for the views |

## 11. Needs from other subsystems

| From | What |
|---|---|
| `log` | `PublicKey` (and its text form), `Entry::decode`, `Entry` accessors, `EntryId`, `MAX_ENTRY`. No `Log`. A keyless open for replicas (`log` 12, item 13), used by `dist`. |
| `dist` | through `node`: the holder methods of 3.1, a record reason `kept`, a keyless open (`dist` Q13), reads that do not hold the store lock across network calls |
| `node` | the `Holdings` implementation; `told`, `need`, `need_blob` calls; `watane-node` |
| `app` | the views and confirmations: turn on, the join question with the short key, devices and roles, expiry, relays, the DHT switch with its consent text, restore hints |
| `host` | reserve `<home>/net/` beside `log/`, `dist/` and `app/` (done) |

## 12. Questions for the step 8 design slice

| # | Question | Proposal |
|---|---|---|
| Q1 | 5.4 and 5.8 rest on two library features: adding a file to the iroh-blobs store by reference, and wrapping a protocol handler in iroh's router. | Answered by N1 against iroh 1.4.1, iroh-blobs 0.105.0 and iroh-gossip 0.102.0: both are present. `ImportMode::TryReference` (`api::blobs::AddPathOptions`) leaves a 1 MiB file out of the store (`q1a_blobs_by_reference_keep_no_second_copy`; the store's redb file is preallocated at about 1 MiB whatever it holds, so the test measures growth and the `data/` directory, and a `Copy` import as the control). A wrapper `Gate<P: ProtocolHandler>` that checks `Connection::remote_id()` before it calls the inner `accept`, and forwards `shutdown`, works around `BlobsProtocol` and around `Gossip`, both in one `Router`; a refused node's download fails and the inner handler is not called (`q1b_a_wrapper_admits_a_connection_by_its_remote_key`). |
| Q2 | What does a light machine keep after a search pulled a channel? | **Decided (S59).** `Asked` channels stay in the store with no record, like a channel read for a provider today (`dist` 5.11): not held, so not in `heads(None)`, not synced and not refreshed (`dist` 5.12, DD-38). A search shows one only when `node` verified the remote hit against it (`node` 5.4). An install from such a hit gives the channel a `kept` record on the replica, so it stays current through the full machines (`node` 5.3, NO-6). `Log::forget` cleans up at D5. |
| Q4 | (node Q4) Do `Options` and `Limits` derive `Clone`, `Debug` and `Default`? | **Answered (S62).** Yes, both, since N1; `Default` is `on` false, no relays, `dht` false, no relay server, no bind, and the limits and budgets of 4.2 (section 3). |
| Q5 | (node Q5, the N2 part) The anchor is `full` in the device list, but keeps only what the participant's acts there chose (node NO-8). Which machine is a full machine for NET-F-21 and for a light machine's `query` and `need`? | **Answered for N2 (S62).** The list keeps the anchor as `full` (4.3 unchanged; it is the role a lost anchor's restore needs). NET-F-21's "keep everything" binds full replicas only, not the anchor. A light machine asks a full replica first and the anchor only when no full replica is reachable; built in 5.9 (S67). |
| Q3 | How does the app show own machines and their freshness? | **Decided (S59), `app/TECHNICAL.md` 9.8 (stage G).** A machines view with each machine's short key, role, this machine and anchor marks, online or last seen (`PeerUp`, `PeerDown`), and the last pull from it (`Pulled`), kept in memory from `events()`; refusals and unreachable machines with their causes. No heads count: it needs no new op, and the last pull says how current a machine keeps this one. The join question, the relay and the DHT switch go through the app's consent view with the texts of `node` 7. |
| Q6 | (node Q3) Does a hint make a light machine pull any channel any machine tells? | **Answered (N3).** No: a full machine acts for any channel; a light machine only for a channel in `holdings.heads(None)` (5.7). |

## 13. Decisions of this subsystem

| # | Decision | Reason |
|---|---|---|
| ND-1 | `net` stores no channel; the holder (`dist`, through `node`) holds, verifies and stores. | One store, one verifier; `log` stays opened only by `dist`. |
| ND-2 | A blocking API over an internal tokio runtime; the holder is called only from blocking threads. | iroh needs tokio; `dist` keeps no runtime (`dist` AGENTS rule 7). |
| ND-3 | The node key is iroh's endpoint key, in its own file. | One key per node; the participant key never leaves `log`. |
| ND-4 | The device list is signed by the anchor's node key; pairing is a code with the anchor's key, hints and a 128-bit secret, and the participant confirms the joiner's short key. | No new form for the participant key; QUIC proves the anchor, so a long secret needs no PAKE; the confirmation stops a stolen code. |
| ND-5 | Full and light machines; no shards. | A personal keep set fits on one always-on machine (PEERS A8). |
| ND-6 | Pull only, no forwarding. | Nobody can fill another's store, and no request can be amplified. |
| ND-7 | iroh-blobs for packages, iroh-gossip for hints, from D1. | Decided by the human (proposal Q8): verified streaming and gossip as shipped, not written here. |
| ND-8 | Head vectors, no set reconciliation yet. | A thousand channels is about 75 KiB. Undo: a new op at D5. |
| ND-9 | Nothing reachable by default: no relay, no DHT. | V4, W32; decided by the human (proposal Q2). |
| ND-10 | Device list expiry is a setting; 0, the default, never expires. | Decided by the human (proposal Q14). |
| ND-11 | Limits in bytes and connections; one shared pool for unknown callers. | Keys are free; a count of requests says nothing about cost. |
| ND-12 | `Options::bind` chooses the bind address (default `0.0.0.0:0`). N1's `Options::trusted` stand-in is gone since N2: the device list alone decides the class. | Tests bind to loopback only. |
| ND-15 | `pair` takes the secret only; the role is the code's, kept on the anchor (N2). | The code text holds no role; a role the joiner names could only be checked against the code's. Undo: a new op. |
| ND-16 | Budgets in `Limits`, not on the wire; byte budgets polled from `Connection::stats()` in `Gate`, around every protocol (N2). | One place enforces every protocol's allowance, including the ones `net` does not parse (net Q1). |
| ND-17 | Expiry is reset by any authenticated contact with the anchor, either way. | Otherwise an expired machine would refuse the anchor that comes to renew it. |
| ND-18 | The relay server speaks HTTP, and `relay_url` is `http://<addr>/`; clients dial the relays of `relays` for any machine they dial. | A relay on an IP has no certificate a client would accept, and the relay sees only QUIC that is encrypted end to end. The participant adds one relay URL to all her machines, so it is the other side's relay too (S68). |
| ND-19 | With the DHT on, a machine publishes its addresses unfiltered. | The participant chose the DHT; with no relay, an address filter that publishes relay addresses only would publish nothing (S68). |
| ND-13 | One connection per call in N1 and N2. From N3 an own machine keeps one connection to each other own machine, calls `hello` on it and syncs; a call to a machine with no kept connection still opens a connection for itself. | Sync, hints and `PeerUp`/`PeerDown` need a standing connection (5.2). |
| ND-14 | A caller checks what it can without the holder: each entry decodes, is of the channel's author, starts at `from`, chains (`prev` is the id before) and is at most `limit` (at most 1000) in number; a `heads` answer holds each channel once and, when asked for some, exactly those; a `fork` pair is two entries of the channel at one seq with two ids. Anything else is `BadObject`. | NET-F-23, NET-F-24. The callee passes on what its holder returns, so a faulty holder shows at the caller. |
