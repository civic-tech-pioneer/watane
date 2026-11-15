# net: vision

## Why

Today a channel moves only through a static web host: the maker copies a
folder, a reader polls it. That works with no operator (V4), but a
participant's own machines cannot keep each other current without a host
in between, an always-on machine cannot hold what a laptop reads, and a
host that withholds new statements is caught only by chance.

`net` is the data plane of `spec/PEERS.md`. It connects machines directly,
dialed by key, rates each machine by the class the participant gave it, and
moves the same signed objects that `log` and `dist` already define. Step 8
builds it for the participant's own machines. Friends, strangers and public
peers come in later stages, on the same objects and the same wire.

## Goals served

| Goal | How `net` serves it |
|---|---|
| V4 No center | Machines dial each other by key. Nothing that someone else runs is on by default: no relay, no DHT, no bootstrap host, no public peer. Each is one choice of the participant. |
| V5 Safe by default | `net` is off until turned on. It passes on only objects that verify, pulls and never accepts a push, forwards nothing, and checks a byte limit before any work. It publishes nothing that ties the participant to a machine or an address. |
| V6 Chosen trust | The participant puts each machine in a class. A class comes only from the participant's own acts (pairing, a ticket), never from a public statement or a measurement. |
| V1 Own environment | The participant's own machines keep each other current, and a light machine uses what a full machine holds. |

## Boundaries: what `net` must never become

- **Never a signer of statements.** `net` never opens `log` and never holds
  the participant key. Its node key signs only the forms of
  `TECHNICAL.md` 4.3, each with its own prefix, and what iroh signs with
  it (the handshake, and the DHT record when turned on).
- **Never a store of channels.** Entries and packages live in `dist`.
  `net` asks the holder what it holds and hands it what arrives; the holder
  verifies and stores. iroh-blobs keeps a copy only to move packages.
- **Never a reader of meaning.** `net` knows entries, heads, blobs and query
  filters. It does not know what a release or a listing means.
- **Never a translator.** `net` passes on the original bytes. It does not
  re-sign, re-encode or summarise another key's objects.
- **Never a trust authority.** The class of a machine is the participant's
  choice. Measurements order machines inside a class; they never change a
  class.
- **Never a publisher of the participant.** No public record names the
  participant's machines, unless the participant turned on a function that
  does (the DHT, and later "serve strangers").
- **Never a requirement.** Every function of watane keeps working with
  `net` off, from static hosts and offline.

## Success

- Two machines of one participant pair with one code and a confirmation,
  and keep each other current within seconds, with no static host and no
  endpoint that someone else runs.
- A light machine searches and installs through a full machine, and
  verifies every byte itself.
- A changed byte from any machine is refused, with the machine named.
- A removed machine stops being trusted as soon as the others learn of it.
- After a restore, the own channel's newest statement comes back from a
  full machine.
