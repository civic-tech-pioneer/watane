# net: functional specification

What `net` does, free of implementation. "net" in a requirement is the
subsystem. "The participant" is the person who owns the home. "The
holder" is the code that holds the channels and packages: `dist`, joined
to `net` by `node`. Ids of `spec/PEERS.md` (PG-n, sections) and of
`spec/FUNCTIONAL.md` (F-...) are cited in `Traces:` lines only.

Section 2 is stage D1 of `spec/PEERS.md`: step 8, own machines. Section 4
sketches the later stages; a later step writes their requirements.

## 1. Terms of this subsystem

| Term | Meaning |
|---|---|
| node | One running `net`: in the app's home, or in `watane-node`. |
| node key | The Ed25519 key of a node. It names the node and signs only `net`'s own forms. |
| anchor | The node in the home that holds the participant key. |
| own machine | A node in the device list that the anchor signed. Its class is trusted. |
| full machine | An own machine that keeps everything that any own machine holds. |
| light machine | An own machine that keeps what its own home uses. |
| device list | The anchor's signed list of own machines, each with its role (full or light), and an expiry. |
| pairing code | Text with the anchor's node key, hints of where to dial it, and a one-time secret. |
| hint | Something a node said that is not verified, or cannot be: an address, a new head. |
| class | The participant's rating of a node: trusted or unknown in D1; friendly and public later. |

## 2. Requirements (D1, step 8)

### 2.1 Node

- **NET-F-1** When net opens a home with no node key, net shall create one on the device, with no network contact.
  Traces: PEERS 3.
- **NET-F-2** net shall keep the node key in a file that only the user can read, and shall never give it out.
  Traces: PEERS 12.
- **NET-F-3** net shall make no network contact until the participant turned it on, and shall stop all contact when the participant turns it off.
  Traces: F-OWN-1, W32.
- **NET-F-4** When another node connects, net shall know its node key from the transport, never from a field of a request.
  Traces: PG-4.
- **NET-F-5** net shall never send the participant key, and shall never publish anything that names this node beside the participant.
  Traces: PG-7, F-OWN-8.

### 2.2 Pairing and the device list

- **NET-F-6** When the participant asks the anchor for a pairing code for a role, net shall give a code with the anchor's node key, its current addresses as hints, and a secret, valid for 10 minutes and for one use.
  Traces: F-OWN-2.
- **NET-F-7** When a node presents a valid code, the anchor shall show the node's short key to the participant and wait for a confirmation. On yes, it shall sign a new device list with that node and role, give it to the node, and send it to every own machine. A confirmation with no answer is a no (W19).
  Traces: F-OWN-2, PEERS 6.1.
- **NET-F-8** If a code is wrong, used or expired, or the participant says no, then the anchor shall refuse the join and change nothing.
  Traces: F-OWN-2.
- **NET-F-9** A joining machine shall store the list only when the anchor of the code signed it and it names the joining machine. A machine that already has an anchor shall join another only when the participant asked to replace it.
  Traces: PEERS 6.1.
- **NET-F-10** When the participant removes a machine or changes its role, the anchor shall sign a new list and send it to every own machine.
  Traces: F-OWN-6.
- **NET-F-11** A machine shall accept a newer list from any node, but only when its anchor signed it and its seq is higher.
  Traces: PEERS 4.2.
- **NET-F-12** When the participant sets an expiry of n days, a machine shall stop treating the others as trusted once its newest list is older than n days and it has not reached the anchor since. With 0, the default, the list never expires.
  Traces: F-OWN-6, PEERS 4.2.

### 2.3 Serving

- **NET-F-13** net shall rate a node as trusted exactly when it is in the newest valid device list, and as unknown otherwise.
  Traces: PEERS 5.
- **NET-F-14** net shall serve each caller within its allowance, on every protocol: a trusted caller everything the holder holds, an unknown caller nothing but `hello` and `pair`.
  Traces: PEERS 6.3, W32.
- **NET-F-15** net shall send every entry and package as its original bytes.
  Traces: PG-3.
- **NET-F-16** If a request or its answer is over its byte limit, or the caller is over its byte or connection budget, then net shall refuse it before doing the work, and name the cause.
  Traces: PEERS A6.
- **NET-F-17** net shall never forward a request to another node.
  Traces: PEERS 8.3.

### 2.4 Sync

- **NET-F-18** When an own machine connects, and every 10 minutes, net shall compare heads with it and read what this machine's role needs and the holder lacks.
  Traces: F-OWN-4, PEERS 9.
- **NET-F-19** When the holder has a new head, net shall tell the own machines that are online, so that each reads it within seconds.
  Traces: F-OWN-4.
- **NET-F-20** net shall treat a told head as a hint: it changes nothing until the entries it names arrive and verify.
  Traces: PEERS A2.
- **NET-F-21** A full machine other than the anchor shall keep every channel and every package of a live release that any own machine holds. A light machine, and the anchor, shall keep what its holder asks for. A light machine shall ask a full machine other than the anchor first.
  Traces: F-OWN-3, PEERS 3.
- **NET-F-22** net shall pull only: it shall never take entries or packages that it did not ask for.
  Traces: PEERS 6.2.
- **NET-F-23** net shall pass to the holder only entries that decode and belong to the channel asked for, in seq order, and packages whose BLAKE3 hash and size match.
  Traces: PG-3, V5.
- **NET-F-24** If a node sends an object that does not verify, then net shall drop it for the rest of the session and report the node by its short key.
  Traces: F-QUAL-3.

### 2.5 Light machines

- **NET-F-25** When the holder of a light machine searches, net shall ask a full machine too and give the hits, each with the node that gave it.
  Traces: F-OWN-5.
- **NET-F-26** When the holder of a light machine needs a channel or a package that it lacks, net shall read it from a full machine.
  Traces: F-OWN-5.

### 2.6 Reaching machines

- **NET-F-27** net shall find own machines on the local network, at the addresses it last reached them on, at the addresses other own machines pass on, and at the hints of a code.
  Traces: PEERS 8.1.
- **NET-F-28** net shall use a relay only when the participant chose one, and the Mainline DHT only when the participant turned it on. When no path exists without them, the failure shall say so.
  Traces: F-OWN-7, W32.
- **NET-F-29** `watane-node` shall be able to act as a relay for the participant's own machines.
  Traces: PEERS 8.1.

### 2.7 Restore

- **NET-F-30** After a restore, when the holder asks for the newest own head, net shall ask the full machines and give their answers as hints for the participant to confirm.
  Traces: F-APP-6, PEERS 6.1.

### 2.8 Always

- **NET-F-31** With net off or offline, every function of the holder that works today shall keep working.
  Traces: PG-6, F-QUAL-1.
- **NET-F-32** Every refusal and every failed read shall name its cause and the node.
  Traces: F-QUAL-3.

## 3. Scenarios

Each scenario is the test of the requirements it names.

| # | Scenario | Requirements |
|---|---|---|
| S1 | Alice opens a new home with net off; a node key exists and no packet leaves the machine. A second open keeps the same key. | 1, 2, 3 |
| S2 | Alice's laptop (the anchor) gives a code for a full machine. Her server joins with it; the laptop shows the server's short key; Alice confirms. Both hold a list with both machines. | 6, 7, 9 |
| S3 | A second use of the code, a wrong secret, an expired code, and a "no" are each refused; the list does not change. | 8 |
| S4 | An unknown node connects to the server. Its `hello` answer holds no participant key; its `heads` is refused with the cause. A request that claims another key in a field changes nothing. | 4, 5, 13, 14 |
| S5 | Alice publishes on the laptop. The server has the entry and its package within seconds, with the same bytes. A forged hint changes nothing. | 15, 19, 20, 21 |
| S6 | The server is offline while Alice publishes twice. When it comes back, it catches up. | 18 |
| S7 | A machine sends an entry with a changed byte, an entry of another author, and a package whose hash does not match. Each is refused, the machine is named and dropped for the session. | 23, 24, 32 |
| S8 | Alice pairs a second laptop as light. It searches; the hits come from the server, and each is verified. It installs one; the package comes from the server. | 21, 25, 26 |
| S9 | A node sends entries that nobody asked for; nothing is stored. A request over its byte limit is refused before any work. A light machine's query reaches the server and no third node. | 16, 17, 22 |
| S10 | Alice removes the second laptop. The server stops serving it. With an expiry of 1 day, a machine that has not reached the anchor for 2 days stops trusting the others. | 10, 11, 12 |
| S11 | Two machines on different networks with no relay and no DHT fail to connect, and the failure says a relay or the DHT is needed. With the server's relay chosen, they connect. | 27, 28, 29 |
| S12 | Alice restores her identity on a new laptop, which pairs the server with `replace`. It learns the own head from the server as a hint; nothing is signed until Alice confirms. | 9, 30 |
| S13 | With net off, J3, J4 and J5 pass unchanged. | 31 |

## 4. Later stages (sketch)

A later step writes these requirements. They are listed so that D1 keeps
them open (`spec/PEERS.md` 10).

- **D2 Friends.** A friend's ticket names their anchor; the nodes of the
  friend's device list are friendly. Grants of channels and a byte quota.
  Friend sync, pull only. Friends' heads count as checked. Friends asked
  for queries only when ticked, and never forward.
- **D3 Strangers.** "Serve strangers" serves the own channel only. The node
  resolver for links (W31). Unrated nodes are dialed on global addresses
  only.
- **D4 Public peer.** A mirror job writes site folders and a heads file,
  re-signed on a timer, for any web server. Public peers chosen in the app;
  their heads files count as checked.
- **D5 Scale.** Forgetting, set reconciliation, Bao ranges, public search,
  witness cosignatures: each when a measurement shows the need.
