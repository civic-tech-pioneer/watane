# Technical specification: watane

How watane does what `FUNCTIONAL.md` says. This file names the subsystems,
what each one owns, and what flows between them. The exact interface of a
subsystem is in its own `TECHNICAL.md`, and only there.

## 1. Constraints

- **TC-1** All trusted code is Rust.
- **TC-2** A logic part is a WebAssembly component (WASI 0.2), run by Wasmtime.
- **TC-3** A page is plain web content (HTML, CSS, JavaScript) on an origin of its own.
- **TC-4** The contract of a mod is WIT: its exports, its imports and its events.
- **TC-5** The app is Tauri 2. The host also runs with no app.
- **TC-6** No function needs a server that a participant must trust.
- **TC-7** macOS (aarch64) first, Linux (x86_64) second.

## 2. Subsystems

Each subsystem is one folder beside `spec/`, in one git repository.

| Subsystem | Reason to exist | Depends on | Program | Step |
|---|---|---|---|---|
| `pkg` | Defines what a mod and a package are. | none | none | 1 |
| `host` | Runs the installed mods together and guards every authority. | `pkg`, `log` | `watane` | 1, 2 |
| `dev` | Lets a maker and a coding agent make a mod. | `pkg`, `host` | `watane-dev` | 1 |
| `app` | Gives the participant a window, and the one place for every approval. | `host`, `dist` | the desktop app | 2 |
| `log` | Makes statements that others can verify: the identity and the signed channel. | none | none | 3 |
| `dist` | Moves releases between people: publish, follow, find. | `log`, `pkg` | none | 3, 4 |

```text
programs     watane-dev        watane        desktop app
subsystems   dev ──▶ host ◀── app ──▶ dist
base         pkg (used by dev, host, dist)      log (used by host, dist)
```

Rules:
- No cycle. A subsystem uses only the subsystems in its "Depends on" cell.
  A type of a lower subsystem can pass through a higher one: `app` gets the
  types of `pkg` and `log` through the re-exports of `host` and `dist`.
- `host` never signs, and never fetches a channel or a package. A request
  of a mod with a network grant is the mod's own. `dist` never runs a mod.
- `dev` never signs. The host and the app never compile a mod.
- `log` is opened only by `dist`. `dist` passes the backup and the restore
  of the identity through to `app`.
- The inner modules of a subsystem are its own business (its `TECHNICAL.md`).
- Each subsystem is one crate `watane-<name>` at the root of its folder.
  The root `Cargo.toml` is a workspace: a subsystem's crate becomes a member
  when it is created. A dependency on a sibling is a path dependency
  (`{ path = "../pkg" }`).
  One version of Wasmtime (48.0.8) and of each wasm-tools crate is in the build.

## 3. The mod

A mod is a folder. The WIT world is its contract; nothing else repeats it.

```text
mod.toml        name, summary, [http] hosts, [origin] release
wit/world.wit   package mods:<slug>@<version>; the exports, imports and events
wit/deps/       the WIT of each provider, and of watane:host
deps.json       each provider mods:<slug>: its publisher key, or null if local
logic/          a Rust crate that builds to logic.wasm      (optional)
logic.wasm      the built logic part                        (optional)
ui/             index.html and its files                    (optional)
tests/          contract tests
```

The slug and the version are in the WIT package line, and nowhere else. The
version of a provider is the version of its WIT in `wit/deps/`. A mod with
only a page still has a world, which may be empty. The exact rules are in
`pkg/TECHNICAL.md`.

**Grants come from the world.** A mod asks for exactly what its world
imports and exports. There is no second list.

| In the world | Grant |
|---|---|
| import `watane:host/state` | private storage |
| import `wasi:http/outgoing-handler`, with hosts in `mod.toml` | network access to those hosts |
| import an interface of another package | a call to that interface of the provider in `deps.json` |
| export an interface of another package | a subscription to those events of that provider |
| import an interface of its own package | none: these are the events that the mod sends |
| import `watane:host/caller` | none: always present |

**Package.** A package is the built mod folder as one canonical file: equal
input gives equal bytes. It holds `mod.toml`, `wit/`, `deps.json`,
`logic.wasm`, `ui/` and, where the maker agrees, the source (`logic/` and
`tests/`). It is at most 64 MiB. The package id is the BLAKE3 hash of its
bytes.

**Text forms.** A publisher key is 52 characters of z-base-32 (`log`
defines it, `pkg` uses the same text). A release is named
`<publisher key>/<slug>@<version>`; a channel publishes each version of a
slug at most once, so this name is unique. The id of a log entry is used
only inside its own channel.

**Identity of a mod.** A mod is `<publisher key>/<slug>`, and a local mod is
`local/<slug>`. `deps.json` pins the publisher of each provider, so a WIT
package name alone never binds a provider. A local mod can depend on local
mods; a publish refuses a dependency with no release.

**Installed mod.** One model: the verified package, unpacked into
`<home>/mods/<local name>/`, with a record of its package id, its release,
its publisher and its approved grants. The host gives each installed mod a
local name that is unique in that host. The origin of the page and the tool
names use the local name. The private state is kept by the identity of
the mod, so that kept data comes back when the mod is installed again.

## 4. Host interfaces

A logic part reaches the host only through `watane:host`:

```wit
package watane:host@0.1.0;

interface state {             // private storage of this mod
  get: func(key: string) -> option<list<u8>>;
  put: func(key: string, value: list<u8>);
  delete: func(key: string);
}
interface caller {            // "page", "agent:<handle>" or "mod:<local name>"
  who: func() -> string;
}
```

- A mod is a reactor: each call runs in a new instance and ends.
- The writes of a call commit only when the call succeeds. In a nested
  call, the writes of each mod commit when its own call succeeds.
- An event is a call to an interface of the mod's own package. The host
  queues it and, after the call succeeds, delivers it as one call into each
  granted subscriber.
- An import with no grant is linked to a stub. A call to it fails the
  whole call with the code `not_granted`, which the page shows. The mod
  itself still loads and runs its other calls.
- The base WASI interfaces are linked with nothing behind them: no
  directory, no environment variable and no socket.

## 5. What flows between the parts

| From | To | Through |
|---|---|---|
| page | its own mod | `watane.js`: `call(fn, args)` and `on(fn)`, over `/call` and `/events` on the origin of the mod |
| mod | host | the WIT interfaces of section 4 |
| mod | mod | WIT imports that the host links and grants; events |
| agent | host | MCP at `/mcp/<toolset>`, with a token |
| `watane-dev` | a preview host | it starts the program `watane` on a preview home, then uses the local API |
| `watane-dev` | the home host | the local API of the running app: `host` answers status, list, plan and install; `app` adds publish (and later search and fetch) on the same port |
| app | `host`, `dist` | library calls in one process |
| `dist` | a static web host | HTTP GET to read; a folder of plain files to publish |

**Homes.** A home is one data directory with one running host. The app
uses the home of the participant, which holds the identity. A preview uses
a home of its own, with no identity and no data of the participant.

**Homes.** The home layout is `host`'s. It reserves `log/` for `log`,
`dist/` for `dist` and `app/` for `app`; a preview home has none of them.

**The local API** is HTTP on the loopback interface of a running host. The
host listens on `127.0.0.1` and `::1` with one port, because macOS looks up
`*.localhost` at `::1` first. Any local program can send a request. That is
safe because a request has no authority of its own: in the home host, each
install, grant and signature waits for the confirmation of the participant
in the app. The app gives that confirmation inside its process, never over
the local API. Remove, disable, grant, revoke and tokens have no route on
the local API at all. An error is `{"error": {"code", "message"}}`.

## 6. Security model

| Boundary | Protects | How |
|---|---|---|
| Sandbox | The device and the host from a logic part | Wasmtime; fuel, memory and time limits for each call |
| Linker | The host and other mods from an ungranted import | only granted imports reach a provider; the rest are stubs |
| Origin | The host and other mods from a page | one origin for each mod; a strict content policy; `/call` accepts only the exact origin |
| Broker | Mods from each other | the caller comes from the transport, never from the request; a grant check at each call; no cycles |
| Toolset | Mods and the participant from an agent | only the calls of the toolset; a token for each run; no request with an `Origin` header |
| Local API | The participant from web pages and local programs | no request with an `Origin` header; no request has authority without the confirmation floor |
| Builder | The maker's device from mod source | offline build, locked dependencies from a reviewed allowlist, no build script or procedural macro in the mod; unit tests run as Wasm with no capabilities |
| Verification | The participant from web hosts and other people | signatures, a hash chain, package hashes; fork detection |
| Confirmation floor | The participant from agents and tools | each install, grant and signature needs a confirmation in the app (F-AGENT-4) |

Not protected in the first version:
- another program of the same user. It can read the files of the home, and
  it can call the loopback addresses of the host as if it were a page;
- a compromised operating system;
- a fault in Wasmtime: all mods run in the host process (decision W7);
- a web host that withholds new statements. It can delay, but not forge;
- one identity on two devices. The two would fork the channel, so the
  first version supports one device for each identity.

## 7. Decisions of this fresh start

Each one closes a gap of the old specification. Veto any of them.

| # | Decision |
|---|---|
| W1 | Six subsystems. `host` is one subsystem; its inner modules need no cross-subsystem contract. |
| W2 | The WIT world is the contract. `mod.toml` holds only what WIT cannot say. Grants come from the world. |
| W3 | An installed mod has one model: a verified folder in the home. A mod has at most one logic part. |
| W4 | A local mod installs unsigned and is marked local. |
| W5 | A preview has no identity and no data of the participant. It grants each mod what the mod declares, with no question. |
| W6 | The home host gives a toolset token only in the app. A preview prints its token where it starts. |
| W7 | All mods run in the host process, in the Wasmtime sandbox. One process for each mod is a later hardening. |
| W8 | Pages are served only on the origins of the host, also inside the app. |
| W9 | An ungranted import is a stub that fails at the call, not a failure to load the mod. |
| W10 | Distribution is static files first. Direct transfer between devices comes later. |
| W11 | The first version has no trust policy. Safety is signatures, hashes, fork detection, isolation and consent. |
| W12 | An agent has a toolset and a token, and the confirmation floor holds. Finer tool controls come later. |
| W13 | Only `dist` signs, and only after a confirmation in the app. A publish needs the running app. |
| W14 | `deps.json` pins the publisher of each provider. |
| W15 | The statement kinds are `profile`, `release` and `withdraw`. Step 4 adds the kinds of an index. |
| W16 | A release is named `<publisher key>/<slug>@<version>` outside its channel. |
| W17 | The slug and the version are in the WIT package line `mods:<slug>@<version>`, not in `mod.toml`. |
| W18 | After a restore of the identity, nothing is signed until the newest statement of the own channel is known. |
| W19 | A confirmation with no answer is a no: closing the consent view, quitting the app, or 10 minutes without an answer. |

## 8. Build order

| Step | Build | Done when |
|---|---|---|
| 1 Make | `pkg` (mod folder, world, grants, pack); `host` (run a mod, state, pages, toolset, preview); `dev` | J1 passes |
| 2 Run together | `host` (plan, install, links, grants, events, removal); `app` | J2 passes |
| 3 Publish | `log`; `dist` (own channel, publish folder, follow); the confirmation in `app`; `watane-dev publish` | J3 passes |
| 4 Find | `dist` (index, search, updates, withdrawals); the views in `app` | J4 passes |
| 5 Build on | `dev` (derive, depend on a release); `host` (pick a provider) | J5 passes |

A journey test drives the real programs. A step is not done on the tests of
one subsystem alone. J1 lives in `dev/tests/`; J2, J3 and J4 live in
`app/tests/`, because they drive the app.

## 9. Not designed yet

Steps 1 to 3 are designed in the documents of the subsystems: the builder
allowlist and the contract test form in `dev`, the grant words in `app`,
the local name in `host`, the published folder, the link and the statement
bodies in `dist`, the backup format in `log`. These wait for their step:

- Step 4: the statements of an index; how deep includes go; the search by
  interface; whether watane ships with a first index.
- Step 5: when the interface of another provider "fits"; where to read the
  channel of a provider that is not followed (a link in `deps.json`).

## 10. Reference material

`../colos` holds the prototypes. They are reference, not a base: code is
written new here, against these specifications. Each subsystem names the
prototypes that taught the most.
