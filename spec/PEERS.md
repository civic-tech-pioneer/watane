# The peer model and the data plane

How watane moves channels, packages and queries between machines, beside
the static hosts of today. Step 8 (own machines, `VISION.md`) builds the
first part (stage D1, section 10). The rest is a sketch: it keeps the later
stages open and is marked as such (README rule 3).

The decisions behind this file are in `../net/ARCHITECTURE-PROPOSAL.md`
16.1 (2026-11-14), after the review in `../net/REVIEW.md`. Section 11 lists
what this file changes in the rest of `spec/`.

## 1. Goals

- **PG-1** The core needs no public peer: only the participant's own
  machines, friends and strangers. Where every machine is behind NAT, the
  participant adds a relay or turns the DHT on, and the app says so (8.1).
- **PG-2** A public peer is optional. It makes things faster and more
  available, and it is simple to run: a job that writes files that any web
  server serves (D4).
- **PG-3** Every machine moves the same signed objects. Nothing is re-signed
  or translated on the way.
- **PG-4** A machine trusts another for exactly what it cannot verify, and
  only as far as the class of that machine allows.
- **PG-5** No participant has to hold the whole network.
- **PG-6** Every function keeps working offline and with `net` off, over
  what the device already holds (F-QUAL-1).
- **PG-7** Nothing ties a participant to a machine, an address or a read
  set unless the participant chose to publish it (V5).

## 2. Principles

| # | Principle | From |
|---|---|---|
| A1 | Verify everything that can be verified, at the machine that uses it. Trust a class only for the rest. | Saltzer, Reed, Clark 1984 |
| A2 | Every source can only raise a head: `log` takes an entry only when it extends the chain. So sources need no coordination, and the highest valid head wins. | `log` 5.2 |
| A3 | An answer about absence or completeness is a statement of one machine at one time. It is shown with its source, never as a fact. | rule of claims |
| A4 | The participant names machines; no global name or rating exists. | V6; petnames (Stiegler 2005) |
| A5 | Nothing is published about a participant that they did not choose to publish. | V5 |
| A6 | A request costs its sender more to forge than the receiver to refuse. Limits are in bytes and connections, checked before work. | V5 |
| A7 | No built-in endpoint: no relay, bootstrap host, index or public peer is on unless the participant turns it on. | V4, W23 |
| A8 | The simplest mechanism that works for ten machines and a thousand channels. Each scaling step is named, and none is built early. | order of work |

## 3. Terms

| Term | Meaning |
|---|---|
| participant key | The Ed25519 key of a participant (`log`). It signs channel entries and nothing else. |
| node | One running `net`: in the app's home, or in `watane-node`. |
| node key | The Ed25519 key of a node. It names the node on the network and signs only the forms of `net/TECHNICAL.md` 4.3. |
| anchor | The node in the home that holds the participant key. It signs the device list. |
| own machine | A node in the device list that the participant's anchor signed. Class trusted. |
| full machine | An own machine that keeps every channel and package that any own machine holds. |
| light machine | An own machine that keeps what its own home uses, and asks a full machine for the rest. |
| device list | The anchor's signed list of own machines (4.2). Never published. |
| ticket | Text that names a node by key, with hints of where to dial it now (4.3). |
| class | How this participant rates a node: unknown, public, friendly or trusted (section 5). Local, never published. |
| source | A place to read a channel: a link's location (a base URL today) or a node. |
| hint | Something a node said that is not verified, or cannot be. |
| checked | A head that a source this participant rates confirmed at a time (section 7). |

## 4. Objects

The narrow waist. Every transport carries these bytes unchanged, and every
receiver verifies them the same way, whatever their source.

### 4.1 Unchanged

An entry (`log` 4.3, 4.4). A channel: the hash chain of one author. A
package, named by its BLAKE3 hash (`pkg`). The site folder (`dist` 4.3).
A fork proof: two entries of one author that cannot both be in one chain.

The statement kinds stay `dist`'s. The data plane moves `dist`'s kinds
only; kinds that mods define wait for the step of V7.

A link names a channel by key and a location (W31, `dist` 4.2). The
location is a URI, read by the resolver of its scheme. Today the only
resolvers are `http` and `https`. A node resolver (`watane-node:<node
key>`) comes with stage D3.

### 4.2 The device list

Signed by the anchor's node key, form and prefix in `net/TECHNICAL.md` 4.3:
anchor, seq, time, `expires`, and the own machines with their roles (full
or light). It travels only over connections between own machines (and, from
D2, to friends). It is never published.

`expires` is a setting of the participant, in days. 0, the default, means
the list never expires. Otherwise a machine stops treating the others as
trusted when its newest list is older than `expires` and it has not reached
the anchor since. So a lost machine that nobody removed stops counting on
its own.

### 4.3 Tickets and codes

A ticket names a node key and carries **hints**: a relay URL, a socket
address. Hints are for the first connection only. A pairing code is a
ticket of the anchor with a 128-bit secret. Grammar in `net/TECHNICAL.md`
4.1. This is what iroh's own tickets carry.

### 4.4 Not used

No cosignature, no signed grant, no signed receipt, no signed query answer,
and no `peer` statement in the core. A grant is a record of its issuer
(D2). Section 10 names where signed forms would come back.

## 5. Classes

A participant puts every node they deal with into one class. The class
changes three things only: what this machine **asks** of the node, what it
**serves** the node, and which of its answers count as **checked**. It
never changes the objects or the protocol.

| Class | Who | Enters by | Bound to the participant by | Leaves by |
|---|---|---|---|---|
| unknown | Any node. Keys are free. | default | nothing | n/a |
| public | A well-known service the participant chose. | adding its ticket in the app (D4) | the participant's choice | removal |
| friendly | A node of a person the participant knows. | adding a friend's ticket in the app (D2). The ticket names the friend's anchor; each node of the device list it signs is friendly too. | the ticket, handed over out of band | removal; the friend's newer device list |
| trusted | An own machine. | pairing (6.1) | the pairing code | a newer device list; expiry (4.2) |

The class is directional: each node also rates its callers and serves them
by that class (6.3). Friends exchange tickets both ways.

Three facts, three mechanisms, none of them public: an own machine is a
line in the device list; a friend's node is a ticket exchanged by hand; a
public peer is a ticket the participant added. No class follows from a
public statement.

### 5.1 What each class is trusted for

| Guarantee | unknown | public | friendly | trusted |
|---|---|---|---|---|
| **Authenticity** | signature | signature | signature | signature |
| **Freshness** | only raises a head | checked by its signed heads file (7) | checked by its answer | checked by its answer |
| **Completeness** | never | never | stated, shown with its source | stated, shown with its source |
| **Availability** | best effort | as it publishes | by grant | full |
| **Computation** (a query) | never asked | hints, each hit verified | hints, each hit verified | hits, each hit verified |
| **Storage for me** | never asked | never asked | by grant, within a quota | full or light role |

Authenticity never depends on the class. A design pushes each operation to
the cheapest class that still gives what the operation needs.

## 6. Interfaces

### 6.1 Own machines (D1, built)

**Pairing.** The participant asks the anchor for a pairing code (valid 10
minutes, one use). On the joining machine, `watane-node join <code>` or the
app's join view dials the anchor by key, using the code's hints. The anchor
shows the short key of the joining node, and the participant confirms in
the app. The anchor then signs a new device list with the joiner and its
role, and pushes it to every own machine. A joining home is opened with no
participant identity: a replica holds no signing key (W33).

**Removal.** The anchor signs a list without the machine. Every own machine
stops serving it as trusted once it verifies the newer list.

**Lost anchor.** The participant restores the identity on a new home, which
becomes the anchor with a new node key, and pairs each machine again
(`join --replace`, confirmed on that machine). The restored home may then
learn the own head from a full machine; the participant still confirms it
(F-APP-6).

### 6.2 Between nodes

QUIC through iroh, dialed by node key. The caller's key comes from the
handshake, never from a request. Three protocols, each its own ALPN:

| Protocol | For | Defined in |
|---|---|---|
| `watane/1` | requests: `hello`, `heads`, `entries`, `fork`, `query`, `devices`, `pair` | `net/TECHNICAL.md` 4.2 |
| iroh-blobs | packages, by BLAKE3 hash, with verified streaming | iroh-blobs (Bao) |
| iroh-gossip | new heads, as hints, to the machines of one topic | iroh-gossip (HyParView, Plumtree) |

| Op | Asks | Answer |
|---|---|---|
| `hello` | the version | version, operations, limits; for a trusted caller also the device list seq |
| `heads` | a list of channels; empty means all, trusted callers only | for each: seq, entry id, forked; or not held |
| `entries` | channel, from, limit | the stored entries, original bytes |
| `fork` | channel | the two entries of a fork, if any |
| `query` | a `dist` 5.10 query | hits (channel, seq, entry id), each verified by the asker |
| `devices` | the seq the caller has | a newer device list, and the last good addresses of its machines |
| `pair` | the secret of a code | the new device list |

Sync is **pull only**: a machine learns a head (`heads`, or a gossip hint),
reads what it lacks with `entries`, and fetches the packages its role
needs with iroh-blobs. Nobody pushes entries, so no one can fill another's
store. A change to an operation is a new name; an unknown name answers
`unknown_op`.

### 6.3 What a node serves, by the caller's class

Each column is an **allowance**: what a peer may use, over every protocol,
and its budget in bytes and connections. A class only chooses the
allowance; a grant widens it for one friend (D2). Every check runs before
any work: allowance, budget, decode, size.

| Operation | trusted | friendly (D2) | public, unknown |
|---|---|---|---|
| `hello` | everything | everything | version, operations, limits |
| `heads`, `entries`, `fork`, blobs | everything held | the own channel; channels a grant names | nothing; from D3 the own channel, only when "serve strangers" is on |
| gossip | the own-machines topic | topics a grant names | none |
| `query` | yes | by grant | no |
| `devices` | yes | yes | no |
| `pair` | n/a | n/a | with a valid code only |

`net` is off until the participant turns it on. A personal node tells
strangers nothing about other channels, ever.

## 7. Verification and freshness

Authenticity never depends on the class: `Entry::decode`, the chain, and
BLAKE3 for packages, at the machine that uses the bytes. `dist` verifies
everything it ingests, again.

A head is **checked** at a time when one of these holds:

| Source | Class | Evidence |
|---|---|---|
| the publisher's link location gave `Current` | any | today's rule (`dist` 5.4) |
| an own machine answered `heads` with the same head | trusted | authenticated connection |
| a friend's node answered `heads` with the same head (D2) | friendly | authenticated connection |
| a chosen public peer's heads file names the same head within two periods (D4) | public | signature of its node key |

Any other source only raises the head. The app shows which sources checked
a head and when. This is the timestamp role of TUF (Samuel et al., CCS
2010): someone other than the withholding host states the newest version,
for a short time. A fork proof needs no class: any node that holds one
passes it on, and `log` marks the channel.

## 8. Choosing nodes

### 8.1 Reaching a node

In this order: an address on the local network (mDNS); the last good
addresses this node stored; addresses that an own machine passed on
(`devices`); the hints of the ticket or code; a relay the participant
chose; the Mainline DHT, when the participant turned it on.

| Situation | Works with |
|---|---|
| own machines on one network | mDNS only |
| one own machine with a public address | that machine, as a direct address and as relay (`watane-node` runs iroh-relay) |
| every machine behind NAT | nothing in the core: the participant adds a relay, or turns the DHT on. The app says so. |

The DHT is one switch away, and the consent names it: the node then
publishes its addresses under its node key (pkarr). A relay sees who talks
to whom, but only encrypted QUIC. No relay and no DHT is on by default
(A7).

### 8.2 The caller

| Need | Asked, in order | Stops when |
|---|---|---|
| read entries or a package | own machines; friends that hold it (D2); the link's location; chosen public peers (D4) | the bytes verify |
| freshness of a channel | in parallel: the link's location; one full own machine; one friend that holds it (D2); one public peer's heads file (D4) | all answered, or 5 s |
| query | this home; a full own machine when this one is light; friends only when the participant ticks them (D2) | each asked node answered, or 5 s |
| keep | the full own machines (by role); friends by grant (D2) | n/a |

Inside one class, nodes are ordered by measured success and latency (a mean
and a mean deviation, RFC 6298); `net` starts the next node when one has
not answered by mean + 4 × deviation, at most 2 in flight (Dean and Barroso
2013). Across classes, privacy decides: an own machine learns nothing new,
a friend learns one interest, a publisher's host learns an address.

A remote hit is a hint. The asker verifies it by reading its channel from
the node that gave it, at most 32 channels per query.

### 8.3 A node that asks others

| Node | May ask others for a request it serves |
|---|---|
| a light own machine | no: it answers from what it holds |
| a full own machine | no: it syncs on its own and answers with its heads, so the caller sees how current it is |
| a friend's node (D2) | no: friends never forward to their friends |
| a public peer (D4) | no: it pulls on its own schedule |

No request is forwarded in the core. So no budget, no hop count and no
amplification exist to bound.

## 9. Processes

| Process | Between | Over | Trigger | Stage |
|---|---|---|---|---|
| own-machine sync | every pair of own machines | `heads`, `entries`, iroh-blobs | connect; a gossip hint; every 10 minutes | D1 |
| new-head hints | own machines | iroh-gossip, one topic per anchor | an own append; a raised head | D1 |
| address upkeep | own machines | `devices` | connect; an address change | D1 |
| local discovery | nodes on one network | mDNS (RFC 6762) | while `net` is on | D1 |
| DHT record | this node and the Mainline DHT | BEP 44, pkarr | only when turned on | D1 |
| follow by link | `dist` and link locations | HTTP GET | the app's timer (APP-F-36) | today |
| friend sync | this node and each friend's node | as own-machine sync, for granted channels | the same | D2 (sketch) |
| public poll | this node and chosen public peers | HTTP GET with `If-None-Match` | the peer's period, at least 60 s | D4 (sketch) |
| mirror job | a public peer and the sources it admits | HTTP GET | its timer | D4 (sketch) |

A head vector of a thousand channels is about 75 KiB. Range-based set
reconciliation (Meyer, SRDS 2023) replaces the vector when it stops
fitting (D5).

## 10. Stages

Each stage is useful alone and ends in a journey on the real programs. D1
is step 8 (own machines) and is designed here and in `../net/`. D2 to D5
are a sketch: a later step designs each one.

| Stage | What | Journey | Status |
|---|---|---|---|
| D1 Own machines | `net`: node key, tickets with hints, `watane/1`, iroh-blobs, iroh-gossip, limits in bytes, pairing, device list with expiry, mDNS, addresses, relay choice, the DHT switch. `dist`: the holder methods, a keyless open. `log`: a keyless open. `node`: the glue and `watane-node` (full replica, relay role). `app`: turn on, pair, devices, relay, DHT switch. | J8 (`FUNCTIONAL.md`) | built (J8 passes, S75) |
| D2 Friends | friend tickets; device lists to friends; grants with byte quotas; friend sync; the "checked by" mark; friends asked for queries when ticked | Bob keeps Alice's channel; with Alice's laptop closed, Carol installs from Bob's node; a stale host shows as unchecked | sketch |
| D3 Strangers | "serve strangers" (own channel only); the node resolver for links; global addresses only for unrated nodes | Dave follows Alice from a link with no static host; a changed byte is refused and named | sketch |
| D4 Public peer | `watane-node mirror`: a job that verifies the channels it admits and writes their site folders and a heads file, re-signed on a timer, for any web server; optional search and relay roles; public peers in the app | a mirror serves Alice's channel behind a plain web server; Alice's host withholds a withdrawal; Carol sees it, checked by the mirror | sketch |
| D5 Scale, each on evidence | `Log::forget` (quotas only count up until then); range-based set reconciliation; Bao ranges from several nodes; public search; witness cosignatures when several public peers exist | one each | sketch |

## 11. Effect on the rest of `spec/`

| Where | Change |
|---|---|
| `VISION.md` | Step 8, own machines. Nothing else: V4 holds (no endpoint on by default), V7 holds (no mod-defined kinds). |
| `FUNCTIONAL.md` | Terms, F-OWN requirements and J8 for step 8; F-QUAL-1 and F-APP-6 name own machines. |
| `TECHNICAL.md` | Subsystems `net` and `node`; W10, W13; W31 (links name a location URI read by a resolver); W32 (`net` off by default, no endpoint on); W33 (a replica holds no participant key); build order row 8; security rows. |
| `log` | A keyless open (step 8). `Log::forget` at D5. No format change. |
| `dist` | The link location is a URI with resolvers (now, W31). The holder methods and a keyless open (step 8). No new kind. |
| `host` | Reserves `<home>/net/`. |
| `app` | The views of D1: turn `net` on, pair, devices, relay, the DHT switch. |

## 12. Threats

| Threat | Answer | Left open |
|---|---|---|
| Forged entry or package | `Entry::decode`, the chain, BLAKE3, at the machine that uses it | none |
| A host or node withholds new entries | several sources raise the head; rated nodes state their head (7); the app names who checked | a channel that only its publisher's host and strangers hold |
| Fork | `log`'s rule; `fork` passes the proof on | none |
| Sybil keys | no rule counts nodes; unknown callers get nothing in D1 and one small byte pool later | fair service among honest strangers during a flood (D3) |
| Eclipse | own machines are reached from pinned keys and stored addresses, not only from the DHT | a participant with no rated node that holds the channel |
| Amplification | pull only; no forwarding; a gossip hint causes reads only from own machines | none known |
| Probes of the local network | listed locations as `dist` 5.9; unrated nodes are dialed on global addresses only (D3) | a host name that an HTTP CONNECT proxy from the environment resolves (`dist` 4.5) |
| Exhaustion | byte, connection and stream limits per key and per class, before work | an own machine has no limit |
| Link from a participant to an address | no public statement names a node; no participant key in `hello`; the DHT and relays off unless chosen | a participant who turns on the DHT publishes a node key's addresses; one who serves strangers (D3) publishes that node key |
| A stolen pairing code | 128 bits, 10 minutes, one use, and the participant confirms the joiner's short key | none |
| A compromised own machine | it holds no participant key; packages are verified where they run; a newer device list removes it | it can withhold and lie about completeness until removed; an offline machine trusts it until it sees the new list, or until `expires` when set |
| A relay | sees encrypted QUIC | who talks to whom, and when |
| A local program | no `net` route on the local API; pairing and every class change are confirmed in the app | another program of the user can read `node.key` (spec T 6) |

## 13. Sources

- Saltzer, Reed, Clark. End-to-end arguments in system design. ACM TOCS 1984.
- Demers et al. Epidemic algorithms for replicated database maintenance. PODC 1987.
- Tarr, Lavoie, Meyer, Tschudin. Secure Scuttlebutt. ACM ICN 2019.
- Samuel, Mathewson, Cappos, Dingledine. Survivable key compromise in software update systems (TUF). ACM CCS 2010.
- Laurie et al. Certificate Transparency. RFC 6962, RFC 9162. Syta et al. CoSi. IEEE S&P 2016. Sigsum; C2SP tlog-witness.
- Leitão, Pereira, Rodrigues. HyParView (DSN 2007); Plumtree (SRDS 2007). Meyer. Range-Based Set Reconciliation. SRDS 2023.
- O'Connor et al. BLAKE3; Bao verified streaming.
- RFC 9000 (QUIC). RFC 6762 (mDNS). Ford, Srisuresh, Kegel. Peer-to-peer communication across network address translators. USENIX ATC 2005.
- BEP 5, BEP 44; Pkarr. iroh, iroh-blobs, iroh-gossip, iroh-relay.
- Dean, Barroso. The Tail at Scale. CACM 2013. RFC 6298.
- Douceur. The Sybil Attack. IPTPS 2002. Heilman et al. Eclipse attacks. USENIX Security 2015.
- Stiegler. An introduction to petname systems. 2005. Kleppmann et al. Local-first software. Onward! 2019.
- Nostr NIP-65 (relay lists); AT Protocol (credible exit). Syncthing (untrusted devices).
