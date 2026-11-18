# node: technical specification

How `node` does what `FUNCTIONAL.md` says. Requirements are cited as
`NODE-F-<n>`; `net`'s as `NET-F-<n>`; sections of a sibling as
`dist 5.12`, `net 3.1`. Section 14 lists the decisions made here, section
13 the questions left open.

Stages 1 to 3 (section 9) are built: the crate is a workspace member with
`Node`, `DistHoldings`, `convert.rs`, the wrappers with `told` and `need`,
`installed`, `search`, `join`, the restore hints and `words.rs`, and the program
`watane-node` (section 6, `src/bin/watane-node.rs`). Where the code differs from the text below, the
text says so.

## 1. Crate

| Item | Value |
|---|---|
| Package | `watane-node`, lib `watane_node`, binary `watane-node` |
| Edition, toolchain | 2024, stable, `rust-version = "1.91"` |
| Lints | `unsafe_code = "forbid"`; `cargo clippy --all-targets -- -D warnings` is clean |
| Threads | No runtime of its own. The library API blocks, like `dist`'s and `net`'s: call it from a blocking thread, never from inside a tokio runtime (`net` 1). `net` calls the holder of section 4 from its blocking threads; no holder method calls `net` or reads from a web host (RV-06). The program runs one thread that prints `net`'s events and waits for a signal on the main thread. |
| Network | None of its own. `net` makes every connection between machines; `dist` every read from a web host. |
| Opens | Nothing directly: `Dist` opens `<home>/log/` and `<home>/dist/`, `Net` opens `<home>/net/`. The program reads only whether `<home>/home.json` exists, and the `relays` and `dht` through `Net::options()` (6.1). |

```text
node/
  Cargo.toml
  src/
    lib.rs        Node, Options, Machine, NodeHit, OwnHint; re-exports; the only `pub` items (section 3)
    error.rs      NodeError
    holdings.rs   DistHoldings: net's Holdings over a Dist handle (section 4)
    convert.rs    net Query and Hit to dist Query and Hit and back; PackageId and BlobId (4.3)
    calls.rs      the wrappers: told after a raised head, need before a light read (5.1 to 5.3)
    search.rs     local and remote hits (5.4)
    words.rs      the consent texts that the app and the program both show (7)
    bin/watane-node.rs   the program (section 6)
  tests/
    support/mod.rs   homes, signed fixture channels, local nodes on loopback
    holdings.rs  open.rs  own.rs  light.rs  restore.rs  program.rs
```

Inner modules are private. Only `lib.rs` exports.

## 2. Dependencies

```toml
[dependencies]
watane-dist = { path = "../dist" }   # the holder; log and pkg types come through its re-exports
watane-net = { path = "../net" }     # Net, Holdings and the types of net 3
thiserror = "2.0"
tokio = { version = "1", features = ["rt", "signal"] }   # the program: a small runtime on the main thread that only waits for SIGINT or SIGTERM
url = "2.5"                          # the program: a relay URL, as net's options hold it

[dev-dependencies]
tempfile = "3.27"
serde_json = "1.0"                   # tests read the files of a home (book.json, follows.json)
tiny_http = "0.12"                   # a web host for S2 and S5, as dist's tests use it
watane-pkg = { path = "../pkg" }     # tests only: pack Alice's fixture mod, as her own tools would
```

No direct dependency on `watane-log` or `watane-pkg`: `dist` re-exports
`PublicKey`, `Entry`, `EntryId`, `PackageId`, `ReleaseRef`, and for the
mode filter of 4.3 `Grant`, `InterfaceName` and `Provider` (dist 3).

## 3. Public API

```rust
pub use watane_dist as dist;   // the app reaches dist's types through node from step 8 (spec T 2)
pub use watane_net as net;

#[derive(Clone, Debug, Default)]
pub struct Options {
    pub dist: watane_dist::Options,
    pub net: watane_net::Options,       // `limits` and `relay_server`; the switches are net's own (5.6)
}

/// This home's place among the participant's machines, from net's device list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Machine {
    Alone,      // no device list: never paired
    Anchor,     // the list's anchor is this node: it holds the identity; full
    Full,       // a replica named full
    Light,      // a replica named light
    Removed,    // a replica that a newer list no longer names
}

/// One per home. Send + Sync; share it as Arc<Node>. Every method blocks.
pub struct Node { /* Arc<Dist>, Net, Arc<DistHoldings> */ }

/// `net`'s `Holdings` over a `Dist` (4.1). Public so that tests call the
/// holder directly; `Node` makes its own. `empty()` answers as an empty
/// holder until `set` gives it the `Dist`.
pub struct DistHoldings { /* OnceLock<Arc<Dist>> */ }
impl DistHoldings {
    pub fn empty() -> Self;
    pub fn set(&self, dist: Arc<Dist>) -> Result<(), NodeError>;   // a second call: HolderSet; the first Dist stays
}

impl Node {
    /// The app's open (5.6): a replica opens keyless, any other home with its identity.
    pub fn open(home: &Path, confirm: Arc<dyn watane_dist::Confirm>, options: Options) -> Result<Node, NodeError>;
    /// watane-node's open: always keyless (spec W33). Refuses a home with an identity.
    pub fn open_keyless(home: &Path, options: Options) -> Result<Node, NodeError>;
    pub fn dist(&self) -> &Arc<Dist>;   // views and every call that has no net side
    pub fn net(&self) -> &Net;          // switches, pairing codes, answers, devices, events
    pub fn machine(&self) -> Machine;
    pub fn close(self);

    // Own statements (5.1): dist's call, then `told`.
    pub fn set_name(&self, name: &str) -> Result<Entry, DistError>;
    pub fn publish(&self, package: Vec<u8>) -> Result<Published, DistError>;
    pub fn withdraw(&self, release: &ReleaseRef, reason: &str) -> Result<Entry, DistError>;
    pub fn list(&self, link: &Link, note: &str) -> Result<Entry, DistError>;
    pub fn include(&self, link: &Link, note: &str) -> Result<Entry, DistError>;
    pub fn unlist(&self, listing: &EntryId) -> Result<Entry, DistError>;
    pub fn confirm_own(&self, own: Option<&Link>) -> Result<(), DistError>;

    // Reads (5.2, 5.3): `need` first on a light machine, `told` for each raised head.
    pub fn follow(&self, link: &Link) -> Result<RefreshReport, DistError>;
    pub fn refresh(&self) -> Vec<RefreshReport>;
    pub fn refresh_channel(&self, channel: &PublicKey) -> Result<RefreshReport, DistError>;
    pub fn choose_index(&self, link: &Link) -> Result<Vec<RefreshReport>, DistError>;
    pub fn fetch(&self, release: &ReleaseRef) -> Result<Fetched, DistError>;
    pub fn fetch_for(&self, release: &ReleaseRef, consumer: Option<&PublicKey>) -> Result<Fetched, DistError>;
    pub fn open_link(&self, link: &Link) -> Result<Fetched, DistError>;
    pub fn read_link(&self, link: &Link) -> Result<Fetched, DistError>;
    /// After an install of `fetched` (NODE-F-11): on a replica, keeps a channel that has no link.
    pub fn installed(&self, fetched: &Fetched) -> Result<(), DistError>;

    // Search (5.4).
    pub fn search(&self, query: &watane_dist::Query) -> Result<Vec<NodeHit>, DistError>;

    // Pairing and restore (5.5, 5.7).
    pub fn join(&self, code: &PairCode, replace: bool) -> Result<DeviceList, NodeError>;
    pub fn own_hints(&self) -> Vec<OwnHint>;
    pub fn confirm_own_hint(&self, hint: &OwnHint) -> Result<(), NodeError>;
}

#[derive(Clone, Debug)]
pub struct NodeHit {
    pub hit: watane_dist::Hit,       // verified by this home's dist (5.4)
    pub from: Vec<NodeKey>,          // the machines that gave it; empty for a hit of this home only
}
#[derive(Clone, Debug)]
pub struct OwnHint { pub node: NodeKey, pub head: Option<HeadInfo> }   // a full machine's answer (net 5.11)

#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error(transparent)] Dist(#[from] DistError),
    #[error(transparent)] Net(#[from] NetError),
    #[error("this home's identity {fingerprint} has signed at least one statement; a home that joins your machines holds no identity")]
    IdentityInUse { fingerprint: String },
    #[error("{path} is a watane home with mods; watane-node runs a home of its own")]
    HostHome { path: PathBuf },
    #[error("this hint names no head")]
    EmptyHint,
    #[error("this holder already holds a home")]
    HolderSet,
}
```

`NodeKey`, `Net`, `NetError`, `PairCode`, `DeviceList`, `HeadInfo` and
`Role` are `net`'s (net 3); `Dist`, `DistError`, `Link`, `Entry`,
`EntryId`, `ReleaseRef`, `Published`, `RefreshReport`, `Fetched`, `Query`
and `Hit` are `dist`'s (dist 3).

## 4. The holder

### 4.1 `DistHoldings`

`DistHoldings` implements `net`'s `Holdings` (net 3.1) over an
`Arc<Dist>`. Each method is one call of a holder method of dist 5.12, which
reads only the stored channels and package files, makes no request, and
never takes dist's read lock (RV-06). So no question of `net` waits for a
read from a web host (NODE-F-7).

| `Holdings` | `DistHoldings` |
|---|---|
| `is_anchor()` | `dist.identity().is_some()` |
| `heads(channels)` | `dist.heads(channels)`, each `HeadState` as `HeadInfo`; an error gives an empty list |
| `entries(channel, from, limit, max_bytes)` | `dist.entries(..)`; an error gives an empty list |
| `fork(channel)` | `dist.fork(channel)`; an error gives `None` |
| `blob(id)` | `dist.package_file(&id)` (4.3) |
| `wanted_blobs(Full)` | `dist.missing_packages()` |
| `wanted_blobs(Light)` | the packages that `need_blob` asked for (5.3) and `dist.package_file` still lacks |
| `ingest(channel, entries, why)` | `dist.ingest(channel, entries, Reason::Kept)` for `Why::Kept` on a replica; `Reason::Asked` for `Why::Asked`, and for `Why::Kept` on the anchor (NO-8); `dist::Ingested` as `net::Ingested`, a `Compromised` without its seq, a `Failed` with the error's text |
| `store_blob(id, file)` | `dist.store_package(&id, file)`, its error as text |
| `search(query)` | `dist.search(&q)` with `q` from 4.3, each hit as `net::Hit` (4.3) |

`wanted_blobs(Light)` lists the packages that `need_blob` asked for and that `dist.package_file` still lacks (built in stage 2). A second `set` is `NodeError::HolderSet` and keeps the first `Dist`.

`DistHoldings` is made before the `Dist` exists (5.6) and holds it in a
`OnceLock`. Until it is set, every method answers as an empty holder:
`false`, no heads, no entries, `Failed("the home is opening")`. An empty
answer is a statement of one machine at one time (PEERS A3); it makes no
other machine drop anything.

### 4.2 What a home holds

The holder's "held" channels are dist's records and the own channel
(dist 5.12): followed, chosen, listed and kept channels. A channel that a
light machine pulled for a hit or a need (`Asked`) is stored with no
record (net Q2): it is not in `heads(None)`, so no machine syncs it, until
an install keeps it (5.3).

### 4.3 Conversions

- **Query.** net's `Query` (net 4.4) to dist's: `text` (null as `""`),
  `publisher`, `origin` (`Slug::from_str`; a bad slug gives no hits) and
  `interface` (`InterfaceQuery::from_str` of the name; a bad name gives no
  hits). dist's `Query` has no mode, so node keeps a hit only when the
  release has the name in that mode: `offers` in `release.offers`, `sends`
  in `release.events`, `uses` in a call or subscribe grant. `limit` cuts
  the result. Back, for `net.query` (5.4): dist's `text`, `publisher` and
  `origin` and `limit` 200. dist's `InterfaceQuery` matches any of the
  three modes and net's names one, so a query with an interface becomes
  three net queries, one per mode (`offers`, `uses`, `sends`) with that
  name, whose hits are merged by channel and entry (5.4 step 2); a query
  with none sends `interface` `null`. `search_stored` then applies dist's
  own filter again (5.4 step 4).
  Built so far: dist's `InterfaceQuery` has no accessor for its name, so
  `search` sends one net query with `interface` `null` and only
  `search_stored` applies the interface (12): the shown hits are the
  same, but the remote machine's 200 hits and net's 32 pulls are spent on
  hits of any interface.
- **Hit.** dist's `Hit` to net's: `[channel, seq, entry]` from
  `hit.info.channel`, `hit.info.seq` and `hit.info.entry`.
- **Package ids.** net's `BlobId` and pkg's `PackageId` are both the plain
  BLAKE3 hash. node converts with `PackageId::from_bytes` and `as_bytes`.

## 5. Behaviour

### 5.1 Own statements

`set_name`, `publish`, `withdraw`, `list`, `include`, `unlist`: call
dist's method of the same name; on `Ok`, call `net.told(own, entry.seq(),
&entry.id())` (NET-F-19, NODE-F-8). `publish` gives `published.entry`.
`confirm_own`: dist's call; then `told` for the own head when one is
stored. With `net` off, `told` does nothing (net 3).

### 5.2 Reads and `told`

Each read wrapper (`follow`, `refresh`, `refresh_channel`,
`choose_index`, `fetch`, `fetch_for`, `open_link`, `read_link`) takes
`dist.heads(None)` before and after dist's call, and calls `told` for each
channel whose head rose, in key order (NODE-F-8). node calls `told` after
dist returned, never inside a holder method.

`told` is called for the heads that this home learned from a web host or
signed. A head that `net` itself pulled is not told again: the hint that
caused the pull already spread through the topic (net 5.7), and the
10-minute sync catches a missed one.

### 5.3 A light machine

Only when `machine()` is `Light` (NODE-F-9). Every `need` and `need_blob`
error is ignored (`Off`, `NoFullMachine`, an unreachable machine) except a
refusal (`NetError::Refused`, the full machine no longer lists this one):
dist then reads from the link location as before (net 5.9), and when that
call fails with a missing source (`NotFollowed`, `NoRelease`,
`UnknownRelease`, `StaleBases`), the refusal is returned in its place, as the
cause of a `DistError::Io` (`node <short> refused: not_allowed`), not dist's
"is not followed" (NO-14). Any other dist error (`Compromised`, `Withdrawn`,
`BadKey`, `NoIdentity`, a web host's `Network`/`Http`/`BadLink`) is returned as it is.

| Wrapper | Before dist's call |
|---|---|
| `follow(link)`, `choose_index(link)` | `net.need(link.channel())` |
| `refresh_channel(c)` | `net.need(c)` |
| `fetch(r)`, `fetch_for(r, _)` | if the stored view of `r.publisher` lacks `r`: `net.need(channel)`. Then, if the release is stored and `dist.package_file(release.package)` is `None`: `net.need_blob(id, release.size)` |
| `open_link(link)`, `read_link(link)` | `net.need(link.channel())`; then the package of `dist.latest(channel, slug)` as for `fetch` |
| `refresh()` | nothing: `net` syncs a light machine's held channels every 10 minutes (net 5.6) |

After `need_blob`, dist's fetch finds the verified package in its cache
(dist 5.7 step 3) and makes no request. `need_blob` adds `(id, size)` to
the asked set of 4.1 first, and removes it once the package is stored, so
`net`'s next sync retries a failed one.

`installed(fetched)` (NODE-F-11): when this home is a replica,
`fetched.link` is `None` and the channel is not followed,
`dist.keep(channel)`. The channel is then held, so `net` keeps it current
from the full machines, and the app's refresh shows its updates (APP-F-39).
On any other home it does nothing: the app follows `fetched.link` as before
(app 9.4).

### 5.4 Search

`search(query)` (NODE-F-10):

1. `local = dist.search(query)`. Not light: return each as `NodeHit` with
   `from` empty.
2. `remote = net.query(&q)` with `q` from 4.3 (three calls when the
   query has an interface, merged). `net` already pulled each
   hit's channel that the holder lacked, from the machine that gave it, at
   most 32 channels per query (net 5.9).
3. For each remote hit whose channel is stored with a head below the hit's
   seq: `net.need(channel)`, counted against the same 32.
4. `stored = dist.search_stored(query, channels)` over the distinct
   channels of the remote hits (dist 5.12). A remote hit is **verified**
   when a stored hit has its channel and its entry id (`hit.info.entry`):
   dist decoded and stored that entry, and its release passes the same
   filters as a local hit. Others are dropped; the participant sees only
   what this home verified (F-FIND-4).
5. Merge: the local hits in dist's order, each with `from` the machines
   whose verified hit has the same `ReleaseRef`; then the other verified
   hits in dist's order with their machines. At most 200.

### 5.5 Join

`join(code, replace)` (NODE-F-12):

1. If `dist.identity()` is set: the own channel must have no statement
   (`dist.own()?.head` is `None`), else `IdentityInUse`. Then
   `dist.drop_identity()` (dist 5.12), which `log` refuses again if the
   identity signed (log 5.5).
2. `net.join(code, replace)`; return its list.

The identity is dropped first, so no crash leaves a replica with a key.
If step 2 fails, the home stays keyless until it is opened again; it is
then no replica, and `Dist::open` creates a fresh identity (dist 5.1). An
unused identity loses nothing.

### 5.6 Open

`Node::open(home, confirm, options)`:

1. `holdings = Arc::new(DistHoldings::empty())`.
2. `net = Net::open(home/net, holdings.clone(), options.net)`, with
   `options.net` as the caller gives it. The app gives `on` true, so `net`
   takes its own switches (`on`, relays, the DHT) from `book.json` (net
   4.5, 5.1, Q1); a new home starts with all of them off. `watane-node`
   gives `on` false to every command but `join` and `run` (6.1).
3. A **replica**: `net.devices()` is `Some(list)` and `list.anchor !=
   net.node_key()`.
4. Replica: `Dist::open_keyless(home, options.dist)`. Else
   `Dist::open(home, confirm, options.dist)`, which creates the identity at
   the first open (DIST-F-1).
5. `holdings.set(dist)`.

`Node::open_keyless(home, options)`: `<home>/home.json` exists:
`HostHome`. Then steps 1, 2, `Dist::open_keyless` (it refuses a home with
an identity key, log `HasIdentity`), 5.

`machine()`: from `net.devices()` and `net.node_key()`, each call: no list
is `Alone`; the anchor is this node: `Anchor`; this node in the list with
its role: `Full` or `Light`; else `Removed`.

### 5.7 Restore hints

`own_hints()`: with no identity, empty. Else `net.own_heads(&own)`, each
as an `OwnHint` (NET-F-30). The app shows them as hints; nothing is
confirmed (W18).

`confirm_own_hint(hint)` (NODE-F-13): `hint.head` is `None`:
`EmptyHint`. `net.need(own)` from a full machine (a refusal, here or of a package, is kept and
returned as `NodeError::Net` only if the next step fails). For each live own release whose
package `dist.package_file` lacks: `net.need_blob`. Then
`dist.confirm_own_head(seq, &id)` (dist 5.12): it refuses with
`HeadNotStored` unless the stored own head is exactly that seq and id and
not forked, so a hint that this home did not verify confirms nothing.

## 6. The program `watane-node`

### 6.1 Commands

```text
watane-node --home <dir> join [--replace] [--bind <addr:port>] <code>    NODE-F-15
watane-node --home <dir> run [--relay <addr:port>] [--bind <addr:port>]  NODE-F-14, NODE-F-16
watane-node --home <dir> status
watane-node --home <dir> relay add <url> [--yes]    NODE-F-17
watane-node --home <dir> relay remove <url>
watane-node --home <dir> dht on [--yes]             NODE-F-17
watane-node --home <dir> dht off
watane-node --help
watane-node --version
```

`--help` or `-h` anywhere, or `help` as the first word, prints this usage on
stdout and exits 0; no home is needed. `--version` alone prints
`watane-node <crate version>` on stdout and exits 0, also with no home.
Any other unknown flag exits 2 with `unknown flag <flag>` and the usage.

`--home` is required; `WATANE_NODE_HOME` stands in for it. The home is
created when missing. A home with `home.json` (a home of the app or of
`watane`) is refused (`HostHome`): a node home holds no mods and no
identity. Arguments are parsed by hand, as `watane` does.

- Every command but `join` and `run` opens with `options.net.on` false,
  so it makes no network contact whatever `book.json` says (Q1).
  `join` and `run` set `on: true` with `net.set_options` when they start,
  and `on: false` when they end, also on SIGINT and SIGTERM; after a crash
  `book.json` may still say on, which no other command acts on.
- **join**: `Node::open_keyless`; `net.set_options` with `on: true` for
  the duration of the command; print `this machine: <short key>` and
  `waiting for your answer on the anchor`; `node.join(code, replace)`;
  print `joined as <full|light>; machines: <short key> <role>, ...`. A
  `Declined` exits 1 with `watane-node: declined`.
  The signals are registered before net turns on; `node.join` runs on a
  scoped thread and the main thread waits for its result or for SIGINT or
  SIGTERM. On a signal it turns net off (persisted, outside the signal
  runtime) and exits 1 with `watane-node: interrupted`.
  `--bind <addr:port>` (also on `run`) is net's `Options.bind`, the address
  to bind; the default is net's (`0.0.0.0:0`), and a loopback address keeps
  mDNS off (10, NO-12).
- **run**: `Node::open_keyless`. `Alone` exits 1 with `not paired: run
  watane-node join <code> first`; `Removed` exits 1 with `this machine was
  removed from your machines: run watane-node join --replace <code>` (it
  still stores the old anchor, so a plain `join` is `HasAnchor`).
  `options.net.relay_server` from
  `--relay`. `net.set_options` with `on: true`; print `running as <full|
  light>, node <short key>` (and `relay on <addr>; add http://<addr>/ to
  the relays of your other machines` with `--relay`; `Net::relay_url()` gives that URL, `http`, not `https`). `--relay` takes a fixed `addr:port`: port 0 is refused (exit 2), because the other machines store the relay's URL. One thread prints
  each `net` event (6.2); the main thread waits for SIGINT or SIGTERM
  (tokio's `signal` on a runtime of the main thread that does nothing else; no node or net call runs inside it), then `node.close()` and exit 0. `net` is on only while
  `join` or `run` runs.
- **a home that another watane-node holds**: `Dist` keeps its lock while
  `run` runs. A command that finds `DistError::Locked` exits 1 with
  `another watane-node is running on this home (<home>); stop it first`
  and changes nothing; `status` instead opens `net` alone (`Net::open` on
  `<home>/net`, net off, an empty holder; it writes nothing), prints its
  lines for node, machine, anchor, devices, relays and dht, and in place
  of the channel and package lines `channels and packages: watane-node run
  is running on this home`.
- **status**: no network. Node key, machine, anchor, device list (seq,
  expiry, each machine), relays, DHT on or off, the number of held
  channels and of missing packages.
- **relay add**, **dht on**: print the consent text of section 7. With
  `--yes`, or a line `y` or `yes` on standard input, store it with `net.set_options`;
  else change nothing and exit 1 with `not changed`. When standard input
  is not a terminal, the program writes the answer it read (trimmed) and a
  line break after `Turn this on? [y/N] `, so the next output starts on its
  own line. After a change, the command prints the line `status` prints for
  it: `dht on` or `dht off`, `relays <list|none>`. **relay remove**,
  **dht off** need no question: they only take something away.

Exit codes: 0 done; 1 a refusal or failure, with one line
`watane-node: <cause>` on stderr (the whole cause chain, each cause once,
as app 13 does, and each node's short key once: `node K is unreachable:
refused: not_allowed`, not `node K ... node K refused`); 2 bad arguments, with the usage.

### 6.2 Output of `run`

One line per event on stdout (a repeated `node <short>` in a cause is dropped), in this form, so that a test and a person
read the same text (NODE-F-18):

| Event (net 3) | Line |
|---|---|
| `PeerUp(n)`, `PeerDown(n)` | `up <short>`, `down <short>` |
| `Pulled { channel, seq, from }` | `pulled <channel fingerprint> to <seq> from <short>` |
| `Refused { node, cause }` | `refused <short>: <cause>` |
| `Unreachable { node, cause }` | `unreachable <short>: <cause>` |
| `DevicesChanged(list)` | `machines <seq>: <short> <role>, ...` |
| `JoinAsked` | never: a node home is never the anchor |

## 7. Consent texts

`words.rs` holds the texts that the app's consent view and the program
both show, so that the participant reads one text for one choice (W32):

```rust
pub fn net_on() -> &'static str;          // turn on own machines
pub fn dht_on(node: &NodeKey) -> String;  // the DHT switch
pub fn relay_add(url: &str) -> String;    // a relay
pub fn join_asked(node: &NodeKey, role: Role) -> String;   // the anchor's question
```

- **net on**: `Connect your own machines. This machine will announce
  itself on your local network and connect to the machines that you pair.
  It publishes nothing on the internet, and uses no relay and no DHT
  unless you choose one.`
- **DHT on**: `Publish this machine's addresses in the Mainline DHT. The
  DHT is a public network that anyone can read. Anyone who knows this
  machine's key <short> can then see its addresses, this machine's local
  network addresses included, and when it is online.
  Your identity and your other machines are not published. A record
  already published can stay readable for some hours after you turn this
  off.`
- **relay**: `Use the relay <url>. When your machines cannot reach each
  other directly, their traffic goes through that server. It cannot read
  or change what they send, but it sees which of your machines talk, when,
  and how much. Choose only a relay that you run or trust.`
- **join asked**: `A machine asks to join as a <full|light> machine. Its
  key is <short>. Pair it only if this is the key that the machine shows.
  A full machine keeps a copy of everything that your machines hold; a
  light machine keeps what it uses. Neither can sign for you.`

## 8. Security properties

| Property | Enforced by | Test |
|---|---|---|
| A node home cannot sign | `Dist::open_keyless` over `Log::open_keyless`: no key file, every signing path `NoIdentity` (dist 5.12, log 5.5) | S1 |
| A replica of the app holds no key | the open of 5.6 chooses keyless from the device list; `join` drops the unused key first (5.5) | S8 |
| Nothing unverified is stored | `net` checks before the holder (NET-F-23); `dist.ingest` and `store_package` verify again (dist 5.12) | S3 |
| A remote hit is shown only when verified here | 5.4 step 4: its entry is stored by dist | S6 |
| No net question waits on a web host | the holder methods never take the read lock (dist 5.12) | S5 |
| No network with net off | `net` makes no socket; node has none (NET-F-3) | S2 |
| The DHT and relays only on the participant's choice | the consent texts of section 7; the program asks or needs `--yes` | S12 |

Not protected: another program of the user can read `<home>/net/node.key`
of a node home and act as that machine (spec T 6). It still cannot sign
for the participant: the home holds no participant key.

## 9. Build plan

| Stage | Build | Needs | Done when |
|---|---|---|---|
| 1 Holder | crate and workspace member; `DistHoldings`, `convert.rs`; `Node::open`, `open_keyless`, `dist`, `net`, `machine`, `close`; the wrappers with `net` off | dist S8, log L6 (keyless open, holder methods); net N1 (`Net`, `Holdings`, `Options`) and N2 (`devices`, for `machine` and the open of 5.6) | S1, S2, S3 |
| 2 Machines | `told` and `need` in the wrappers, `installed`, `search`, `join`, `own_hints`, `confirm_own_hint`; `words.rs` | net N4 (light machines, restore, reach) | S4 to S9 |
| 3 Program | `watane-node`: join, run, status, relay, dht; the output of 6.2 | stage 2; net N4 (relay server) | S10 to S13 |

Then app stage G (app 12) builds the views on stage 2, and J8
(`app/tests/it/j8.rs`) closes step 8.

## 10. Test plan

- No test reaches the internet. Nodes bind to loopback with mDNS off and
  addresses given directly, as `net` 7 says; a relay test starts the relay
  of `watane-node run --relay 127.0.0.1:<p>`, with a free port that the test picks.
- Channels are signed in the test by a keyed `Dist` of another home,
  with an approving `Confirm`; the expected heads, entries and package
  bytes come from that home, not from `watane_node`.
- S3 calls the `Holdings` methods directly, with no network.
- S5 uses a `tiny_http` host that answers after a delay longer than the test needs (S5 of stage 2 uses 4 s, with dist's `entry_timeout` at 3 s, to keep the test short), and times the
  server's `heads` answer.
- `program.rs` runs `env!("CARGO_BIN_EXE_watane-node")` and reads its
  stdout and exit code; the anchor is a `Node` in the test process, which
  answers `JoinAsked` with `answer_join`.
- Each scenario of `FUNCTIONAL.md` 3 is one test, named `s<n>_...`.

## 11. Provides to other subsystems

| To | What |
|---|---|
| `net` | the `Holdings` implementation over `dist` (section 4) |
| `app` | `Node`, `Options`, `Machine`, `NodeHit`, `OwnHint`, `NodeError`; the consent texts of section 7; `dist` and `net` re-exported |
| participants, J8 | the program `watane-node` (section 6) |

## 12. Needs from other subsystems

| From | What |
|---|---|
| `dist` | stage S8 (dist 8, 5.12): `Dist::open_keyless`; `identity()` and `fingerprint()` as `Option`; `drop_identity`; `confirm_own_head`; the holder methods `heads`, `entries`, `fork`, `package_file`, `missing_packages`, `ingest`, `store_package`, `keep`, `search_stored`; `HeadState`, `Reason`, `Ingested`, `Via::Kept`; `NoIdentity`, `HeadNotStored`. With `net` off, the rest of dist 3 as today. |
| `log` | nothing directly; stage L6 (`Log::open_keyless`, `drop_identity`, `channels`) through `dist`. |
| `net` | net 3 as written: `Net::open`, `set_options`, `close`, `node_key`, `devices`, `join`, `told`, `need`, `need_blob`, `query`, `own_heads`, `events`; `Holdings`, `Why`, `Ingested`, `Role`, `Query`, `Hit`, `HeadInfo`, `BlobId`, `NodeKey` (short form), `PairCode` (`FromStr`), `DeviceList`, `Event`, `NetError`, `Options`. |
| `host` | nothing; node homes are no host homes. The app opens `host` and `node` on one home. |

Gaps found in stage 2 (no code of a sibling was changed):

| Subsystem | Gap |
|---|---|
| `dist` | `InterfaceQuery` has no accessor for its name, so `search` cannot turn a query with an interface into a `net` query (4.3). It sends the remote query with no interface and lets `search_stored` apply the interface (5.4 step 4): correct, but the remote machine may return hits that are then dropped. |
| `net` | A hint is acted on with the machine that delivered it (net 5.7), and iroh-gossip's `delivered_from` is the neighbor that relayed the message, not the machine that told it. In a topic of three machines the hint often reaches a machine through another replica that has not pulled the head yet (seen in S7: the light machine got the server's relay of the laptop's hint, asked the server, which still had the old head, and pulled nothing). The head then waits for the next sync (on connect, or 10 minutes). In about 1 run in 5 of `tests/it/light.rs` the tests of S6 and S7 reconnect (`set_options` off and on, which syncs) after 3 to 5 s without the entry. Fixed in S72 (net 5.7): a machine is asked again every 250 ms, up to 5 s, until its head reaches the hint's seq; each retry asks the next trusted machine with a live link, so a light deliverer that never holds the channel does not stop the pull. The reconnect workaround of `tests/it/light.rs` is removed. |

## 13. Questions

| # | Question | Answer used here |
|---|---|---|
| Q1 | net 3 gives `Net::open` an `Options` with `on`, `relays` and `dht`, and net 5.1 reads the same switches from `book.json`. Which wins at open? | `Options.on` false keeps the endpoint off for that open, whatever `book.json` says. With `Options.on` true, `book.json` decides `on`, the relays and the DHT; a new home's `book.json` has all of them off. `set_options` writes `book.json`. For net N1 to confirm and write into net 3 and 5.1. |
| Q2 | Does `Net::events()` give one receiver or one per call? | node takes no events itself; the app and the program each call it once, and each needs every later event. |
| Q3 | net 5.7 runs a pull for any told channel whose seq is above the holder's. On a light machine that pulls every channel any machine tells, as `Asked`, against NET-F-21 and net Q2. | node assumes the net N3 rule: a light machine acts on a hint only for a channel in `holdings.heads(None)`; a full machine for any channel. For net N3 to write into net 5.7 (outside S59's part of net). |
| Q4 | node 3 derives `Default`, `Clone` and `Debug` on an `Options` that holds `watane_net::Options`. | net's `Options` and `Limits` derive `Clone`, `Debug` and `Default`, with `on` false, `relays` empty, `dht` false, `relay_server` `None` and the limits of net 4.2. For net N1 to write into net 3. |
| Q5 | The anchor is `full` in the device list (net 5.5), but keeps only what the participant's acts there chose (NO-8). | NET-F-21 ("a full machine keeps every channel any own machine holds") holds for full replicas only, and a light machine's `query` and `need` (net 5.9) should prefer a full replica over the anchor, whose search does not cover what only the replicas keep. For net N2 and N4 to write into NET-F-21 and 5.9 (outside S59's part of net). Answered in net 5.9 (S67). |
| Q6 | A query with an interface is three `net.query` calls (4.3), and net 5.9 pulls at most 32 channels per call. | One participant query may then pull up to 96 channels, against the 32 of PEERS 8.2. For net N4: a cap per caller over 5 s, or a `query` that takes several modes; until then node stops pulling in step 3 once the three calls pulled 32. Answered in net 5.9 (S67): the cap of 32 holds per `Net` over 5 s. |

## 14. Decisions of this subsystem

| # | Decision | Reason |
|---|---|---|
| NO-1 | The app's open decides keyless from `net`'s device list: `Net` opens first, with an empty holder, then `Dist`. | No new file and no new mark: `net` already stores the anchor; `dist` and `log` stay unaware of `net`. Undo: cheap (a mark of dist's own). |
| NO-2 | A home joins only while its identity signed nothing, and drops that key before it joins. | A replica holds no key (W33), and a key that signed nothing loses nothing; a key that signed is the participant's and is never deleted. |
| NO-3 | `told` only for own statements and heads raised by a web read; heads that `net` pulled are not told again. | Gossip already spread the hint (net 5.7); a second hint per machine only adds traffic. Undo: cheap. |
| NO-4 | A light machine asks `net` before dist reads and ignores its failure, except a refusal (NO-14). | dist then reads from the link location as before, so `net` is never a requirement (NET-F-31). |
| NO-5 | A remote hit counts only when its entry is stored here; the shown hit is dist's view of it. | One verifier, the same filters as a local hit (F-FIND-4, PEERS 8.2). |
| NO-6 | An install from a remote hit with no link gives the channel a `kept` record on a replica. | Updates keep coming through the full machines, as a follow does through a web host (DD-24); it is net Q2's answer. |
| NO-7 | The consent texts live in `node`, used by the app and the program. | One text per choice, wherever it is made (W32). |
| NO-8 | Only a replica keeps: on the anchor, a `Kept` pull is ingested as `Asked`, with no record. | The anchor's held set comes only from the participant's own acts there (follow, index, install), so an unfollow on the anchor takes effect and the anchor does not collect what the light machines follow. Undo: cheap. |
| NO-9 | The program reads `relays` and `dht` with `Net::options()`, to keep them when it changes the other switch and to print them in `status`. | `net` keeps them from `book.json` at open (net 5.1) and gives them back. Undo: cheap. |
| NO-10 | A question (`relay add`, `dht on`) reads one line from standard input; only `y` or `yes` counts, so a closed or empty input is a no. | A script and a person get the same rule, and nothing changes without a yes. |
| NO-12 | `join` and `run` take an optional `--bind <addr:port>`, passed as `Options.bind`. | A test of the program, and a participant with several networks, choose the interface; the default is unchanged. |
| NO-13 | `status` beside a running `run` reads `net`'s files alone and says that the channel and package lines are `run`'s. | `run` holds dist's lock for as long as it runs; net's files are read without the lock and the command writes nothing. Undo: cheap. |
| NO-14 | A refusal of `need`/`need_blob` is kept; if dist's call then fails with `NotFollowed`, `NoRelease`, `UnknownRelease` or `StaleBases` (no source), the wrapper returns `DistError::Io { path: "the server", source: <the refusal> }`. | A removed light machine learns that the server refused it (the participant must re-pair), where dist alone says the channel is not followed. A wrapper keeps its `DistError` type, so the app and dist callers are unchanged; the chain carries the net text. Undo: cheap. |
