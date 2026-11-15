# net: architecture proposal for the data plane

Written on 2026-11-14 by the reviewer of `REVIEW.md`, as an answer to it.
It is a proposal outside the chain: nothing here binds a builder until the
human takes it into `spec/`. Findings are cited as RV-n.

**Status (2026-11-14).** Taken into `spec/PEERS.md` and the `net`
documents, with the decisions of 16.1. Those documents are the contract;
this file stays as the record of why.

Precedence used, strictly in this order:

1. `spec/VISION.md`, and `net/VISION.md` where the two agree.
2. `spec/FUNCTIONAL.md`, then `net/FUNCTIONAL.md`.
3. `spec/TECHNICAL.md`, `spec/PEERS.md`, `net/TECHNICAL.md`, and the
   formats of `log` and `dist`.

Section 13 lists every place where this proposal leaves a lower document,
and why.

## 1. The design in one page

- **The waist stays.** Every machine moves `log` entries and BLAKE3
  packages, byte for byte. `dist` stays the one verifier and the one
  store. No class is trusted for authenticity (PEERS PG-3, PM-2).
- **Every source only raises a head.** `log` takes an entry only when it
  extends the chain. So a static host, a friend and a stranger can all
  feed the same channel with no coordination, and "the highest valid head
  wins" needs no protocol. Classes decide only what is *asked*, what is
  *served*, and which answers count as *checked*.
- **Three facts, three mechanisms.** A place to read is a public `peer`
  statement. An own machine is a line in a private device list. A friend's
  node is a ticket exchanged by hand. No class follows from a public
  statement (RV-01, RV-02).
- **Private first.** `net` is off until the participant turns it on. A
  personal node serves strangers only the own channel, and only when asked
  to.
- **Own machines are full or light.** No shards (RV-07).
- **A public peer is files.** A job writes site folders and a signed heads
  file. Any web server or CDN serves them. Search and relay are separate
  optional services (RV-10).
- **Freshness without cosignatures.** A node the reader rates answers
  `heads` over an authenticated connection. A public peer re-signs a heads
  file on a timer (RV-04).
- **`dist` never calls `net`.** A small crate `node` joins them and owns
  the program `watane-node` (RV-06, RV-11).

## 2. Principles

| # | Principle | From |
|---|---|---|
| A1 | Verify everything that can be verified, at the machine that uses it. Trust a class only for the rest. | Saltzer, Reed, Clark 1984; PEERS PG-4 |
| A2 | An answer about absence or completeness is a statement of one node at one time. It is shown with its source, never as a fact. | PEERS "rule of claims" |
| A3 | The participant names nodes; no global name or rating exists. | V6; petnames (Stiegler 2005) |
| A4 | Nothing is published about a participant that the participant did not choose to publish. | V5 |
| A5 | A request costs its sender more to forge than it costs the receiver to refuse. Limits are in bytes and connections, checked before work. | V5 |
| A6 | No built-in endpoint: no relay, bootstrap host, index or public node ships turned on. | V4, W23 |
| A7 | The simplest mechanism that works for ten machines and a thousand channels. Each scaling step is named, and none is built early. | VISION order of work; README rule 3 |

## 3. Components and boundaries

| Subsystem | Owns | Never |
|---|---|---|
| `log` | The entry, the chain, the fork rule, the identity. Unchanged formats. | Network, meaning, a signature outside an entry. |
| `dist` | Verification, storage, views, search, statement kinds (now also `peer`, `unpeer`), static-host reads, the site folder. | A thread, a socket, a call into `net`. |
| `net` | Node key, tickets, the peer book and classes, the wire protocol, selection, sync loops, limits, the device list, the heads file. | A stored entry, a kind's meaning, the participant key, a class from a measurement. |
| `node` (new) | The glue: `Holdings` for `Dist`; the loops that tie `net` events to `dist` calls; the program `watane-node` (replica, friend's box, mirror job). | A window, a confirmation, a participant signature. |
| `app` | Every view and every confirmation: turn `net` on, pair, add a friend, grant, add a public peer, choose a relay. | A `net` action without a confirmation; a `net` route on the local API. |
| `host` | Reserves `<home>/net/`. | Reading it. |

```text
programs     watane-dev    watane    desktop app    watane-node
subsystems   dev ──▶ host ◀── app ──▶ node ──▶ dist
                                        └────▶ net
base         pkg (dev, host, dist)       log (host, dist, net: types only)
```

Dependencies: `net` on `log` (types and `Entry::decode`); `node` on `dist`
and `net`; `app` on `host` and `node`. `dist` and `net` do not know each
other. No cycle.

**The seam.** `net` defines one trait, `Holdings`, and calls it only from
blocking threads. No method calls back into `net`.

| Method | Purpose |
|---|---|
| `heads(scope)` | (channel, seq, id) of the channels this caller class may see |
| `entries(channel, from, limit, max_bytes)` | stored entries, original bytes |
| `fork(channel)` | the two entries of a fork, if any |
| `blob(id)` | a verified package file, only while a live release in a held channel names it (RV-40) |
| `serves(channel, class)` | may this channel be served to this class (5.2) |
| `ingest(channel, entries, from)` | verify and store; answers stored, head moved, not wanted, compromised |
| `store_blob(id, file)` | verify a package against a held release and keep it |
| `checked(channel, seq, id, class, node, time)` | a rated node confirmed this head (section 8) |
| `search(query)` | `dist` 5.10, hits as (channel, seq, entry id) |
| `sources(channel)` | bases and `peer` nodes that `dist` knows |

`dist` gains the methods behind these, a record reason `kept`, the two
kinds, and a way to open a home with no identity (RV-08). It gains no
thread and no dependency.

## 4. Objects and formats

### 4.1 Unchanged

An entry (`log` 4.3, 4.4). A package named by its BLAKE3 hash (`pkg`). The
site folder (`dist` 4.3). The link (`dist` 4.2), except 4.5 below.

### 4.2 Statements in the participant's channel

Owned by `dist`, signed through `sign_and_append` after a confirmation.

| Kind | Body | Says |
|---|---|---|
| `peer` | CBOR array of 2: `[node: bytes(32), note: text]` | Strangers may read this channel from this node. Nothing else. |
| `unpeer` | CBOR array of 1: `[peer: bytes(32)]` | Ends that `peer` statement. |

Rules, for `dist` 4.1: the first 4 live `peer` statements in seq order
count; a second live `peer` of the same node is a `BadStatement`; a
compromised channel gives no peers. A `peer` gives its node no class. A
reader that does not know the kinds ignores them (dist 4.1 rule 1).

### 4.3 Objects signed by a node key

Ed25519 as `log` 4.5, over an ASCII prefix, a zero byte, and the CBOR of
the items before the signature. Deterministic CBOR, arrays only.

| Object | Prefix | Items before `sig` | Travels |
|---|---|---|---|
| device list | `watane/devices/1` | `anchor: bytes(32), seq: uint, time: uint, devices: [[node: bytes(32), role: uint]]` | Only over connections to own machines and friends. Never published. |
| heads file | `watane/heads/1` | `node: bytes(32), time: uint, bits: uint, shard: uint, heads: [[channel: bytes(32), seq: uint, id: bytes(32)]]` | Public, over HTTP. Written only by a public peer (section 11). |

`role` is 0 (light) or 1 (full). `devices` is sorted by node key and holds
the anchor. A heads file covers the channels whose key starts with the
`bits` bits of `shard`.

There is no cosignature, no signed grant, no signed receipt and no signed
query answer. A grant is a record in the issuer's peer book
(`net/TECHNICAL.md` ND-8, kept).

### 4.4 Text forms

```abnf
ticket  = "watane-node:" key *( ";" hint ) [ "@" https-base ]
friend  = "watane-friend:" key ":" participant-key *( ";" hint )
pair    = "watane-pair:" key ":" secret *( ";" hint )
hint    = "relay=" https-url / "addr=" socket-address
key     = 52zbase32            ; a node key, as log 4.1
secret  = 26zbase32            ; 16 random bytes
```

A hint is where to dial the key now: a relay URL, a socket address. Hints
are data for the first connection only; later addresses come from 9.1.
This is what iroh's own tickets carry (RV-03).

### 4.5 A link that names a node (to decide, Q7)

`watane:<key>@watane-node:<node key>/[#slug]`: the base position holds a
node instead of a URL. `url::Url` parses it; today's `dist` refuses the
scheme, so an old reader refuses that one link or listing and nothing
else. Without this form, a channel still needs a static host to be found
(RV-12).

### 4.6 Files in `<home>/net/`

```text
node.key       the 32-byte seed; rules of log 4.7
book.json      version, on/off switches, friends, public peers, relays, grants, last good addresses
devices        the newest verified device list
anchor         on a replica: the node key of the anchor it paired with
```

## 5. Classes

### 5.1 How a node enters a class

| Class | Enters by | Bound to the participant by | Leaves by |
|---|---|---|---|
| unknown | default | nothing | n/a |
| public | The participant adds a ticket with an HTTPS base, in the app. | the participant's choice | the participant removes it |
| friendly | The participant adds a `watane-friend:` ticket in the app and sees both short keys. The ticket names the friend's **anchor**. Each node in the device list that this anchor signs is friendly too. | the ticket, handed over out of band; optionally compared in person | the participant removes the friend; the friend's newer device list drops a node |
| trusted | Pairing (below). The node is in the device list that this home's anchor signed. | the pairing code | a newer device list |

The **anchor** is the node in the home that holds the participant key. It
signs the device list with its node key (ND-4, kept). A friend pins the
anchor's node key from the ticket and asks the anchor, or any node the
anchor listed, for the newest list. So Bob's node knows Alice's phone,
Alice's home server serves Bob while her laptop sleeps, and nothing is
published.

The class is directional. Friends exchange tickets both ways.

**Pairing.** As `net/TECHNICAL.md` 5.9, with three changes: the code
carries hints (4.4); the anchor shows the short key of the joining node
and the participant confirms before the list is signed (RV-23); a replica
home is opened with no identity (RV-08). A home whose own channel already
has a statement cannot join as a replica; the app says why. A lost anchor
is replaced by a restore and a new pairing of each machine (cheap: a
handful of machines).

### 5.2 What a node serves, by the caller's class

Every check runs before any work: decode, size, byte budget, policy.

| Operation | trusted | friendly | public node | unknown |
|---|---|---|---|---|
| `hello` | all fields | all fields | version, operations, limits | version, operations, limits |
| `heads`, `entries`, `fork`, `blob` | everything held | channels the friend's grant names; the own channel | as unknown | the own channel, and only when "serve strangers" is on |
| `subscribe` | everything held | as reads | as unknown | as reads, 1 stream |
| `offer`, `put` | yes | by grant, within its byte quota | no | no |
| `query` | yes | by grant | no | no |
| `devices` | yes | yes (so friends know the nodes) | no | no |
| `pair` | n/a | n/a | n/a | with a valid code |

A class gives this floor; a grant adds to it (RV-21). A node in public
mode is a different program path (section 11), not a wider row here.

Limits, per caller key and per class, per minute: bytes sent, bytes
received, connections, long-lived streams. Unknown callers share one pool
that is smaller than the friendly pool, so free keys starve only each
other (ND-10, kept, in bytes: RV-17). Fairness inside a class is a deficit
round robin queue per caller key (Shreedhar and Varghese 1995).

## 6. Operations

### 6.1 Between nodes

QUIC through iroh, dialed by node key, ALPN `watane/1`. One bidirectional
stream per request. CBOR as `net/TECHNICAL.md` 4.2. The caller's key comes
from the handshake.

| Op | Args | Answer | Notes |
|---|---|---|---|
| `hello` | `version: uint` | `[version, ops: [text], limits, devices_seq or null, hints: [text] or null]` | no participant key (RV-01); `devices_seq` and the node's own hints only for trusted and friendly callers |
| `heads` | `[channel]`; empty means all (trusted only), paged by 1 024 | `[[channel, seq, id, forked: bool] or [channel, null]]` | replaces `head`, `have` and `locate` |
| `entries` | `channel, from, limit` | entries, at most 4 MiB per answer | |
| `fork` | `channel` | `[a: bytes, b: bytes]` or null | any node can pass a fork on (RV-37) |
| `blob` | `id, max` | `[size]`, then bytes | whole blob, BLAKE3 at the end (ND-7, kept) |
| `subscribe` | `[channel]` | a stream of `[channel, seq, id]` | hints; only on streams the receiver opened (RV-05) |
| `offer` | `channel, [entry]` | `[stored]` | |
| `put` | `id, size`, then bytes | `[stored: bool]` | a package that a held release names |
| `query` | `dist` 5.10 query: text, publisher, interface, origin, limit | `[hits: [[channel, seq, entry id]], more: bool]` | no kinds, no backlinks (RV-15) |
| `devices` | `seq or null` | `[list: bytes or null, hints: [[node, [text]]]]` | the hints are unsigned: last good addresses of the listed nodes (9.1) |
| `pair` | `secret` | `[list: bytes]` | |

There is no `announce`, `locate`, `keep` or `witness`. A grant to keep a
channel *is* the keep: the keeper subscribes and accepts offers. A change
to an operation is a new operation name; an unknown name answers
`unknown_op`.

Forwarding: a node forwards only for a trusted caller, only to own
machines, one hop, with a request id that a node drops when seen twice,
and budgets that the callee clamps (RV-16).

### 6.2 Over HTTP

| What | Form | Who serves |
|---|---|---|
| a channel and its packages | `dist` 4.3 under a base | any static host; a public peer under `<base>c/<key>/` (ND-12, kept) |
| heads | `GET <base>heads/<bits>/<shard>` (4.3) | a public peer |
| what the peer offers | `GET <base>node`: its ticket, `bits`, the period of the heads files, its roles | a public peer |
| search (optional role) | `POST <base>query`, the CBOR query; the CBOR answer | a public peer that runs the search service |

## 7. Choosing nodes

### 7.1 The caller

Candidates come from four places: the device list, the peer book
(friends, public peers), `Holdings::sources` (bases and `peer` nodes), and
what recent `heads` answers showed a node to hold.

| Need | Asked, in order | Stops when | Counts as |
|---|---|---|---|
| Read entries or a package (verifiable) | own machines; friends that hold it; the publisher's base and `peer` nodes; listing bases; chosen public peers | the bytes verify | a raised head, whatever the source |
| Freshness of channel K | in parallel, at most 3: the publisher's sources; one friend that holds K; one chosen public peer's heads shard | all answered or 5 s | "checked" only by the rule of section 8 |
| Query | this home; a full own machine when this one is light. Friends and public search only when the participant ticks them for this search | each asked node answered or 5 s | hits are hints; each is verified (below) |
| Store | every full own machine; friends whose grant names it | each accepted | a copy, not a proof |
| Updates | `subscribe` to own machines, to friends for granted channels, to the `peer` nodes of followed channels; heads shards of chosen public peers on a timer; static bases as today | n/a | hints, then reads |

Order inside one row and one class: by measured success and latency. The
estimate is a mean and a mean deviation (RFC 6298). `net` hedges node
reads itself: it starts the next node at mean + 4 × deviation, at most 2
in flight (Dean and Barroso 2013). `dist` keeps its sequential static
reads.

Privacy decides the order across classes for verifiable reads: an own
machine learns nothing new, a friend learns one interest, a publisher's
source learns a reader's address, a public peer learns it for many
channels.

An unknown node is never asked a query and never asked to store. Its
`heads` answers and announcements only cause reads.

**A remote hit** is verified by reading its channel from the node that
gave it, with no record (as `dist` 5.11), at most 32 channels per query,
each within the limits of a listed read. The rest are dropped (RV-09).

### 7.2 A node that asks others

| Node | May ask, for a request it serves | Rule |
|---|---|---|
| a light own machine | a full own machine | one hop |
| a full own machine | nothing for the request; it syncs on its own timer | answers from what it holds, with its heads so the caller sees how current it is |
| a friend's node | nothing | no friend-of-a-friend forwarding in the core (Q10) |
| a public peer | nothing | it pulls on its own schedule (section 11) |

PEERS R1 (original bytes), R2 (responsibility stays with the node asked),
R4, R5 and R6 hold unchanged. R5 also covers QUIC: for a node that is not
trusted, not friendly and not found on the local network, `net` dials only
global addresses and the relay (RV-36).

## 8. Verification and freshness, by class

Authenticity never depends on the class. A head is **checked** at a time
when one of these holds:

| Source | Class | Evidence |
|---|---|---|
| the publisher's own base gave `Current`, or a live `peer` node of the channel answered `heads` with the same head | unknown | today's rule (`dist` 5.4 step 7), extended to the nodes the publisher named; only the publisher's own places can be ahead of everyone |
| an own machine answered `heads` with the same head | trusted | authenticated connection |
| a friend's node answered `heads` with the same head | friendly | authenticated connection |
| a chosen public peer's heads file names the same head, and its `time` is within two periods | public | signature of its node key |

Any other source can only raise the head. The app shows "checked against:
publisher's host, Bob, pub.example.org, at 14:02" (F-FIND-4 marks the
rest). A fresh read never fails for lack of sources (RV-22).

This is the freeze-attack defence of TUF's timestamp role (Samuel,
Mathewson, Cappos, Dingledine, CCS 2010): someone other than the
withholding host states the newest version, and that statement is
short-lived. The publisher cannot re-sign on a timer (a laptop, a
confirmation per signature), so rated nodes do it. Certificate
Transparency's signed tree heads and monitors (RFC 9162) are the same
shape. Cosigning (CoSi, Sigsum, C2SP tlog-witness) adds proof against a
split view between witnesses; `log`'s fork rule already gives that for a
one-writer chain, so cosigning waits.

A fork: any node that holds two conflicting entries answers `forked` in
`heads` and gives both in `fork`. The receiver hands them to `ingest`,
and `log` marks the channel. A fork proof needs no class.

## 9. Processes

| Process | Between | Over | Trigger | Established as |
|---|---|---|---|---|
| own-machine sync | every pair of own machines | QUIC; local network first | connect, an announcement, every 10 minutes | anti-entropy (Demers et al., PODC 1987); SSB feed replication |
| friend sync | this node and each friend's node | QUIC | the same, for granted channels | the same |
| follow by node | this node and a followed channel's `peer` nodes | QUIC | `subscribe`; on an announcement, read | SSB `createHistoryStream` |
| follow by host | `dist` and static bases | HTTP GET | the app's timer (APP-F-36) | today |
| public poll | this node and chosen public peers | HTTP GET with `If-None-Match` | a timer of the peer's period, at least 60 s | conditional GET |
| address upkeep | this node and its own and friendly nodes | the hints in `hello` and `devices` (6.1) | connect; an address change | peer exchange |
| local discovery | nodes on one network | mDNS (RFC 6762) | always, when `net` is on | iroh-mdns-address-lookup |
| DHT record (optional) | this node and the Mainline DHT | UDP, BEP 44 | only when "find me by key" is on | pkarr |
| mirror job (public peer) | a public peer and the sources it admits | HTTP GET | its timer | a crawler with a frontier |

Sync is: `heads` both ways, `entries` for what is behind, `offer` where
allowed, `blob` or `put` for packages the receiver must keep. A vector of
a thousand heads is about 75 KiB; range-based set reconciliation (Meyer, SRDS
2023) replaces the vector when it stops fitting. Gossip trees (HyParView,
Plumtree, through iroh-gossip) replace direct subscriptions when one
channel has more followers than its `peer` nodes can hold streams for.

### 9.1 Reaching a node

In this order: an address on the local network (mDNS); the last good
addresses in the peer book; addresses that an own or friendly node passed
on; the hints of the ticket; a relay; the DHT when it is on.

A relay forwards encrypted QUIC and sees who talks to whom. iroh also uses
it to set up hole punching, so two machines behind NAT need one (Ford,
Srisuresh, Kegel 2005). The relay is chosen by class like any other node:
an own machine with a public address, a friend's, or a public relay the
participant adds. `watane-node` can run the relay role (iroh-relay).

## 10. The core, with no public peer

| Need | unknown | friendly | trusted |
|---|---|---|---|
| **Storage** | never asked | keeps the channels and packages a grant names, within a byte quota | full machines keep the whole keep set; light machines keep what they use |
| **Queries** | never asked | answers `query` by grant, when the participant ticked "ask friends" | a full machine answers a light one; the answer carries its heads |
| **Updates** | a publisher's `peer` node announces to subscribers; a static host is polled | pushes and pulls granted channels | pushes and pulls everything |
| **Subscriptions** | 1 stream, own channel of the serving node only | granted channels | everything |
| **Verification** | every byte; its heads only raise | every byte; its `heads` answer counts as checked | every byte of what is installed; its `heads` answer counts as checked |

What the core needs to connect, said plainly:

| Situation | Works with |
|---|---|
| own machines on one network | mDNS only |
| one machine with a public address (a home server with a forwarded port, a rented VM) | that machine as relay for the participant and, by grant, for friends |
| every machine behind NAT, no such machine among self and friends | nothing in the core. The participant adds a public relay or turns the DHT on. Both are outside the three core classes, and the app says so. |

The third row is the honest limit of PG-1 (RV-03).

What stays weak without public peers: finding mods outside the indexes one
reads (unchanged from today: an index is a channel, W20), and freshness of
a channel that no friend follows.

## 11. The public peer

A public peer is a set of **roles**. Each is a separate program path, and
only the first is needed to be one.

| Role | What runs | State | Fails how |
|---|---|---|---|
| **mirror** | `watane-node mirror --out <dir>`, from a timer. It reads the channels it admits (HTTP, as `dist` does), verifies them, and writes `<dir>/c/<key>/` in the `dist` 4.3 layout, `<dir>/heads/…` and `<dir>/node`, each by temp file and rename. Any web server, object store or CDN serves `<dir>`. | a home (`log`, `dist`) and a folder | The job stops: the files stay valid and grow stale; the heads `time` shows it. The web server is whatever the operator already runs. |
| search (optional) | a read-only HTTP service over the same home: `POST query` | none of its own | Down: clients lose remote search, nothing else. |
| relay (optional) | iroh-relay, as shipped | none | Down: nodes that chose it reconnect elsewhere. |

Why this is simple and reliable:

- The serving path of the mirror is a static web server. watane code
  handles no public socket, no TLS and no client state.
- A reader uses `<base>c/<key>/` as an ordinary base. Today's `dist` reads
  it unchanged. This is F-PUB-3 applied to someone else's channels.
- Every file is verified or signed, so a CDN and a second mirror need no
  trust (V4).
- The job is restartable at any point: every write is atomic and every
  read starts from the stored head.

**Admission.** The operator names seed indexes. The mirror admits their
read sets to a depth and a count that the operator sets (the rule of
`dist` 5.9 with larger caps), plus an allow list and a deny list. It takes
no offer from a stranger, so free keys cannot fill it. Removing a channel
from the output folder is a file delete; freeing it in the store waits
for `Log::forget` (dist Q12, RV-18).

**A public peer in the participant's home.** The participant adds its
ticket. From then on: its `c/<key>/` is one more base for reads; its heads
shards are polled and count as "checked by" it (section 8); its search is
asked only when ticked. Removing it loses speed and one checker.

The mirror needs no QUIC endpoint. A public peer may also run one, with
the unknown row of 5.2; nothing depends on it.

## 12. Threat model

| Threat | Answer | Left open |
|---|---|---|
| Forged entry or package | `Entry::decode`, the chain, BLAKE3, at the machine that uses it | none |
| A host or node withholds new entries | several sources raise the head; rated nodes state their head (section 8); the mark names who checked | a channel that only its publisher's host and strangers hold |
| Fork | `log`'s rule; `fork` passes the proof on | none |
| Sybil keys | no rule counts nodes or keys; unknown callers share one small byte pool; a mirror admits by seed, not by offer | fair service among honest strangers during a flood |
| Eclipse | a checked head needs the publisher's host or a rated node; own and friendly nodes are reached from pinned keys and cached addresses, not only from the DHT | a participant with no rated node that holds the channel |
| Reflection and amplification | announcements only on streams the node opened; a failed read after a hint does not fall back; hits verified from the answering node, 32 per query; forwarding one hop, for own machines only; budgets clamped | none known |
| Probes of the local network | listed bases as `dist` 5.9; global addresses only for unrated nodes (7.2) | a host name that resolves to a local address (as `dist` 6) |
| Exhaustion | byte, connection and stream budgets per key and per class, before work | a trusted machine has no limit |
| Who follows what | reads go to own machines first; `heads` for many channels only to own and friendly nodes; a personal node tells strangers nothing about other channels | a friend learns the channels asked of them; a public peer learns its readers' addresses per shard |
| Link from a participant to an address | no `peer` statement unless chosen; no participant key in `hello`; the DHT off unless chosen | a participant who chooses "serve strangers" publishes a node key, and its addresses are then findable |
| A stolen pairing code | 128 bits, 10 minutes, one use, and the anchor confirms the joiner's short key | none |
| A compromised own machine | it holds no participant key; installed packages are verified where they run; a newer device list removes it | it can withhold and lie about completeness until removed; a machine that is offline keeps trusting it until it sees the new list (Q14) |
| A compromised friend's node | it gets only what the grant names; its `heads` can mislead the "checked" mark for channels it holds | as stated: that is what friendly means |
| A relay | sees encrypted QUIC | who talks to whom, and when |
| A local program | no `net` route on the local API; every class change is confirmed in the app | another program of the same user can read `node.key` (as spec T 6) |

## 13. Deviations from lower documents

| Document | Item | Change | Why |
|---|---|---|---|
| PEERS | PM-4; 4.3 last paragraph | Own and friends' nodes are bound by a device list and tickets, not by `peer` statements. `peer` means "a place to read" only. | RV-01, RV-02; V5 |
| PEERS | 5.1; PM-4 | The Mainline DHT is optional and off by default. Tickets carry hints. | RV-03, RV-35; V4, W23 |
| PEERS | PM-7; 4.3 cosignature; 6.1 `witness` | No cosignature. Authenticated `heads` answers and signed heads files. | RV-04 |
| PEERS | PM-9; 7.2, 7.3 | Full and light roles; no rendezvous hashing, no scatter and gather. | RV-07; A7 |
| PEERS | PM-10 | Forwarding: one hop, own machines only. Hop limit 2 dropped. | RV-16; privacy of queries |
| PEERS | PM-11; 10.1 | A public peer is a mirror job and static files; search and relay are optional roles. Not "the same program with a public policy". | RV-10; PG-2 itself |
| PEERS | PM-6; 5.2, 5.3; 4.2 | Head vectors and direct subscriptions first; RBSR, gossip trees and Bao ranges are named scaling steps. | Same as net ND-6, ND-7 |
| PEERS | 4.3, 4.4 | No signed grant, receipt or query answer. | Same as net ND-8, Q2 |
| PEERS | 5.6, 5.7, 6.2, 11 | No `wish` or `voice` kinds, no backlink index, no `refers_to`, no counts. | RV-15; V7 |
| PEERS | 6.1 | `locate`, `have`, `keep`, `witness` removed or merged; `fork` and `put` added. | Fewer operations; RV-37 |
| PEERS | 8.1 step 4 | No "two origins" rule; a "checked" rule by source (section 8). | RV-22 |
| net F | NET-F-5 | `hello` gives no participant key. | RV-01 |
| net F | NET-F-7 | Friendly comes from a ticket and the friend's device list. | RV-02 |
| net F | NET-F-13 | Unknown callers get the own channel only, and only when turned on; no `query`, no `locate`. | RV-01; V5 |
| net F | NET-F-18 | Replaced by section 8. | RV-22 |
| net F | NET-F-30 to 32 | Replaced by full and light roles. | RV-07 |
| net F | NET-F-37 | Dropped. | RV-04 |
| net F | NET-F-38, 39 | Narrowed to own machines. | RV-16 |
| net F | NET-F-40 | Optional. | RV-03 |
| net F | NET-F-43, 44 | Replaced by the mirror role. | RV-10 |
| net T | 3.1, 5.5, 8, 10, 11 | `dist` does not depend on `net` and does not hedge; a crate `node` holds the glue. | RV-06 |
| net T | 8; spec T 2 | `watane-node` is a program of `node`, not of `app`. Eight subsystems. | RV-11 |
| net T | 4.1 | Tickets and codes carry hints; a `watane-friend:` form. | RV-03 |
| net T | 4.2 | Limits in bytes; budgets clamped; request ids. | RV-16, RV-17, RV-26 |
| net T | ND-11 | No relay is proposed at first start. | A6; W23 |
| dist | 4.1, W15 | Two new kinds with rules. | needed |
| dist | 4.2 | A link may name a node (if Q7 is yes). | RV-12 |
| dist | 3, DIST-F-1 | A home can be opened with no identity. | RV-08 |
| log | 2 (`Log::open`) | An open that creates no key. Later `Log::forget`. No format changes. | RV-08, RV-18 |
| spec F | F-QUAL-1, F-APP-6, F-AGENT-4, section 4 | Amendments needed, not deviations: reads "from its web host or a node"; a restore may learn the own head from a trusted machine; the floor names pairing, friends and grants of `net`. | RV-14, RV-23 |

Nothing here leaves `spec/VISION.md`. Three points touch it and are put to
the human: the order of work (Q5), V7 (Q3) and V4's "no operator" for
relays and bootstrap hosts (Q2).

## 14. Build order

Each stage is useful alone and ends in a journey on the real programs.
No stage starts before a step of the vision schedules it (Q5).

| Stage | Build | Journey | Needs from outside |
|---|---|---|---|
| D0 | The decisions Q1 to Q7. Amend VISION (a step), FUNCTIONAL (terms, requirements, a journey), TECHNICAL (subsystems, W13, security rows). | n/a | the human |
| D1 Own machines | `net`: node key, tickets with hints, wire, limits in bytes, `hello`, `heads`, `entries`, `fork`, `blob`, `subscribe`, `offer`, `put`; mDNS and explicit addresses; pairing and the device list. `dist`: the `Holdings` methods, `kept`, an open with no identity. `node`: glue, `watane-node` as a full replica. `app`: turn on, pair, devices. | Alice pairs a home server on her network. It holds her read set. Her second laptop, a light machine, searches through it. | none: one network |
| D2 Friends | Friend tickets, device lists to friends, grants and byte quotas, friend sync, the "checked by" mark. The relay role in `watane-node`; relay choice in the app. | Bob keeps Alice's channel and packages. With Alice's laptop closed, Carol, a friend of Bob, installs from Bob's node. A stale static host is shown as unchecked, and Bob's node shows the newer head. | a machine with a public address, in tests a local relay |
| D3 Strangers | `peer` and `unpeer` with their confirmation; "serve strangers"; the node form of a link; global-address rule; optional DHT. | Dave follows Alice from a link with no static host. A changed byte is refused and named. | the Q7 decision |
| D4 Public mirror | `watane-node mirror`, heads files, `<base>node`; public peers in the app; the public poll. | A mirror serves Alice's channel behind a plain web server. Alice's own host withholds a withdrawal; Carol's app shows it from the mirror, checked by the mirror. | a static host |
| D5 Scale, each on evidence | `Log::forget`; public search; iroh-gossip; range-based set reconciliation; Bao ranges (bao-tree); shards by rendezvous hashing. | one each | measurements that show the need |

## 15. Assumptions

1. The human wants this design kept ahead of the steps. Nothing is built
   until a step schedules it.
2. A personal keep set is small: hundreds to a few thousand channels,
   entries of at most 64 KiB, packages of at most 64 MiB. One always-on
   machine can hold all of it.
3. A participant has few own machines (2 to 5) and few friends who grant
   storage (under 20).
4. iroh 1.x stays able to: dial by Ed25519 key, run with no relay and no
   default address lookup, take addresses from the caller, and embed or
   ship a relay server. Verified today only as far as crates.io shows
   (iroh 1.4.1; `iroh-relay` 1.4.1).
5. iroh needs a relay path to set up hole punching. Not tested in this
   review (RV-03, marked S).
6. `Entry::decode` is enough for `net` to drop garbage early; the holder
   repeats every check.
7. Statements stay `dist`'s. Mods do not write to the channel (V7 as
   written).
8. A static web server or CDN is something a public operator already
   knows how to run reliably.
9. One writer per identity stays the rule (spec T 6, PM-3). Several
   writers are out of scope.
10. A public statement cannot be withdrawn in effect, so any design that
    publishes a node key or an address is treated as irreversible.

## 16. Open questions for a human

Ordered by the sum of three factors, each high (3), medium (2) or low (1):
the risk if the choice is wrong, the cost of postponing it, and the cost
of reverting it later. Ties go to the higher risk.

| # | Question | Options | Recommendation | Risk / postpone / revert |
|---|---|---|---|---|
| Q1 | How is a node bound to a participant? | (a) Private: tickets and an anchor-signed device list. (b) Public `peer` statements, as drafted. (c) An anchor key derived from the participant seed, so a restore keeps it. | (a). Add (c) only if re-pairing after a restore proves painful; it lets a derived secret leave `log`. | high / high / high = 9 |
| Q2 | What ships turned on for reaching nodes? | (a) Nothing: local network, hints, a relay the participant adds. (b) The Mainline DHT, with its bootstrap hosts. (c) iroh's public relays, as ND-11. | (a), with (b) one switch away and named in the consent. (c) is one company's servers. | high / medium / medium = 7 |
| Q3 | May a channel carry kinds that mods define (wishes, voices)? V7 says watane does not carry them. | (a) No: the data plane moves `dist`'s kinds only. (b) Yes: amend V7; each such statement needs a confirmation or a new rule. | (a) until the step for V7 is next. | high / low / high = 7 |
| Q4 | What does a personal node serve to strangers? | (a) Nothing unless turned on, then the own channel only. (b) Everything it holds, as drafted. (c) Also channels marked one by one. | (a), then (c) when asked for. | high / medium / medium = 7 |
| Q5 | When is this built? README rule 3 and the order of work say a later step is not designed in detail. | (a) Add a step 7 "own machines" (stage D1) and keep D2 to D5 as a sketch. (b) Keep all of it designed ahead and amend rule 3. (c) Cut the documents to what keeps the step open. | (a). It keeps the rule and gives the first stage a journey. | medium / high / low = 6 |
| Q6 | Where does a home with no identity come from? | (a) `log` and `dist` gain an open that creates no key. (b) Every home keeps an unused identity. | (a): no signing key on a server that must not sign. | medium / medium / medium = 6 |
| Q7 | How does a link name a channel with no static host? | (a) `watane:<key>@watane-node:<node key>/`. (b) A static host stays required to be found. (c) A new statement kind. | (a), decided before stage D3. Links are published in `list` statements and cannot be recalled. | medium / low / high = 6 |
| Q8 | iroh or rust-libp2p; and how much to rest on young crates? | (a) iroh for transport and relay only; mDNS and DHT behind one trait. (b) iroh with its gossip and blobs crates now. (c) rust-libp2p. | (a). The node key is plain Ed25519, so the transport can change without a new key. | medium / medium / medium = 6 |
| Q9 | Where does `watane-node` live? | (a) A crate `node` with the glue. (b) A binary of `app`. (c) A binary of `net`, which then depends on `dist`. | (a). | medium / medium / low = 5 |
| Q10 | Do friends forward queries to their friends? | (a) No. (b) Two hops, as PM-10. | (a). It shows a query to people the participant never chose. | medium / low / medium = 5 |
| Q11 | What freshness evidence does a public peer give? | (a) Heads files re-signed on a timer. (b) C2SP tlog-witness cosignatures. (c) None; only its `c/<key>/head`. | (a). (b) when several public peers exist and split views matter. | medium / low / medium = 5 |
| Q12 | When does `log` learn to forget? | (a) Before friends' quotas (D2). (b) At D5, with quotas that only count up until then. | (b), stated in the grant view. | medium / medium / low = 5 |
| Q13 | Own machines: roles or shards? | (a) Full and light. (b) Rendezvous hashing, r = 2. | (a). | low / low / low = 3 |
| Q14 | Does a device list expire? | (a) No. (b) A machine stops counting another as trusted when the list is older than 30 days and the anchor has not been seen. | (b) at D2. | low / low / low = 3 |
| Q15 | May a restore take the own head from a trusted machine? | (a) Yes, as one more source beside the link, and the participant still confirms. (b) No, link only. | (a) at D1. | low / low / low = 3 |

### 16.1 Decisions (2026-11-14)

The human decided every question. Each follows the recommendation except
Q7, Q8 and Q14.

| # | Decision |
|---|---|
| Q1 | (a) Private binding: tickets and an anchor-signed device list. |
| Q2 | (a) Nothing on by default; the DHT is one switch away and named in the consent. |
| Q3 | (a) The data plane moves `dist`'s kinds only, until the step for V7 is next. |
| Q4 | (a) Nothing to strangers unless turned on, then the own channel only. |
| Q5 | (a) A step "own machines" (stage D1), numbered 8 because `factory/PLAN.md` already has a step 7; D2 to D5 stay a sketch. |
| Q6 | (a) `log` and `dist` gain an open that creates no key. |
| Q7 | Changed: a channel is named by a flexible URI with resolvers. The first resolver reads http(s) URLs; resolvers for nodes and others come later. |
| Q8 | Changed: (b) iroh with its gossip and blobs crates. |
| Q9 | (a) A crate `node` holds the glue and builds `watane-node`. |
| Q10 | (a) Friends do not forward queries. |
| Q11 | (a) Heads files re-signed on a timer. |
| Q12 | (b) `log` learns to forget at D5; quotas only count up until then. |
| Q13 | (a) Full and light machines. |
| Q14 | Changed: expiry is a setting of the device list; the default 0 means it never expires. |
| Q15 | (a) Yes, at D1, and the participant still confirms. |

## 17. Sources

- Saltzer, Reed, Clark. End-to-end arguments in system design. ACM TOCS 1984.
- Demers et al. Epidemic algorithms for replicated database maintenance. PODC 1987.
- Tarr, Lavoie, Meyer, Tschudin. Secure Scuttlebutt. ACM ICN 2019.
- Samuel, Mathewson, Cappos, Dingledine. Survivable key compromise in software update systems (TUF). ACM CCS 2010.
- Laurie et al. Certificate Transparency. RFC 6962, RFC 9162. Syta et al. CoSi. IEEE S&P 2016. Sigsum; C2SP tlog-witness.
- Dean, Barroso. The Tail at Scale. CACM 2013. RFC 6298 (retransmission timer).
- Shreedhar, Varghese. Efficient fair queueing using deficit round robin. SIGCOMM 1995.
- Ford, Srisuresh, Kegel. Peer-to-peer communication across network address translators. USENIX ATC 2005. RFC 9000 (QUIC). RFC 6762 (mDNS).
- Douceur. The Sybil Attack. IPTPS 2002. Heilman et al. Eclipse attacks. USENIX Security 2015.
- Leitão, Pereira, Rodrigues. HyParView (DSN 2007); Plumtree (SRDS 2007). Meyer. Range-Based Set Reconciliation. SRDS 2023.
- Thaler, Ravishankar. Using name-based mappings to increase hit rates. IEEE/ACM ToN 1998.
- O'Connor et al. BLAKE3; Bao. BEP 5, BEP 44; Pkarr.
- Stiegler. An introduction to petname systems. 2005. Kleppmann et al. Local-first software. Onward! 2019.
- iroh 1.4.1, iroh-relay 1.4.1, iroh-mdns-address-lookup 0.6.1, iroh-mainline-endpoint-discovery 0.1.6, bao-tree 0.17.0 (crates.io, 2026-11-14).
