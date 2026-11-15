# net: adversarial review of the peer model and the data plane

Reviewed on 2026-11-14. Subject: `spec/PEERS.md`, `net/VISION.md`,
`net/FUNCTIONAL.md`, `net/TECHNICAL.md`, `net/AGENTS.md`, and the edits to
`spec/README.md`, `spec/TECHNICAL.md` and `host/TECHNICAL.md`. Checked
against `spec/VISION.md`, `spec/FUNCTIONAL.md`, `spec/TECHNICAL.md`,
`log/VISION.md`, `log/TECHNICAL.md` (2, 4, 5, 7, 10, 11), `dist/VISION.md`,
`dist/AGENTS.md`, `dist/TECHNICAL.md` (1 to 6, 12, 13), the workspace
manifests, and crates.io.

This file reviews. `ARCHITECTURE-PROPOSAL.md` beside it answers.

**Status (2026-11-14).** A record, not a contract. The human decided the
proposal's open questions (`ARCHITECTURE-PROPOSAL.md` 16.1), and
`spec/PEERS.md` and the `net` documents were rewritten to them. The
findings cite the old section and requirement numbers. Resolved by the
rewrite: RV-01 to RV-30, RV-32, RV-33, RV-35 to RV-41, RV-43, RV-45 to
RV-55 (most by removal: no `peer` statement, cosignature, shard,
forwarding, public mode or HTTPS front in D1). Moved to a later stage's
design: RV-34 and RV-42 (D5), RV-44 (the DHT switch, D1, as a choice).
RV-31 is fixed in `log/TECHNICAL.md` 10.

## 1. Verdict

The object model is sound: every transport moves `log` entries and BLAKE3
blobs unchanged, and no class is trusted for authenticity. The citations
are almost all correct. The design fails in four places:

1. **Identity binding.** One public statement kind, `peer`, carries three
   meanings: "a place to read", "my own machine" and "treat as my friend's
   node". This publishes what should be private and lets a friend raise a
   third party to the friendly class (RV-01, RV-02).
2. **Reachability.** The core promise "works with no public peer" rests on
   a relay and a DHT that the design treats as details. Tickets carry no
   address, and the stage that makes nodes reachable comes last (RV-03).
3. **The public node.** It is the whole program with every role turned on,
   over a store that cannot delete. That is neither simple nor reliable
   (RV-10, RV-11).
4. **Fit with `dist`.** The holder must hedge without threads, is called
   from inside a runtime it may not block, and gets an identity it must
   not have (RV-06, RV-08).

Counts: 3 critical, 22 major, 23 minor, 7 nit. 55 findings.

**Mark.** V = verified against the repo text, by arithmetic, or on
crates.io. S = suspected: it rests on behaviour outside the repo that this
review did not run.

## 2. Summary, most severe first

| Id | Sev. | Cat. | Where | Finding | Mark |
|---|---|---|---|---|---|
| RV-01 | critical | error | PEERS 4.1, 5.1, 6.4; net T 5.2, 5.4 | A participant's key, node keys, IP addresses and read set become public and linked | V |
| RV-02 | critical | error | PEERS 4.1, 4.3; NET-F-7; net T 5.3, 5.13 | `peer` means source, own machine and friendly node at once; a friend can raise any node to friendly | V |
| RV-03 | critical | goal-fit | PEERS 9, 10, PG-1; net T 4.1, 5.16, 9, ND-11 | No address in a ticket or pairing code; reachability is stage N7; no relay means no hole punching; PG-1 does not hold as written | V, part S |
| RV-04 | major | error | PEERS 5.5; net T 5.12 | Cosignatures give no freshness, and a reader cannot count the publisher's witnesses | V |
| RV-05 | major | error | net T 5.4, 5.7 | An unsolicited `announce` from an unknown caller makes the node fetch and fall back to other sources | V |
| RV-06 | major | coherence | net T 5.5, 8, Q4; dist 1, 3, AGENTS 7 | Hedging sits in the holder, which has no thread; `ingest` to `announce` re-enters a blocking API from the runtime | V, part S |
| RV-07 | major | goal-fit | PEERS 7.2; net T 5.10, 5.11 | Sharding splits nothing: the primary holds the whole keep set | V |
| RV-08 | major | coherence | net T 5.9, 8; dist 3; log 2 | Every home gets a participant identity, so every replica looks like a primary and holds a signing key | V |
| RV-09 | major | error | net T 5.11; dist 5.9; W21 | Remote hits make the caller read up to 200 channels per answer; the read-set bound is gone | V |
| RV-10 | major | goal-fit | PEERS 10.1, PG-2, PM-11; net T 5.14 | The public node is not simple: no admission rule, a store that cannot delete, linear search, no TLS story, no owner for the crawl | V |
| RV-11 | major | coherence | spec T 2; net T 8 | `watane-node` as an `app` binary compiles Tauri; the "Depends on" cells are not updated | V |
| RV-12 | major | completeness | dist 4.2; PEERS 12 | A link needs an http(s) base, so a channel with no static host cannot be followed or listed | V |
| RV-13 | major | coherence | spec README rule 3; VISION order of work; PEERS intro | 1 600 lines designed ahead, against the rule that PEERS cites | V |
| RV-14 | major | coherence | spec FUNCTIONAL; PEERS 12 | No parent functional requirement; F-QUAL-1, F-PUB-6, F-APP-6, F-AGENT-4 not amended | V |
| RV-15 | major | coherence | VISION V7; PEERS intro, 5.6, 5.7, 6.2 | Wishes and voices as channel statements, against "watane itself does not carry these statements" | V |
| RV-16 | major | error | net T 4.2, 5.11 | Forwarding: budget set by the caller and not clamped; no request id; unknown callers may trigger it | V |
| RV-17 | major | error | net T 4.2 | Limits count requests, not bytes or connections | V |
| RV-18 | major | coherence | log 4.8, 5.3; net T 5.9, 5.10, 5.13 | `release`, quotas and "drops its shards" cannot free space: `log` never deletes | V |
| RV-19 | major | completeness | net F 3 | 12 of 48 requirements have no scenario | V |
| RV-20 | major | completeness | net T vs net F | Design with no requirement, and requirements with thin design | V |
| RV-21 | major | coherence | NET-F-13; net T 3, 5.4, 5.11 | Grant and class policy contradict each other | V |
| RV-22 | major | error | PEERS 8.1 step 4; NET-F-18; net T 5.5 | "Two origins" counts free keys; a fresh read with one source is undefined | V |
| RV-23 | major | completeness | PEERS 7.1; net T 3, 5.9; F-AGENT-4 | Pairing, friends, grants and classes have no confirmation rule and no local-API rule | V |
| RV-24 | major | completeness | net V, F; F-APP-2; W23 | Nothing says whether `net` is on by default, or what a first start contacts | V |
| RV-25 | major | completeness | net T 5.9, 4.2, 4.5 | Lifecycle gaps: lost primary, stale device list, node key change, compromised friend, protocol and file versions | V |
| RV-26 | minor | error | net T 4.2, 5.8 | A `have` of 4 096 channels is about 170 KiB; the request limit is 64 KiB | V |
| RV-27 | minor | coherence | PEERS 4.3, 6.1; net T 4.2, Q3 | Operation lists disagree; Q3 cites an `assign` that PEERS does not have | V |
| RV-28 | minor | coherence | PEERS 3, 4.3, 4.4 | "A grant signed by the participant key" contradicts PM-3 | V |
| RV-29 | minor | coherence | PEERS 4.3, 4.4, 6.1; net V, F 1, T Q2 | Signed grants, receipts and answers in four places; unsigned in the contract | V |
| RV-30 | minor | error | net T 6; AGENTS 5 | "The node key signs only the two forms" is false: iroh signs handshakes and records with it | V |
| RV-31 | minor | coherence | log 4.5, 10; log V | `log` says it is the only signature check; its provides table has no `net` | V |
| RV-32 | minor | coherence | spec T W1, 2, 5, 6, 8, 9; net T Q5 | The spec edits are partial; Q5 says the diagram was updated and it was not | V |
| RV-33 | minor | coherence | PEERS 12; spec T 6 | "It has no confirmation, so it never signs" against a witness that signs | V |
| RV-34 | minor | error | PEERS 4.2, 6.1; dist 4.3 | A static host cannot give a Bao proof; "16 KiB groups" is bao-tree, not Bao | V |
| RV-35 | minor | error | PEERS 9; net T 2 | iroh facts: discovery is in other crates, one of them six weeks old | V, part S |
| RV-36 | minor | error | PEERS R5; NET-F-45; net T 5.15 | QUIC addresses are exempt from the local-probe rule | S |
| RV-37 | minor | error | net T 5.7, 4.2 | Dedupe by (channel, seq) hides a fork; no way to pass a fork proof | V |
| RV-38 | minor | error | net T 5.5 | A 95th percentile cannot come from a moving average; the `Fresh` pick is ambiguous | V |
| RV-39 | minor | completeness | PEERS 4.1 | `peer` has no statement rules: cap, duplicates, a node that did not consent | V |
| RV-40 | minor | completeness | net T 3.1, 5.6; dist DD-12 | Packages of withdrawn releases stay served | V |
| RV-41 | minor | error | net T 5.14 | A 64 KiB filter in a URL; `POST /witness` cannot read from its caller | V |
| RV-42 | minor | error | PEERS 5.7 | A SQLite file from an untrusted node is an attack surface | S |
| RV-43 | minor | completeness | net T 3, 4.5, 5.1, 5.7 | Relays in two places; `events()` and `subscribe` under-specified | V |
| RV-44 | minor | error | PEERS 5.1, 11 | The DHT is safe for integrity, not for availability | S |
| RV-45 | minor | error | net T 5.12 | A witness that loses its store can cosign a conflicting head | V |
| RV-46 | minor | coherence | PEERS 3; net T 5.4 | "Public" names a class of node and a kind of client | V |
| RV-47 | minor | completeness | net AGENTS | Rules missing; PEERS's place in the chain is defined only here | V |
| RV-48 | minor | coherence | host T 3.1, 16; spec T 5 | `net/` reserved in `host`, not in `spec`; host's provides table has no `net` | V |
| RV-49 | nit | error | PEERS 5.2, 5.3, PM-6 | EBT and Plumtree are listed as two mechanisms; one is the other applied to feeds | V |
| RV-50 | nit | coherence | PEERS 5.1 | "All of them in the core" includes a public index | V |
| RV-51 | nit | coherence | net F header, NET-F-17, 45, 48 | Citation rule broken; weak traces | V |
| RV-52 | nit | error | PEERS 5.5, 15 | CT has no witnesses; CONIKS is cited and unused | V |
| RV-53 | nit | error | PEERS 8.1 step 5 | Power of two choices is for load, not for latency rank | V |
| RV-54 | nit | coherence | PEERS 12 | 7.4 is an open question, not an answer | V |
| RV-55 | nit | coherence | PEERS 6.3; dist DD-12 | "Cache for ever" against removed packages | V |

## 3. Findings on `spec/PEERS.md`

### RV-01 A participant's key, node keys, addresses and read set become public and linked
- **Where.** PEERS 4.1 (`peer`), 5.1, 6.4; net T 5.2 (`hello`), 5.4, 5.16; NET-F-5, NET-F-7, NET-F-13.
- **Severity, category.** Critical, error (privacy leak). Verified.
- **Finding.** Four steps link a person to their network address and reading habits, for any stranger, for ever.
  1. A node counts as a participant's own only when the participant's public, append-only channel names its node key in a `peer` statement (NET-F-7, net F term "own node").
  2. The node publishes its addresses under that node key in the Mainline DHT (NET-F-40, 5.16).
  3. `hello` tells any caller the participant key of the home (NET-F-5).
  4. An unknown caller may ask `head`, `entries`, `blob`, `have` and `query` over everything the holder holds (5.4 table). "Reads answer only what the holder holds" is exactly the leak: a probe of key X answers whether this participant reads X.
- **Evidence.** net T 5.4: `have` with channels, `query`, `head`: "yes" for Unknown. net T 6 claims "Privacy of who keeps whom" through `locate` alone. Today a follow list is private: only a static host sees a reader's requests. A `peer` statement cannot be unpublished (log 5.3: "Nothing deletes an entry").
- **Fix.** Bind own and friends' nodes privately (tickets, a device list handed over a connection). Use `peer` only for a node that its owner wants strangers to read from. Serve an unknown caller only the own channel and channels marked for public serving. Drop the participant key from `hello` for unknown callers. No `query` and no multi-channel `have` for unknown callers on a personal node.

### RV-02 `peer` means three things, and a friend can raise any node to friendly
- **Where.** PEERS 4.1, 4.3 (last paragraph); NET-F-7; net T 5.3 step 2, 5.13.
- **Severity, category.** Critical, error (trust escalation) and goal-fit. Verified.
- **Finding.** PEERS 4.1 defines `peer` as "this node serves my channel", a source "of the same standing as the link's base". PEERS 4.3 uses the same statement to tie a participant's own nodes to them. net T 5.3 makes every node in a friend's live `peer` statements friendly, and 5.13 gives each of them the friend's grant.
  - Bob names a commercial mirror as `peer`, as 4.1 invites. That mirror is now friendly for everyone who marked Bob a friend. Its head answers are "trusted, spot-checked" (PEERS 3.1), its query answers are "accepted", and it may `offer` and `keep` on their nodes within Bob's grant.
  - Alice's laptop is recognised by Bob's node only when Alice's channel names it. So every device that wants friendly service must be published as a public source, and strangers will then read from it.
  - The class is directional (PEERS 3), so both sides must do this.
- **Evidence.** net T 5.3: "In `holder.own_nodes(p)` for a friend `p` of `peers.json`: `Friendly`." net T 3.1: `own_nodes` is "live `peer` statements". Nothing separates a mirror from an own machine.
- **Fix.** Three separate facts, three mechanisms: a public source (`peer`, in the channel); an own machine (the device list); a friend's node (a ticket exchanged out of band, plus the friend's device list received over an authenticated connection). A class never follows from a public statement.

### RV-03 Reachability is assumed, not designed, and PG-1 does not hold as written
- **Where.** PEERS 9 (Connectivity), 10, PG-1; net T 4.1, 3, 5.9, 5.16, 9, ND-11; NET-F-40 to 42.
- **Severity, category.** Critical, goal-fit and completeness. Verified, except the iroh behaviour (S).
- **Finding.**
  1. A ticket is `watane-node:` key and an optional HTTPS base. A pairing code is a key and a secret. Neither carries an address or a relay URL. `Net::head(node: &NodeKey, …)` takes no address. So a node is dialable only through discovery.
  2. Discovery (DHT, mDNS) and relays are stage N7. Pairing is N5, friends are N6, reads are N3. Stages N3 to N6 cannot run between two real machines. The test plan binds nodes to loopback and does not say how one finds the other.
  3. (S) In iroh, hole punching is coordinated over the relay path. With no relay, two nodes behind NAT do not connect. net T 5.16 admits "two nodes connect only directly"; PEERS 9 says "hole punching where NATs allow it and a relay where they do not", which reads as if punching needs no relay.
  4. The Mainline DHT needs bootstrap hosts with fixed names. That is a well-known, long-lived service: the definition of a public peer (PEERS 3). PEERS 10 lists "node addresses from the DHT" under "without public nodes".
  5. ND-11 proposes iroh's public relays at first start. Those are one company's servers. W23 ships no index and no default channel for this reason.
- **Evidence.** net T 4.1 grammar; net T 9 order; iroh 1.4.1 depends on `iroh-relay` and `iroh-dns` (crates.io). No sentence in PEERS 10 names what the core needs to connect two laptops in two homes.
- **Fix.** Put addresses and a relay URL in tickets and codes, as iroh's own tickets do. Move reachability to the first stage that uses two machines. State the truth in PG-1: two nodes connect with no public peer when they share a network, or when one of the participant's or a friend's machines has a public address and serves as relay; otherwise the participant picks a relay or the DHT, and both are named in the consent. Ship no relay and no DHT by default, or put that default to the human.

### RV-04 Cosignatures give no freshness
- **Where.** PEERS 5.5 item 2, 3.1 row 2; net T 5.12; NET-F-37.
- **Severity, category.** Major, error. Verified by reasoning.
- **Finding.**
  1. A witness cosigns when the publisher asks, after each own append (net T 5.12). A channel that is quiet for a month has a month-old cosignature. A host that withholds serves an old head with its old, valid cosignature. The reader cannot tell the two apart. PEERS 5.5 says such a head "is as fresh as their time stamps", which is true and useless.
  2. Only the publisher's primary stores and serves cosignatures (`<home>/net/cosignatures`; "`head` answers for the own channel carry them"). A static host, a replica and a friend's node serve none.
  3. The publisher asks its own friends. A reader "counts a cosignature only from a witness it rates friendly or public". The publisher's friends are unknown to most readers, so their cosignatures count for nothing.
  4. A reader who has a friendly or public node that follows the channel can ask that node for `head` directly. That is mechanism 1, and it needs no signed object.
- **Evidence.** The systems cited re-sign on a timer for this reason: the TUF timestamp role (Samuel et al., CCS 2010) against freeze attacks, and tlog-witness checkpoints requested again while a log is idle.
- **Fix.** Drop the `witness` operation from the core. Freshness evidence is (a) an authenticated `head` answer from a node the reader rates, and (b) for a public node behind a cache, a heads file signed with its node key and re-issued on a timer. Keep C2SP cosignatures as a later option.

### RV-07 Sharding splits nothing
- **Where.** PEERS 7.2, PM-9; net T 5.10, 5.11, ND-5; NET-F-30 to 32; net V success 1.
- **Severity, category.** Major, goal-fit and coherence. Verified.
- **Finding.** net T 5.10: "The keep set is the channels of the primary's holder." So the primary already holds all of it. `holder.release(c, Shard)` removes one reason to keep; the primary still follows the channel. Shards then decide only which replicas hold a second copy.
  - `Scope::OwnMachines` scatters a query over keepers, but the primary answers the whole query locally (W22).
  - PEERS 7.2 wants a set larger than one device ("everything a chosen index lists or a public index offers"). net never builds that set.
  - "Answers from trusted machines are taken as complete for their shards" is unsound when a keeper is behind. No rule compares head vectors first.
  - NET-F-30 demands two keepers "when there are two or more" machines. A primary and a phone of weight 0 give one.
  - A replica learns the keep set by an empty `have` to the primary. With the laptop closed, the set is stale and unsigned.
- **Fix.** Two roles until a keep set outgrows a machine: a full replica holds the whole keep set; a light machine holds what it uses and asks a full one. Rendezvous hashing returns later as a local change (ND-5 already rates the undo as cheap).

### RV-13 Designed ahead against the rule that PEERS cites
- **Where.** spec README rule 3; VISION "Order of work"; PEERS intro; net F intro.
- **Severity, category.** Major, coherence. Verified.
- **Finding.** PEERS: "This file designs ahead of the steps (README rule 3)." Rule 3 says the opposite: "A later step gets only what keeps it open, marked as such, not a guess." VISION: "A question that only a later step needs waits until that step is next." PEERS and `net/*` are about 1 600 lines of byte formats, limits and API for no step.
- **Fix.** The human decides one of: add a step to VISION's order of work and to FUNCTIONAL; or cut PEERS to what keeps the step open (the reserved `<home>/net/`, the reserved kind names, the domain-prefix rule) and mark the rest as a proposal outside the chain.

### RV-14 No parent requirement in `spec/FUNCTIONAL.md`
- **Where.** spec FUNCTIONAL 2, 4; PEERS 12.
- **Severity, category.** Major, coherence. Verified.
- **Finding.** PEERS 12 lists changes to VISION and TECHNICAL, none to FUNCTIONAL. FUNCTIONAL 4 lists "direct transfer between devices, and helper services" as later, and has no requirement, term or journey for nodes. So NET-F-1 to 48 trace to PG and PM ids in a technical document. The chain "vision, then functional, then technical" is skipped. Requirements that the design touches and does not amend:
  - F-QUAL-1: offline "except reading a channel or an index from its web host". A node is not a web host.
  - F-PUB-6: follow "from a link". A link has no node form (RV-12).
  - F-APP-6: after a restore, the own head could come from a trusted machine. Not said.
  - F-AGENT-4: the floor names install, grant and signature. Pairing a machine is a stronger authority and is not named (RV-23).
  - F-FIND-2 and W23: no public node built in. Said in PEERS 12, required nowhere.
- **Fix.** Add terms (node, class), requirements and one journey to FUNCTIONAL before `net/FUNCTIONAL.md` traces to them.

### RV-15 Wishes and voices against V7
- **Where.** VISION V7; PEERS intro, 5.6, 5.7, 6.2, 10.1, 11; net T 4.4 (`kinds`, `refers_to`); W15.
- **Severity, category.** Major, coherence. Verified.
- **Finding.** V7: watane "keeps the way open for mods that exchange signed statements between people. watane itself does not carry these statements." PEERS treats `wish` and `voice` as statement kinds in channels, replicated, indexed by backlinks and counted. Each would be signed by the participant key, so each needs a confirmation in the app (W13). "The open index of the Wishes prototype" names something that is not in this repository (the word occurs only in PEERS).
- **Fix.** Remove the kinds, the backlink index, `refers_to` and the counts. Keep the data plane blind to kinds. Put "may channels carry kinds that mods define" to the human as a vision question.

### RV-22 "Two origins" counts free keys
- **Where.** PEERS 8.1 step 4; NET-F-18; net T 5.5 step 4.
- **Severity, category.** Major, error. Verified.
- **Finding.** net T 5.5: "an origin is a node key, or the host of a base". Node keys are free (PEERS 3), so two unknown keys of one attacker are two origins. Only the clause "one source that is not unknown" resists an eclipse, and it applies "when one is reachable". When none is, the text does not say whether the read fails or is marked. F-FIND-4 marks; it does not fail.
- **Fix.** Drop the origin count. Rule: a head is "checked" when the publisher's own base gave `Current` (today's rule) or a node rated above unknown confirmed the same head. Otherwise it is shown and marked with its sources. A fresh read never fails for lack of diversity.

### RV-12 A channel with no static host cannot be followed
- **Where.** dist 4.2, 4.1 (`list`, `include`); PEERS 12 last row, 14 item 4.
- **Severity, category.** Major, completeness. Verified.
- **Finding.** A link is `watane:` key `@` base, and the base is an absolute http(s) URL. `list` and `include` bodies hold a link. So every channel still needs a static host before anyone can follow or list it, and `peer` statements are found only after a first read from that base. PEERS 12 leaves the node form of a link "not decided". VISION's later item is "direct transfer between devices, without a static host".
- **Fix.** Decide the node form of a link before any stage that serves strangers. It is a published format, so it is expensive to change later.

### RV-10 The public node is not simple
- **Where.** PEERS 10.1, PG-2, PM-11; net T 5.4, 5.14, ND-12; NET-F-43, NET-F-44.
- **Severity, category.** Major, goal-fit. Verified.
- **Finding.** PG-2 promises "a plain HTTP service that is easy to run reliably". PM-11 delivers "the same program": an iroh endpoint, a tokio runtime, `dist`, `log`'s SQLite store, a crawler, an indexer, a witness, a subscription server and an HTTPS front.
  1. **No admission rule.** "A public node's holder reads every channel it learns of" and "`offer` from any class is accepted for channels the holder wants". Keys are free. A million valid channels cost an attacker nothing.
  2. **No deletion.** `log` 4.8 refuses DELETE by trigger. A public operator cannot remove spam or answer a legal demand.
  3. **Search does not scale.** dist 5.10 derives every view on each call and DD-23 rates it "good enough for a few thousand mods". net T 5.4 serves it to any caller at up to 400 requests per second.
  4. **No TLS story.** `Options::https` is a socket address. Certificates are not mentioned.
  5. **No owner for the crawl.** PEERS 9 lists a crawl process. net T 5.14: "`net` adds no crawler of its own." `dist` has no timer. Nothing drives it.
  6. dist's caps (512 listed channels, DD-27) contradict "everything its crawler finds".
- **Fix.** Define the public peer by role, and make the base role static: a job writes site folders and a signed heads file, and any web server or CDN serves them. Search and relay are separate, optional services.

### RV-27 Operation lists disagree
- **Where.** PEERS 4.3, 6.1; net T 4.2, Q3. Minor, coherence. Verified.
- **Finding.** PEERS 6.1: a node "says which [operations] in its node record". PEERS 4.3: "its services come from the `hello` operation". PEERS 6.1 has no `hello`, `announce`, `devices` or `pair`; net T 4.2 has them. net T Q3 says "PEERS 6.1 lists `assign`"; it does not. PEERS `subscribe` takes a filter; net takes channel keys. PEERS `have` takes reconciliation ranges; net does not.
- **Fix.** One operation table, in `net/TECHNICAL.md`. PEERS names the needs, not the operations.

### RV-28 A grant signed by the participant key
- **Where.** PEERS 3 (class table, trusted row) against 4.3, 4.4, PM-3. Minor, coherence. Verified.
- **Finding.** Trusted machines are "listed in a grant signed by the participant key". 4.3: "The participant key signs channel entries and nothing else." 4.4: the issuer is "the node that holds the participant key".
- **Fix.** "Listed in the device list that the primary's node key signs."

### RV-29 Signed in four places, unsigned in the contract
- **Where.** PEERS 4.3, 4.4, 6.1; net V boundaries; net F 1; net T Q2, ND-8. Minor, coherence. Verified.
- **Finding.** PEERS signs grants, `keep` receipts and query answers. net V: the node key signs "device lists, grants, cosignatures, query answers". net F: a grant is "a signed permission". net T signs only cosignatures and device lists. PEERS 4.4 opens with the net decision and then describes the signed form.
- **Fix.** Remove the signed forms from PEERS, net V and net F until delegation exists.

### RV-33 "It never signs"
- **Where.** PEERS 12 (subsystems row); spec T 6 (confirmation floor). Minor, coherence. Verified.
- **Finding.** "`watane-node` ... has no confirmation, so it never signs." A public or friendly `watane-node` cosigns and signs device lists with its node key. The floor in spec T 6 says "each ... signature ... needs a confirmation in the app".
- **Fix.** Say "never signs with a participant key". Limit the floor's "signature" to statements, in F-AGENT-4 and spec T 6.

### RV-34 Bao on a static host
- **Where.** PEERS 4.2, 6.1; dist 4.3. Minor, error. Verified.
- **Finding.** PEERS 6.1 gives `blob` an output of "bytes with Bao proof" and lets a static host offer it. A site folder holds `packages/<id>` and no outboard file (dist 4.3: "No other file"), so a range from a static host cannot be verified alone. The 16 KiB group is the encoding of the `bao-tree` crate that iroh-blobs uses; the original Bao verifies 1 KiB chunks and has a different encoding.
- **Fix.** Say that a static host serves whole packages, checked by BLAKE3 at the end. Name `bao-tree` where ranges are meant.

### RV-36 QUIC addresses escape the local-probe rule
- **Where.** PEERS R5; NET-F-45; net T 5.15; net V (V5 row). Minor, error. Suspected.
- **Finding.** R5 forbids any request to a local address "because a request or a listing named it". net narrows this to HTTP bases and allows "direct QUIC addresses on the local network" because they "come from discovery". A stranger's `peer` statement names a node key; the stranger's DHT record for that key can name `192.168.1.1:443` or a victim's address. Every follower then sends QUIC handshakes there. net V claims "a node never probes the participant's own network because of a stranger's request".
- **Fix.** For a node that is not trusted, not friendly and not found by mDNS, dial only global addresses and the relay.

### RV-39 `peer` has no statement rules
- **Where.** PEERS 4.1; dist 4.1 statement rules. Minor, completeness. Verified.
- **Finding.** dist 4.1 gives 14 rules for the six kinds. PEERS gives none for `peer` and `unpeer`: a duplicate node, an `unpeer` of something else, the number of live peers a reader will dial, a node that never agreed to serve, a compromised channel. dist caps listed bases at 4.
- **Fix.** Rules in dist 4.1: at most 4 live `peer` statements count, in seq order; a duplicate is a `BadStatement`; a compromised channel gives none.

### RV-42 A SQLite file from an untrusted node
- **Where.** PEERS 5.7 last paragraph. Minor, error. Suspected.
- **Finding.** A reader opens an index snapshot "signed by its node key", which can be an unknown or public key. A crafted database file is a known way to attack SQLite; the SQLite project documents the hardening needed.
- **Fix.** Do not open foreign database files. A snapshot is a list of hits in the wire form, or the feature waits.

### RV-44 The DHT is safe for integrity only
- **Where.** PEERS 5.1, 11. Minor, error. Suspected.
- **Finding.** "The signature and the sequence number make their answers safe to use." True for forgery. An attacker with nodes near a target id can withhold a record or serve an older valid one, so a node becomes unreachable or is dialed at an old address. BEP 44 items also expire and need a republish about once an hour.
- **Fix.** Say so in PEERS 11. Never make the DHT the only path to an own or friendly node: cache last good addresses and exchange them over existing connections.

### RV-46, RV-49, RV-50, RV-52, RV-53, RV-54, RV-55 (minor and nits)
| Id | Where | Finding | Fix |
|---|---|---|---|
| RV-46 | PEERS 3; net T 5.4 | "Public" is a class of node the participant relies on, and in "a public node serves them as `public` client" it is a kind of caller. The serving table has a Public column for callers that are public nodes. | Call the caller column "public node" and drop "public client". |
| RV-49 | PEERS 5.2, 5.3, PM-6 | SSB's EBT is Plumtree applied to feeds with a vector of seqs. PM-6 lists "seq vectors (SSB EBT)" and "Plumtree" as two things. | One sentence that says so. |
| RV-50 | PEERS 5.1 | "In this order, all of them in the core" and item 4 is a public index. | Mark item 4 as outside the core. |
| RV-52 | PEERS 5.5, 15 | "The witness model of Certificate Transparency (RFC 9162)": CT has logs, monitors and auditors, not cosigning witnesses. Those are CoSi, Sigsum and tlog-witness. CONIKS is in the sources and nowhere in the text. | Cite CT for monitors only; drop CONIKS. |
| RV-53 | PEERS 8.1 step 5 | The power of two choices balances load with a load signal. Here it picks by latency rank, which sends most reads to one node. | Say "pick two at random, take the faster". |
| RV-54 | PEERS 12 | "'One identity on two devices' get[s] the answer of ... 7.4." 7.4 is "kept open". The answer is 7.1: one signer. | Cite 7.1. |
| RV-55 | PEERS 6.3; dist DD-12 | "A CDN may cache them for ever." The built-in site deletes the package of a withdrawn release. | "Entries for ever; packages for a bounded time." |

## 4. Findings on `net/VISION.md`

RV-01, RV-02, RV-07 and RV-29 apply here. In addition:

- **RV-24 (major, completeness, verified). Default state.** "Never a requirement" says `net` can be off. No document says whether it starts on. A node that is on publishes addresses and dials a relay at the first start. F-APP-2 creates the identity "with no contact to a server"; V5 is "safe by default"; W23 ships no default endpoint. **Fix:** a requirement: `net` is off until the participant turns it on in the app, and each of serving strangers, the DHT and a relay is its own choice.
- The V5 row claims no probe of the own network (RV-36). The V1 row claims shared search (RV-07). The success line "a new head reaches the followers who are online within seconds" holds only for followers with a path to a node of the publisher (RV-03).

## 5. Findings on `net/FUNCTIONAL.md`

### RV-19 Twelve requirements have no scenario
- **Severity, category.** Major, completeness. Verified by counting section 3 against section 2.
- **Finding.** "Each scenario is the test of the requirements it names." No scenario names NET-F-5 (`hello`), 6 (default unknown), 8 (public rating), 9 (trusted from the device list), 10 (never publish a class), 15 (fair sharing), 16 (size limits), 21 (order by measurement), 24 (compare heads), 33 (catch-up), 40 (DHT) or 48 (never count nodes). Stage N4 is "done" by S7, which does not test `have`; N7 by S17, which does not test the DHT.
- **Fix.** One scenario for each, or delete the requirement.

### RV-20 Design with no requirement, requirement with no design
- **Severity, category.** Major, completeness. Verified.
- **Finding.**

| Design in net T | Requirement |
|---|---|
| `query` with `Scope::Friends` and `Scope::Public` (5.11) | none: NET-F-32 covers own machines only |
| `locate` (5.11) | named once, in the policy of NET-F-13 |
| serving `subscribe` (5.7) | none |
| asking for cosignatures after each append, storing and serving them (5.12) | none: NET-F-37 is the witness side only |
| hedging (5.5) | none |
| weights, `set_weight` (4.3, 5.10) | none; contradicts NET-F-30 |
| `revoke`, `remove_friend`, `remove_public` | none |
| HTTPS `subscribe` as server-sent events (5.14) | NET-F-43, with no scenario |

| Requirement | Design |
|---|---|
| NET-F-15 fair sharing | token buckets only; PEERS 6.4 names a fair queue |
| NET-F-44 a public node reads every channel | handed to the holder; `dist` has no design for it |
| NET-F-16 refuse before the work when "its answer would be" too large | no rule for an answer whose size is unknown before the work |

### RV-21 Grant and class policy contradict
- **Severity, category.** Major, coherence. Verified.
- **Finding.** NET-F-13: friendly callers get "what their grant names"; unknown callers get reads, `locate`, announcements and `query`. Read as written, a friend with no grant gets less than a stranger. net T 5.4 gives friendly callers `query`, `witness` and `have` with no grant. `enum Op { Offer, Keep, Query, Witness, Have }` has three members that 5.4 never checks. 5.11 says "every friend's node that the friend's grant lets us query", while 5.4 lets anyone query.
- **Fix.** One rule: a class gives a floor; a grant adds to it. Remove unused `Op` members.

### RV-23 No confirmation rule for the strongest authorities
- **Severity, category.** Major, completeness. Verified.
- **Finding.** `Net::pair_code`, `add_friend`, `grant`, `add_public` and `set_relays` are plain calls. A paired machine gets "everything; no quota". spec T 5 lets any local program call the local API and relies on the confirmation floor; F-AGENT-4 names install, grant (of a mod) and signature. Nothing says these `net` calls have no route on the local API, and no requirement says the app confirms them. Pairing has no step on the primary that shows the joining node's key: someone who photographs the code within 10 minutes pairs first.
- **Fix.** A requirement and a row in spec T 6: each class change, pairing and grant needs a confirmation in the app, has no route on the local API, and pairing shows the short key of the joiner before the list is signed.

### RV-51 (nit) Citation rule and traces
The header says PEERS ids are "cited in `Traces:` lines only"; NET-F-17 cites "`spec/PEERS.md` 8.1" in its text. NET-F-45 traces to PM-10, which does not hold R5. NET-F-48 traces to PM-8, which is about query answers.

## 6. Findings on `net/TECHNICAL.md`

### RV-05 An unsolicited `announce` makes the node work for a stranger
- **Where.** 4.2 (`announce` op), 5.4 (row 1), 5.7. Major, error. Verified.
- **Finding.** 5.4 lets an unknown caller send `announce`. 5.7 describes announcements only on `subscribe` streams that the node itself opened; the `announce` request has no behaviour text. If it is handled as 5.7 says, then: "on an announcement above the holder's head, `net` reads the missing entries from that node. A failed read falls back to the next source." So a stranger sends `[channel, 2^53-1, id]` for a channel the node follows, serves nothing, and the node asks the channel's other sources. At 20 requests per second per free key, 400 per second for the class, the node becomes a reflector against static hosts and friends. A timeout is `Failed`, not `BadObject`, so the stranger is not dropped. 5.7 also does not say that an announcement for a channel the holder does not want is ignored.
- **Fix.** Accept announcements only on streams this node opened. Remove the `announce` request. A failed fetch after an announcement does not fall back; the next timer sync covers it. Ignore channels the holder does not hold.

### RV-06 The holder must hedge, and must not
- **Where.** 5.5 last paragraph, 1 (Threads), 8, Q4, ND-2; dist 1, dist 3, dist AGENTS rule 7. Major, coherence. Verified; the runtime panic is S.
- **Finding.**
  1. 5.5: "The holder reads in that order and hedges ... with at most 3 in flight." The holder is `dist`, which has "no thread, no timer, no async runtime". `Net::head` blocks on one node. `dist` cannot have three reads in flight.
  2. `net` calls `holder.ingest` from its runtime. The holder then calls `Net::announce` (11: "`announce` after ... a head moved"). net T 1: "A caller must not call it from inside a tokio runtime." dist 3: "never from inside an async task." Unless every `Holdings` call runs on a blocking thread and `announce` never blocks, this panics or deadlocks.
  3. Q4 admits that `dist` reads under its own lock, so a refresh blocks every node that `net` serves.
  4. 8 has `app` hand `Net` to `Dist`, so `dist` depends on `net` and links iroh and tokio. spec T 2 lists `dist`'s dependencies as `log`, `pkg`.
- **Fix.** `dist` never calls `net`. `net` reads from nodes on its own runtime and hands entries to the holder; every source only raises the stored head, so no coordination is needed. Hedging lives in `net`. `Holdings` methods run on blocking threads, and none calls back into `net`: the holder returns "the head moved" and `net` announces.

### RV-08 Every home has an identity
- **Where.** 5.9 (definition of the primary), 8; dist 3 (`Dist::open`), DIST-F-1; log 2 (`Log::open`). Major, coherence. Verified.
- **Finding.** `Log::open` "creates the identity at the first open". `Dist::open` always opens `Log` and takes a `Confirm`. So a replica, a friend's box and a public node each hold a participant signing key they must never use, and need a `Confirm` that 8 says they do not have. 5.9 defines the primary as "the home whose holder has a participant key and whose device list, if any, names it": true for every fresh replica. `hello` on a replica answers the replica's own unused participant key, not the participant it serves. On a second laptop the app can publish under that other key, and W20's "the own channel always counts" as an index points at an empty channel.
- **Fix.** A way to open a home with no identity (`log` and `dist` stages), listed in section 11. The primary is the node that the device list names as primary, nothing else.

### RV-09 Remote hits remove the read-set bound
- **Where.** 5.11 second paragraph; dist 5.9 (caps), DD-27; W21; F-FIND-4. Major, error. Verified.
- **Finding.** "One it does not hold is read from the sources of its channel before it is shown." An answer has up to 200 hits, in channels the answering node picks. Each needs the whole chain from seq 0 (log 5.2), up to 1 000 entry files per read. W21: "a home reads a bounded number of channels because of listings". Nothing bounds the reads because of hits. The holder often knows no source for such a channel, so the answerer supplies it: a reflection path to any host.
- **Fix.** A hit is verified by reading its channel from the node that gave the hit, with no record (as dist 5.11 does), at most N channels per query (proposal: 32), each within the limits of a listed read. More hits are shown as unverified counts or dropped.

### RV-16 Forwarding without a clamp, an id or a rule for who may ask
- **Where.** 4.2 (request form, budget), 5.11. Major, error. Verified.
- **Finding.** The request carries `[deadline_ms, hops, fanout]`. The defaults are "hops 2, fanout 4", but no rule says a callee lowers a larger value. A caller sends hops 255. Requests have no id, so in a friend graph A to B to A, A answers its own query again and B is asked twice. 5.11 does not say for which callers a node forwards. If it forwards for unknown callers, a stranger spends the node's friendly standing at its friends (PEERS R2 notes the responsibility, not the limit) and each stranger request becomes up to 20.
- **Fix.** The callee clamps each budget field to its own limit. A request has a random id; a node drops an id it has seen. A node forwards only for trusted callers, and at most one hop, until a requirement asks for more.

### RV-17 Limits count requests, not bytes or connections
- **Where.** 4.2 limits table. Major, error. Verified by arithmetic.
- **Finding.** An unknown key may send 20 requests per second. One `blob` answer is up to 64 MiB. One `entries` answer is up to 1 000 × 65 728 bytes = 62.7 MiB. So one free key may demand about 1.3 GiB per second, and the class 25 GiB per second. "Open streams per caller: 8" has no total: 10 000 free keys hold 80 000 streams and 10 000 connections. A `subscribe` stream stays open by design.
- **Fix.** Byte budgets per caller and per class per time; a cap on connections and on long-lived streams per class; a byte cap per `entries` answer (proposal: 4 MiB).

### RV-18 Nothing can free space
- **Where.** 5.9 ("drops its shards"), 5.10 (`release`), 5.13 (quota, "`net` calls `holder.release`"); log 4.8, 5.3; dist Q12. Major, coherence. Verified.
- **Finding.** log 5.3: "Nothing deletes an entry or a proof", enforced by triggers. dist Q12 defers `Log::forget`. So a released shard stays on disk, and a friend's quota of "the holder's bytes for the friend" can only count up. A friend who rotates channels fills the disk under any quota.
- **Fix.** State that a quota counts bytes ever accepted until `Log::forget` exists, or schedule `Log::forget` before the friends stage.

### RV-25 Lifecycle gaps
- **Where.** 5.9, 4.2, 4.5, 5.3. Major, completeness. Verified.

| Case | What the text says | Gap |
|---|---|---|
| The primary is lost | "restores the identity on a new home ... and pairs again" | Each replica has a `primary` file. Whether `join` may replace it, and who may ask, is not said. |
| A machine is removed while another is offline | `hello` gives `devices_seq` on the next connection | The device list has no expiry. An offline machine trusts the removed one until it meets a newer list. |
| A node key is lost or stolen | nothing | No rotation. A friend's node changes only through a new `peer` statement. |
| A friend's channel is compromised | nothing | Whether its `peer` nodes stay friendly is not said. dist ignores the listings of a compromised index. |
| A newer protocol | `unknown_op` for a new op | Decoders refuse "input that does not encode back to itself" and arrays are positional. Whether a longer answer array is an error is not said. `hello` has no version. |
| A newer file format | `peers.json` has a version | No rule for `devices`, `cosignatures`, or a downgrade. |
| Restart | subscriptions and measurements in memory (ND-9) | Who calls `subscribe` again is left to the app. |

- **Fix.** One section "Lifecycle" with a row for each case. A change to an operation is a new operation name, as dist 4.1 rules for kinds.

### RV-26 `have` does not fit in a request
- **Where.** 4.2 limits, 5.8. Minor, error. Verified by arithmetic.
- **Finding.** 5.8: "A vector of 4096 channels is about 170 KiB, which is fine." One pair is 1 + 34 + 1 to 9 bytes, so 4 096 pairs are 147 to 180 KiB. The request size limit is 64 KiB, which holds about 1 500 pairs.
- **Fix.** Raise the limit for `have`, or cap `have` at 1 024 channels and page.

### RV-30 The node key signs more than two forms
- **Where.** 6 ("No signing oracle"); AGENTS rule 5; PEERS 2. Minor, error. Verified.
- **Finding.** "The node key signs only the two forms of 4.3." ND-3: "the node key is iroh's endpoint key." iroh signs TLS handshakes and its discovery records with that key. PEERS 2 says so. The domain prefixes still keep the forms apart, so no attack follows, but the property as written is false.
- **Fix.** "`net` signs only the two forms; iroh signs its handshakes and records. No two of these share a first byte sequence", with the argument.

### RV-35 iroh facts
- **Where.** 2, PEERS 9. Minor, error. Verified on crates.io; interoperation is S.
- **Finding.**
  - The versions in section 2 exist (iroh 1.4.1, tokio 1.54.1, ed25519-dalek 3.0.0, minicbor 2.3.1, mainline 8.0.1). `log` uses the same ed25519-dalek and minicbor.
  - iroh 1.4.1 has no discovery feature. Mainline discovery is `iroh-mainline-endpoint-discovery` 0.1.6: created 2026-09-30, 357 downloads. It depends on `n0-mainline`, not on `mainline` or `pkarr`. The dev-dependency `mainline = "8.0"` and "its Testnet" may not be the DHT that crate speaks (S). "As a pkarr record" (PEERS 4.3) is not confirmed for it (S).
  - Local discovery is `iroh-mdns-address-lookup` 0.6.1. Section 2 does not name it.
  - iroh 1.0.0 is from 2026-06-15; iroh-gossip (0.102) and iroh-blobs (0.105) are below 1.0. "Well-established" holds for the ideas, less for these crates.
  - iroh's default features include `portmapper`, which asks the home router to open a port, and it depends on `iroh-dns`, a lookup through its maker's servers. Section 5.1 does not say that both are off.
- **Fix.** Name the crates and the features that are off. Treat DHT discovery as optional and replaceable.

### RV-37 A fork does not travel
- **Where.** 5.7, 4.2. Minor, error. Verified.
- **Finding.** "Each node sends one announcement per channel and seq." A second entry at the same seq is the fork, and it is suppressed. `head` and `entries` answer the stored chain and say nothing of a fork. Only `witness` returns one. PEERS 4.3 defines a fork proof and no operation moves it.
- **Fix.** Dedupe by (channel, seq, id). `head` answers the fork proof when the holder has one.

### RV-38 Selection details
- **Where.** 5.5. Minor, error. Verified.
- **Finding.** The score is a moving average (α 0.2). Hedging waits for "its 95th percentile latency", which a mean cannot give. `Fresh` takes "the first source of each class and of the channel's own link base, then fill ... to `k = 3`": the first clause can already give five.
- **Fix.** Keep a mean and a mean deviation, as TCP's retransmission timer does (RFC 6298), and hedge at mean + 4 × deviation. State the pick as an ordered list.

### RV-40 Packages of withdrawn releases
- **Where.** 3.1 (`Holdings::blob`), 5.6; dist DD-12. Minor, completeness. Verified.
- **Finding.** `blob` serves by hash with no release in sight. dist removes the package of a withdrawn release from the built-in site. A node keeps serving it from its cache.
- **Fix.** The holder answers `blob` only for a package that a live release in a held channel names.

### RV-41 The HTTPS front
- **Where.** 5.14. Minor, error. Verified.
- **Finding.** `GET /query?f=<base64url filter>`: a filter may be 64 KiB; servers and caches cut URLs near 8 KiB, and the search text lands in access logs. `POST /witness` has a body `[channel, head]`; when the node holds a lower seq, 5.12 says "read the missing entries from the caller", and an HTTP caller is no node.
- **Fix.** `POST /query` with the CBOR filter, or a filter limit of 2 KiB. Drop `/witness` (RV-04).

### RV-43 Small gaps in the API
- **Where.** 3, 4.5, 5.1, 5.7. Minor, completeness. Verified.
- **Finding.** Relays are in `Options::relays` and in `peers.json`; 5.1 uses the file. `events()` returns a `std::sync::mpsc::Receiver`: whether a second call takes the events of the first, and what happens when nobody reads, is not said. `subscribe` keeps "one open stream ... (at most 3 per channel)".
- **Fix.** One place for relays. One receiver per `Net`, bounded, oldest dropped.

### RV-45 A witness with a lost store
- **Where.** 5.12. Minor, error. Verified by reasoning.
- **Finding.** "The holder's head only moves forward, so a witness never cosigns two heads that conflict." After a reinstall or a restore from backup, the holder is empty or older, and the witness can cosign the other branch. tlog-witness requires durable witness state for this reason.
- **Fix.** Moot if RV-04 is taken. Otherwise say it in "Not protected".

### RV-11 `watane-node` inside `app`
- **Where.** spec T 2 (table); net T 8, 11; PEERS 12. Major, coherence. Verified.
- **Finding.** `app/Cargo.toml` depends on `tauri = "=2.12.5"` without `optional`. A binary in that crate compiles Tauri and, on Linux, WebKitGTK, to run a headless server. TC-5 says "the host also runs with no app"; a home server should too. The table gives `net` the program `watane-node (an app binary)` and leaves `app` at "Depends on: `host`, `dist`" and `dist` at "`log`, `pkg`", while net T 10 gives types to both.
- **Fix.** A small crate of its own for the program and the glue, or a binary of `net` if `net` may depend on `dist`. Update every "Depends on" cell.

## 7. Findings on `net/AGENTS.md`

### RV-47 Missing rules, and PEERS's place in the chain
- **Severity, category.** Minor, completeness. Verified.
- **Finding.**
  - No rule says that a class change, a pairing or a grant is never reachable without a confirmation in the app (RV-23).
  - No rule says "no built-in relay, bootstrap or public node" (W23, RV-24).
  - No rule says that `Holdings` is called only from blocking threads (RV-06).
  - Rule 5 repeats the false property of RV-30.
  - "Priority: `../spec/` vision, functional, technical, PEERS" gives PEERS a place in the chain. spec README rule 1 does not know PEERS. The place is defined only here.
  - Rule 8 ("build only when a step does") is right, and it is the only place that stops a builder. The same sentence belongs in `net/TECHNICAL.md` 9, where it is, and in the spec README.
- **Fix.** Add the three rules; correct rule 5; put PEERS's place into spec README rule 1.

## 8. Findings on the small edits

### RV-32 The `spec/TECHNICAL.md` edits are partial
- **Severity, category.** Minor, coherence. Verified against the diff.
- **Finding.** The diff adds one table row and one sentence to W13. Not changed:
  - W1 still says "Six subsystems".
  - The diagram under the table has no `net`. net T Q5 says "added to the table and the diagram".
  - The row's last cell holds "later; `dist` will depend on it", in the Step column.
  - Section 5: "It reserves `log/` for `log`, `dist/` for `dist` and `app/` for `app`", and no flow row for `net`.
  - Section 6 has no row for the node boundary; section 8 no build row; section 9 says "Nothing waits now".
  - W10, W11, W22 and W23 are discussed in PEERS 12 and unchanged, which is right for "designed ahead", but nothing in spec T points there.
- **Fix.** Either make the edits complete and mark each "designed ahead", or take the row out and keep one pointer to PEERS.

### RV-48 `host/TECHNICAL.md`
- **Severity, category.** Minor, coherence. Verified.
- **Finding.** The edit reserves `net/` and changes "three" to "four". That is correct and it is the kind of edit rule 3 allows. The table of what `host` provides (section 16 of that file) reserves `<home>/app/`, `<home>/dist/` and `<home>/log/` and has no row for `net`. spec T 5 still says three (RV-32).

### RV-31 `log` is not told
- **Where.** log 4.5, log V success, log 10. Minor, coherence. Verified.
- **Finding.** log 4.5: "This is the only signature check in watane." net T 4.3 adds two more, with its own dependency on ed25519-dalek. log V: "no other code in watane parses or verifies an entry"; `net` uses `Entry::decode`, which is consistent, but log 10 lists `Entry::decode` for `host` and `dist` only and `PublicKey` for five subsystems without `net`. PEERS 12 says "`log`: no change".
- **Fix.** `log` exports the strict check for a key, a prefix and a message (verification is no oracle), and its table gains `net`. Or log 4.5 says "the only check of an entry".

### `spec/README.md`
The two added passages are accurate. "Designed ahead, for no step yet" is honest. The conflict with rule 3 of the same file is RV-13.

## 9. Does the design meet the stated goal?

| Goal | Verdict | Why |
|---|---|---|
| The core works with only unknown, friendly and trusted peers | Not shown | Two machines in two homes need a relay or the DHT (RV-03). A channel still needs a static host to be found (RV-12). Friendly service needs public statements (RV-02). |
| Public peers are optional | Yes, for correctness | Every answer is verified or a hint. Losing a public node loses speed and reach. |
| Public peers are simple and reliable | No | RV-10, RV-11. |
| Composes established work | Mostly | The ideas are established and cited correctly. The witness use is wrong (RV-04). The sharding borrows Dynamo for two to four personal machines (RV-07). Some crates are young (RV-35). |
| Every mechanism acknowledges the four classes | Partly | Storage, queries and verification do. Subscriptions treat all classes alike except for a channel count. Reads to unknown callers are too open (RV-01). |

## 10. Checked and found correct

- The papers and venues in PEERS 15 for Kademlia, SSB (ACM ICN 2019), HyParView (DSN 2007), Plumtree (SRDS 2007), range-based set reconciliation (SRDS 2023), CoSi (IEEE S&P 2016), Macaroons (NDSS 2014), the Sybil attack (IPTPS 2002), SybilGuard (SIGCOMM 2006), eclipse attacks (USENIX Security 2015), EigenTrust (WWW 2003), "The Tail at Scale" (CACM 2013), the power of two choices (IEEE TPDS 2001), rendezvous hashing (IEEE/ACM ToN 1998), Dynamo (SOSP 2007), CRDTs (SSS 2011), the end-to-end argument (ACM TOCS 1984), hole punching (USENIX ATC 2005), OneSwarm (SIGCOMM 2010), and RFC 8742, 8949, 9000, 9180 and 9420.
- A BLAKE3 Bao root equals the plain BLAKE3 hash, so package ids work unchanged.
- The node key text form (52 characters of z-base-32) matches `log` 4.1.
- The domain prefixes `watane/cosign/1` and `watane/devices/1` differ from `watane/entry/1`.
- `<base>c/<key>/` in the layout of dist 4.3 is read by today's `dist` with no change (ND-12).
- The rule of claims (PEERS 3.1), the end-to-end rule R1, and "the class never changes the objects" are right and worth keeping.
- `net` uses `log` for types and `Entry::decode` only and never opens `<home>/log/`. That agrees with "`log` is opened only by `dist`".
