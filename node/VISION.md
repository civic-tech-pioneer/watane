# node: vision

## Why

`dist` holds the channels and packages of a home and verifies every byte.
`net` connects the participant's own machines and moves signed objects
between them. Neither may know the other (spec TECHNICAL 2): `dist` keeps
no thread and no network but HTTP GET, and `net` stores no channel.
Something has to join them, and a server in a cupboard needs a program
that runs a home with no window and no identity.

`node` is that join. It gives `net` a holder over a `Dist` handle, tells
`net` when a head moved here, asks `net` for what a light machine lacks
before `dist` reads, and runs `watane-node`: the always-on, keyless home
that keeps the participant's machines current (spec step 8, J8).

## Goals served

| Goal | How `node` serves it |
|---|---|
| V1 Own environment | The participant's machines keep each other current; a light machine finds and installs through a full one. |
| V4 No center | `watane-node` runs on the participant's own hardware. It needs no host, no account and no endpoint that someone else runs; it can be the relay of the participant's own machines. |
| V5 Safe by default | A node home holds no participant key, so a stolen server cannot sign. Everything `net` hands over is verified again by `dist` before it is stored. |
| V6 Chosen trust | Pairing goes through the participant's confirmation in the app; `node` adds no trust rule of its own. |

## Boundaries: what `node` must never become

- **Never a rulebook.** Verification, storage and the statement rules stay
  in `dist` and `log`; classes, the wire and limits stay in `net`. `node`
  only calls them in the right order.
- **Never a signer.** `node` never asks `dist` to sign on its own. Own
  statements are signed only through the app's consent, as before.
- **Never a cache of its own.** `node` keeps no copy of channels or
  packages; `dist` holds them.
- **Never a second network.** `node` opens no socket. Every byte between
  machines goes through `net`; every byte from a web host through `dist`.
- **Never a requirement.** With `net` off, `node` is a thin pass-through
  and every function of the home works as before.

## Success

- `watane-node` on a home server, paired as a full machine, holds a
  release within seconds of its publish on the laptop, with no static host.
- A light laptop searches and installs through it and verifies every byte.
- The server's disk holds no participant key.
- With `net` off, J3, J4 and J5 pass unchanged on a home opened through
  `node`.
