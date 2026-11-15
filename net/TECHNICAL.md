# net: technical specification

How `net` does what `FUNCTIONAL.md` says, for stage D1 of `spec/PEERS.md`
(step 8, own machines). Requirements are cited as `NET-F-<n>`, the
architecture as PEERS sections. Section 13 lists the decisions made here;
section 12 the questions left for the step 8 design slice.

No crate exists yet: stage N1 (section 9) creates it and adds it to the
workspace.

## 1. Crate

| Item | Value |
|---|---|
| Package | `watane-net`, lib `watane_net`, no binary |
| Edition, toolchain | 2024, stable, `rust-version = "1.91"` |
| Lints | `unsafe_code = "forbid"`; `cargo clippy --all-targets -- -D warnings` is clean |
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
    wire.rs       requests, answers, CBOR codecs, limits (4.2)
    devices.rs    DeviceList: encode, sign, verify (4.3); pairing (5.5)
    serve.rs      the handler of `watane/1`; the wrappers of the iroh-blobs and iroh-gossip handlers (5.4)
    allow.rs      allowances: what each peer may use, and its byte, connection and stream budget (4.2)
    sync.rs       heads, pulls, the gossip topic, blobs (5.6 to 5.8)
    reach.rs      address book, mDNS, relays, the DHT switch (5.10)
  tests/
    support/mod.rs   MemHoldings (an in-memory holder), signed fixture entries, local nodes
    node.rs  pair.rs  serve.rs  sync.rs  light.rs  reach.rs  restore.rs
```

Inner modules are private. Only `lib.rs` exports.

## 2. Dependencies

Versions as on crates.io on 2026-11-14. Stage N1 pins the exact set that
compiles together and records it here.

```toml
[dependencies]
watane-log = { path = "../log" }   # PublicKey, Entry, EntryId: decode and verify only
iroh = "1.4"                       # QUIC dialed by Ed25519 key, hole punching, relays, mDNS, pkarr
iroh-blobs = "*"                   # BLAKE3 verified streaming; N1 pins the release that matches iroh 1.4
iroh-gossip = "*"                  # HyParView and Plumtree; N1 pins the release that matches iroh 1.4
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
iroh-relay = "1.4"                 # a relay in the test, for S11
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

pub struct Options {
    pub on: bool,                     // false: no socket at all (NET-F-3)
    pub relays: Vec<url::Url>,        // chosen by the participant; empty by default
    pub dht: bool,                    // publish and look up addresses in the Mainline DHT; false by default
    pub relay_server: Option<std::net::SocketAddr>, // watane-node only: run iroh-relay here (NET-F-29)
    pub limits: Limits,               // section 4.2 defaults
}

pub struct Net { /* Send + Sync; cheap to clone */ }

impl Net {
    pub fn open(home_net: &Path, holdings: Arc<dyn Holdings>, options: Options) -> Result<Net, NetError>;
    pub fn set_options(&self, options: Options) -> Result<(), NetError>; // on/off, relays, dht
    pub fn close(self);

    pub fn node_key(&self) -> NodeKey;
    pub fn class_of(&self, node: &NodeKey) -> Class;

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
    HasAnchor,                                      // join without replace (NET-F-9)
    Corrupt { path: PathBuf, cause: String },       // a file of <home>/net/
    Io { path: PathBuf, cause: String },
}
```

Every message names the node in its short form (NET-F-32).

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
connection only (PEERS 4.3).

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
| `hello` | `version: uint` | `[version, ops: [text], limits: [uint], devices_seq: uint or null]` | anyone; `devices_seq` only for trusted |
| `heads` | `[channel: bytes(32)]`; empty: all | then one item per channel: `[channel, seq, id: bytes(32), forked: bool]` or `[channel, null]` | trusted |
| `entries` | `channel, from: uint, limit: uint` | then one item per entry: `bytes`, at most 4 MiB in all | trusted |
| `fork` | `channel` | `[a: bytes, b: bytes]` or `null` | trusted |
| `query` | `query` (4.4) | `[hits: [hit]]` | trusted |
| `devices` | `seq: uint` | `[list: bytes or null, addrs: [[node: bytes(32), [text]]]]` | trusted |
| `pair` | `secret: bytes(16), role: uint` | `[list: bytes]`, after the participant's answer | anyone with a valid code |

The other two ALPNs are iroh-blobs' and iroh-gossip's own. `serve.rs`
accepts a connection on them only from a trusted node (5.4).

Error codes: `not_allowed`, `over_budget`, `too_large`, `bad_request`,
`unknown_op`, `not_found`, `busy`, `bad_code`, `declined`. An unknown op is
`unknown_op`, so older nodes stay compatible. A change to an op is a new op.

**Limits** (`Limits`, defaults):

| Limit | Default |
|---|---|
| request size | 256 KiB (a `heads` of 4096 channels is about 170 KiB) |
| answer of `entries` | 4 MiB, at most 1000 entries |
| entry size | `log`'s `MAX_ENTRY` |
| blob size | 64 MiB (spec TECHNICAL 3) |
| hits per answer | 200 |
| connect timeout | 10 s |
| request deadline | 5 s; `pair` 10 min; a blob 120 s |
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
pool, so free keys starve only each other.

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

Stage N1 adds golden vectors made with `ed25519-compact` from the seed of
RFC 8032 test 1, as `log` 4.4 does.

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
  book.json    the switches, relays, last good addresses, last time the anchor was reached
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

Measurements and gossip state live in memory only.

## 5. Behaviour

### 5.1 Open

Create `<home>/net/` and the node key if missing (NET-F-1). Read the
files of 4.5; a bad file is `Corrupt`. With `on` false, stop there: no
socket (NET-F-3). Otherwise start the iroh endpoint with the node key,
the three ALPNs, mDNS, the relays of `book.json` (or none), the DHT only
when `dht` is true, and the relay server when `relay_server` is set.

### 5.2 Connect and hello

The node key of the other side comes from the QUIC handshake (NET-F-4).
A node calls `hello` on each new connection and keeps the answer for the
connection. A trusted caller with a lower `devices_seq` is sent the newer
list (`devices`); a node that sees a higher one asks for it.

### 5.3 Classes

`class_of(node)` is `Trusted` when the node is in the newest verified
device list and the list has not expired (5.5), else `Unknown`. The own
node is trusted. Measurements never change a class.

### 5.4 Serving

Every connection and request goes through, in order: the peer's
allowance for its ALPN, the peer's budget (`over_budget`), decode
(`bad_request`), size (`too_large`), the allowance for the op
(`not_allowed`), then the op. iroh's router gives each protocol handler
the connection with its remote node key; `net` wraps the iroh-blobs and
iroh-gossip handlers, so a connection whose allowance for that protocol is
zero is closed before they see it, and their bytes count against the
budget. Reads answer only what the holder holds; `net` never fetches for a
caller and never forwards (NET-F-17).

### 5.5 Pairing and the device list

On the anchor (`holdings.is_anchor()`):

1. `pair_code(role)`: 16 random bytes, valid for 10 minutes and one use,
   kept in memory with the role, and the endpoint's current addresses as
   hints. Without a device list, the anchor first signs one with itself
   only, as full.
2. `pair` from a node: compare the secret in constant time. A bad, spent
   or expired code is `bad_code`, and changes nothing. A good code is spent
   at once. Emit `JoinAsked`, and wait for `answer_join` up to the code's
   deadline; no answer is a no (`declined`).
3. On yes: add the node with the code's role, seq + 1, sign, store, answer
   the `pair` request with the list, and send it with `devices` to every
   reachable own machine. `remove_device`, `set_role` and `set_expiry` sign
   a new list the same way.

On the joining machine, `join(code, replace)`: refuse with `HasAnchor`
when an anchor is stored and `replace` is false. Dial the code's key with
its hints, call `pair`, verify the list (signed by that key, names this
node), and store it and `anchor`.

A machine stores a list from any node only when its stored anchor signed
it, it verifies, and its seq is higher (NET-F-11). Each time this machine
reaches the anchor, it writes `anchor_seen`. With `expires` > 0, other
machines stop being trusted when now is later than
`max(list.time, anchor_seen) + expires days` (NET-F-12); the anchor itself
never expires for itself.

A lost anchor cannot be replaced from a replica. The participant restores
the identity on a new home, which signs a new list as a new anchor, and
pairs each machine with `replace` (PEERS 6.1).

### 5.6 Sync

For each own machine that is online, on connect, on a hint (5.7), and every
10 minutes (NET-F-18):

1. Ask `heads`: a full machine asks with an empty list (all); a light
   machine asks for the channels `holdings.heads(None)` lists.
2. For each channel where the other's seq is higher: `entries` from the
   own seq + 1, in pages, until the other's seq. Check each entry
   (NET-F-23), then `holdings.ingest(channel, entries, Kept)` (full) or
   `Asked` (light). `NotWanted` stops that channel. Emit `Pulled`.
3. A `forked` channel: ask `fork` and ingest both entries; `log` marks it.
4. Then fetch the packages of `holdings.wanted_blobs(role)` (5.8).

`net` never takes entries it did not ask for (NET-F-22). One sync per pair
of machines runs at a time.

### 5.7 Hints over gossip

Every own machine joins one iroh-gossip topic: `BLAKE3("watane/own/1\0" ‖
anchor node key)`, bootstrapped from the reachable machines of the device
list. `told(channel, seq, id)` broadcasts `[channel, seq, id]` (CBOR, 72
bytes). The holder's caller (`node`) calls `told` after an own append and
after an ingest that raised a head. A received message is a hint: when its
seq is above the holder's, run step 2 of 5.6 for that channel with the
machine that delivered it, else drop it (NET-F-20). Only peers whose
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
- **Fetch:** for each `(id, size)` wanted, download by hash from an own
  machine that holds it, verified by iroh-blobs as it streams; refuse more
  than `size` bytes; export to a temp file; `holdings.store_blob`; then
  replace the store's copy by a reference to the holder's file.

### 5.9 A light machine

- `query(q)`: run `holdings.search(q)` and ask `query` of one reachable full
  machine (5 s). Return each node's hits apart. For a hit whose channel the
  holder lacks, pull it from that node with `Asked`, at most 32 channels per
  query (PEERS 8.2). The holder verifies each hit as it does a local one.
- `need(channel)`, `need_blob(id, size)`: pull from a full machine, as 5.6
  and 5.8, with `Asked`. `node` calls these before `dist` reads, so a
  `dist` fetch then finds the bytes in its own store.

A light machine with no full machine reachable gives `NoFullMachine`, and
the holder reads from link locations as today.

### 5.10 Reaching machines

In this order (PEERS 8.1): mDNS on the local network; the `addrs` of
`book.json`; addresses passed on in `devices` answers; the hints of a code;
a relay of `relays`; the DHT when `dht` is on. A machine stores each
address that worked. With no relay and no DHT, a failure to connect across
networks is `Unreachable` with the cause "no direct path; choose a relay or
turn on the DHT" (NET-F-28).

`watane-node` with `relay_server` runs iroh-relay for the participant's own
machines; the participant adds its URL to `relays` on the others.

### 5.11 Restore

`own_heads(channel)`: ask `heads` for the own channel of every reachable
full machine and return each answer. The caller (`app` through `node`)
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
| Q1 | 5.4 and 5.8 rest on two library features: adding a file to the iroh-blobs store by reference, and wrapping a protocol handler in iroh's router. Both are expected; neither is checked against the releases N1 pins. | N1 checks both against the pinned releases; if either is gone, N1 records the replacement here before N3. |
| Q2 | What does a light machine keep after a search pulled a channel? | `Asked` channels stay, like a channel read for a hit today (`dist` 5.11, no record); `Log::forget` cleans up at D5. |
| Q3 | How does the app show own machines and their freshness? | A devices view with each machine's short key, role, last seen, and its heads count; part of the `app` design. |

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
