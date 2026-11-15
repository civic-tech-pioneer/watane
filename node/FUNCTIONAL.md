# node: functional specification

What `node` does, free of implementation. "node" in a requirement is the
subsystem. "The participant" is the person who owns the home. "The
holder" is `dist` of this home. A "replica" is a home that joined the
participant's own machines and holds no identity (spec W33). Ids of
`spec/` (F-..., W...) and of `net/FUNCTIONAL.md` (NET-F-n) are cited in
`Traces:` lines only.

All of this is step 8 (own machines, stage D1 of `spec/PEERS.md`).

## 1. Terms of this subsystem

| Term | Meaning |
|---|---|
| node home | A home that `watane-node` runs: `log/`, `dist/` and `net/`, no mods, no identity. |
| app home | The home that the app runs. It holds an identity, unless it joined as a replica. |
| role | What the device list says about this machine: anchor (it holds the identity), full or light. |
| kept channel | A channel that this home stores because an own machine holds it (a full machine's copy), not because the participant followed or chose it here. |
| hint | An answer of another machine that this home has not verified: a head, a hit. |

## 2. Requirements

### 2.1 Open

- **NODE-F-1** When node opens an app home that is not a replica, node shall open its holder with the identity, as today. When it opens a replica or a node home, node shall open the holder with no identity, and create none.
  Traces: F-OWN-3, W33.
- **NODE-F-2** node shall open `net` with the holder of the same home, and shall start no network contact that `net`'s own switches do not allow.
  Traces: F-OWN-1, NET-F-3.
- **NODE-F-3** With `net` off, every function of the holder shall work as it does without node.
  Traces: F-QUAL-1, NET-F-31.

### 2.2 Holder

- **NODE-F-4** node shall answer `net`'s questions about what this home holds only from what the holder stored and verified.
  Traces: NET-F-14, NET-F-15.
- **NODE-F-5** node shall hand everything that `net` pulled to the holder, which verifies it again before it stores it; node shall store nothing itself.
  Traces: PG-3, NET-F-23.
- **NODE-F-6** A full replica shall keep each channel that it pulled from an own machine as a kept channel, with no follow and no index choice, and keep it current from the own machines. The anchor shall hold only what the participant's own acts on it chose; what it pulls besides is stored and not kept.
  Traces: F-OWN-3, F-OWN-4, NET-F-21.
- **NODE-F-7** No question of `net` shall wait for a read of the holder from a web host.
  Traces: F-OWN-4.

### 2.3 Telling and asking

- **NODE-F-8** When an own statement is signed, or a read from a web host raised the head of a channel, node shall tell `net`, so that the own machines learn of it within seconds.
  Traces: F-OWN-4, NET-F-19.
- **NODE-F-9** On a light machine, before the holder reads a channel or fetches a package that it lacks, node shall ask `net` to pull it from a full machine. If no full machine answers, the holder shall read from the link location as before.
  Traces: F-OWN-5, NET-F-26.
- **NODE-F-10** On a light machine, a search shall give the hits of this home and of a full machine, each hit verified by the holder, and each remote hit with the machine that gave it.
  Traces: F-OWN-5, F-FIND-4, NET-F-25.
- **NODE-F-11** A light machine shall keep current, through the full machines, each mod that it installed from a remote hit with no link location.
  Traces: F-OWN-4, F-OWN-5.

### 2.4 Pairing and restore

- **NODE-F-12** An app home whose identity signed no statement shall be able to join the participant's machines. node shall delete that unused identity before it joins, and refuse the join when the identity signed a statement.
  Traces: F-OWN-3, W33.
- **NODE-F-13** After a restore, node shall give the heads of the own channel that the full machines report, each with its machine, as hints; the holder shall confirm the own head only when the participant picks one and the holder stored and verified exactly that head.
  Traces: F-APP-6, W18, NET-F-30.

### 2.5 The program `watane-node`

- **NODE-F-14** `watane-node` shall run a node home with no window and no identity, as the full or light machine that the device list names.
  Traces: F-OWN-3, J8.
- **NODE-F-15** `watane-node join` shall join the participant's machines with a pairing code, print this machine's short key so that the participant can compare it, and wait for the participant's answer on the anchor.
  Traces: F-OWN-2, NET-F-7.
- **NODE-F-16** With a flag, `watane-node` shall also run a relay for the participant's own machines.
  Traces: F-OWN-7, NET-F-29.
- **NODE-F-17** `watane-node` shall turn on the DHT or add a relay only on the participant's command, after it printed the same consent text as the app.
  Traces: F-OWN-7, F-OWN-8, W32.
- **NODE-F-18** `watane-node` shall print each refusal and each failure with its cause and the machine's short key.
  Traces: F-QUAL-3, NET-F-32.

## 3. Scenarios

Each scenario is one test, named `s<n>_...`. "The laptop", "the server"
and "the second laptop" are three homes on one machine, with `net` bound
to loopback and mDNS off (`net/TECHNICAL.md` 7).

| # | Scenario | Requirements |
|---|---|---|
| S1 | A node home is opened: `<home>/log/` has a store and no `identity.key`; publish gives `NoIdentity`. An app home opened through node has an identity, as before. | 1, 14 |
| S2 | With `net` off, the home of J3's Bob, opened through node, follows Alice from her link and installs her mod, and no UDP socket exists. | 2, 3 |
| S3 | The holder of a home with Alice's channel answers `heads`, `entries` and `fork` with the stored bytes; a package file is given only for a live release it holds. A changed byte of an entry handed to `ingest` is refused and nothing is stored. | 4, 5 |
| S4 | The server is paired as full with the laptop. Alice publishes on the laptop. The server holds the entry as a kept channel, and its package, within 5 s; its `follows.json` record says `kept` and nothing else. A channel that only the server holds, pulled by the laptop, gives the laptop's `follows.json` no record. | 6, 8 |
| S5 | While the laptop's dist refreshes from a web host that answers after 20 s, the server reads `heads` from the laptop within 1 s. | 7 |
| S6 | The second laptop is paired as light. A search for `reading` gives the hit from the server, marked with the server's short key; it installs it, and the package comes from the server. A hit whose channel the server changed is not shown. | 9, 10 |
| S7 | The installed mod of S6 has no link location. Alice publishes 0.2.0 on the laptop; the second laptop's next refresh offers it. | 11 |
| S8 | An app home whose identity signed nothing joins as light: its key file is gone and the device list names it. An app home with one statement is refused, and its key is unchanged. | 12 |
| S9 | Alice restores her identity on a new home and pairs the server with `replace`. The new home shows the server's own head as a hint; publishing fails until Alice picks it; then the next statement has the next seq. A hint with another id is refused. | 13 |
| S10 | `watane-node join <code>` prints the server's short key and waits; after the anchor says yes it prints the device list; after a no it exits 1 with `declined`. | 15 |
| S11 | `watane-node run --relay 127.0.0.1:<p>` runs a relay; two homes with only that relay connect through it. | 16 |
| S12 | `watane-node dht on` with no answer on its standard input changes nothing; `dht on --yes` prints the consent text and turns it on. | 17 |
| S13 | A machine sends a changed byte to the server; `watane-node run` prints one line with the cause and that machine's short key. | 18 |
