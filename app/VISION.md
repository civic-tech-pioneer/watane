# app: vision

## Why

A participant needs one window to use mods in, and one place that no mod,
agent or tool can imitate, where every approval happens. `app` is that
window and that place. It puts the `host` and the `dist` of the
participant's home in one process and answers their confirmations there.

## Goals served

| Goal | How `app` serves it |
|---|---|
| V1 Own environment | The launcher starts empty and shows only what the participant added. |
| V5 Safe by default | The consent view is the only path to an install, a grant or a signature (F-AGENT-4). Mod views reach no app command. |
| V4 No center | The app creates the identity on the device, with no server. It publishes to a folder, not to a service. |
| V2 Stigmergy | The app installs the mods of others from a link and shows where each mod came from. |
| V6 Chosen trust | The app shows the publisher and the verification result of each mod. The participant decides. |

## Boundaries: what `app` must never become

- **A second rulebook.** Each rule about installing, running, signing or
  verifying lives in `host`, `dist`, `pkg` or `log`. The same rules hold
  with no app. The app shows, asks and forwards.
- **A door for mods.** No mod view gets an app command, a plugin command,
  or a way to answer a confirmation.
- **A confirmation over the network.** The app answers inside its process.
  No HTTP route answers a confirmation (TECHNICAL.md section 5).
- **A store or a feed.** It ranks, curates and recommends nothing.
- **A framework.** Its own interface is plain HTML, CSS and JavaScript.

## Success

- J2 passes on the real app: an empty launcher, a plan in the consent view,
  two mods that work together, data after a restart, a withdrawn grant and
  a removal.
- J3 passes on two real apps: a publish that the participant confirms, and
  an install from a link that shows a verified publisher.
- A hostile mod page reaches no app command, no other origin and no other
  window. A test proves this on every change.
