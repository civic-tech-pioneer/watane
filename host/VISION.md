# host: vision

## Why

Mods are useful only together, and safe only when one party guards every
authority. `host` is that party. It is the one program that loads a mod.
The same code runs a preview for a maker and the home of a participant.
Every call between a page, an agent, a mod and the device crosses `host`,
and `host` decides each crossing from the grants that the participant
approved.

## Goals served (spec/VISION.md)

| Goal | How `host` serves it |
|---|---|
| V1 Own environment | A home starts empty and holds only the mods that the participant installed. |
| V2 Stigmergy | Mods of different makers run together, wired through their WIT interfaces and events. |
| V3 Cheap making | A preview runs a mod seconds after its build, with the grants it declares, and gives the coding agent a toolset. |
| V5 Safe by default | Sandbox, linker, origin, broker and toolset boundaries; nothing installs or gets a grant without the confirmation of the participant. |
| V6 Chosen trust | `host` checks the signature and the package hash of each release, and trusts no web host. |

## Boundaries: what `host` must never become

- **Never a signer, never a fetcher.** `host` holds no key and reads no
  channel or package from a network. The caller supplies every package.
- **Never a compiler.** `host` loads only built components.
- **Never a window.** `host` asks for a confirmation through a trait. The
  app answers it in its own process.
- **Never a second authority.** No request to the local API, to `/call` or
  to `/mcp` carries authority of its own. In a home, only the confirmation
  of the participant adds a mod or a grant.
- **Never a store of shared data.** Each mod has private state. Data moves
  between mods only through calls and events that the participant granted.

## Success

- J1: a coding agent runs `watane-dev serve`. The preview serves the page,
  and the agent calls the mod through MCP with the printed token.
- J2: in the app, two mods install in one plan with the provider first, call
  each other, react to events, keep their state over a restart, and stop at
  a withdrawn grant.
- J3: a release of another participant installs only when its signature and
  package hash check out, and a changed byte gives a refusal that names the
  cause.
- A hostile mod in any of these journeys cannot read another mod's state,
  reach the device, call an ungranted interface, or run without limit.
