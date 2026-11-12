# dist: vision

## Why

A mod becomes a trace for other people (V2) only when it leaves the device.
A person can use a stranger's mod safely (V5) only when they can check who
published it and that the bytes are the published bytes. `dist` does both
with plain files on any static web host, with no operator and no server to
trust (V4, W10).

`dist` is everything between this device and other people: it signs the
statements of the participant, writes the own channel as a folder of files,
reads the channels of others from links, verifies every byte, and hands a
verified package to the caller.

## Goals served

| Goal | How `dist` serves it |
|---|---|
| V2 Stigmergy | Publish a package as a release; follow a channel from a link; fetch its packages. Step 4: find releases through indexes. |
| V4 No center | The channel and the packages are plain files. Any static host serves them. A link carries the key, so the host decides nothing. |
| V5 Safe by default | Every entry is verified by `log`, every package by its BLAKE3 hash, before anything leaves `dist`. Each read has a size and a time limit. |
| V6 Chosen trust | The participant chooses the channels that they follow. `dist` has no default channel and no global list. |
| V1 Own environment | `dist` never installs or runs anything. It only hands over what the participant asked for. |

## Boundaries: what `dist` must never become

- **Never a server.** `dist` never listens on a socket and never uploads.
  The maker copies the folder to a host with their own tools.
- **Never a signer without consent.** `dist` signs nothing that the app did
  not confirm, statement by statement (F-AGENT-4, W13).
- **Never a runtime or installer.** `dist` does not unpack, plan, install or
  run a mod (`host` does).
- **Never a trust authority.** No ranking, no curation, no global index, no
  trust policy (W11).
- **Never trusts a web host.** A host can delay statements. It cannot make
  `dist` accept a changed byte.
- **Never a second format owner.** The entry envelope is `log`'s. The
  package and the release body are `pkg`'s. `dist` owns only the bodies of
  `profile`, `withdraw` and the index kinds (`list`, `include`, `unlist`),
  the folder layout, the link and the follow list.

## Success

- Step 3 (J3): Alice publishes, confirms once, copies one folder to a
  static host and shares one link. Bob's `dist` follows the link, verifies
  the channel and the package, and hands both to `host`. A changed byte on
  the host is refused with its cause named.
- Step 4 (J4): a refresh reports new versions and withdrawals of followed
  channels, and a fork marks a channel compromised and stops it. Search
  reads chosen indexes with the same verification.
- Always: every refusal names its cause (F-QUAL-3); everything except a
  read from a web host works offline (F-QUAL-1).
