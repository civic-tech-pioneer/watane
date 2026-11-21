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
| `app` | Gives the participant a window, and the one place for every approval. | `host`, `dist`; `node` from step 8 | the desktop app | 2 |
| `log` | Makes statements that others can verify: the identity and the signed channel. | none | none | 3 |
| `dist` | Moves releases between people: publish, follow, find. | `log`, `pkg` | none | 3, 4 |
| `net` | Connects machines directly: node keys, classes, the wire, pairing, sync (`PEERS.md`). | `log` | none | 8 |
| `node` | Joins `dist` and `net`: the holder, the sync loops, the always-on program. | `dist`, `net` | `watane-node` | 8 |
| `install` | Puts watane on a machine: a script, a packer, a release workflow and their documents. | none (it packs the programs of `host`, `dev`, `node` and `app`) | `install.sh` | 7 |

```text
programs     watane-dev        watane        desktop app      watane-node
subsystems   dev ──▶ host ◀── app ──▶ node ──▶ dist
                                       └─────▶ net
base         pkg (used by dev, host, dist)      log (used by host, dist; net: types only)
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
- `dist` and `net` do not know each other. `net` defines the holder trait;
  `node` implements it over `dist` and runs the loops between them (step 8).
  `app` reaches `dist` and `net` through `node` from step 8.
- `install` owns a script, a packer, a workflow and documents, and no code
  that another subsystem calls; its one crate (`watane-install`) holds
  tests only (`install/TECHNICAL.md` 1). It is the one subsystem that
  nothing depends on and that is not part of watane at run time.
- The inner modules of a subsystem are its own business (its `TECHNICAL.md`).
- Each subsystem is one crate `watane-<name>` at the root of its folder.
  The root `Cargo.toml` is a workspace: a subsystem's crate becomes a member
  when it is created. The workspace uses resolver `3`, one root `Cargo.lock`
  and the shared root `target/`; S1 members are `pkg`, `host`, and `dev`.
  A dependency on a sibling is a path dependency
  (`{ path = "../pkg" }`).
  One version of Wasmtime (48.0.8) and of each wasm-tools crate is in the build.

## 3. The mod

A mod is a folder. The WIT world is its contract; nothing else repeats it.

```text
mod.toml        name, summary, repository, [http] hosts, [origin] release
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
| export an interface of another package | a subscription to those events and collects of that provider |
| import an interface of its own package | none: these are the events and collects that the mod sends |
| import `watane:host/caller` | none: always present |
| import `watane:host/manage` (step 10) | the management of mods (W35) |
| import `watane:host/claude-code` (step 11) | `ClaudeCode`: start Claude Code to read and change files in folders the participant chose (W38) |
| import `watane:host/claude-code-shell` (step 11) | `ClaudeCodeShell`: also run programs and use the web, inside Claude's sandbox (W38) |
| import `watane:host/https-stream` (step 11) | none: it reaches only the hosts of the mod's `Http` grants (W38) |
| import `streams`, `claude-types`, `claude-session`, `agent-tools`, `timers`; export `stream-sink`, `stream-producer`, `timer-sink` (step 11) | none (W38) |

**Events and collects.** A function of an interface that a mod imports from
its own package is an **event** when it has no result, and a **collect**
when its result is `list<T>` (step 9). Any other result is refused. One
interface may hold both. A subscriber exports the interface and so answers
both; one subscription grant covers them.

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
mods; a publish refuses a dependency with no release. The one exception to
the pin is a provider that the participant picks in the consent view
(F-BUILD-3, W26).

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
  identity: func() -> string;  // step 11: <publisher>/<slug>, local/<slug>, page or agent:<handle-name> (host 19.1)
}
interface manage { ... }      // step 10 (W35): reads, reductions, requests; host 6.4, 6.10
// step 11 (W38): streams, claude-types, claude-code, claude-code-shell, claude-session,
// https-stream, agent-tools, timers; exports stream-sink, stream-producer, timer-sink
```

**Agent layer (step 11).** The interfaces `streams`, `claude-types`,
`claude-code`, `claude-code-shell`, `claude-session`, `https-stream`,
`agent-tools` and `timers`, all `watane:host/<name>@0.1.0`, join the
imports above, with the host-only exports `stream-sink`, `stream-producer`
and `timer-sink`. Their WIT, the streams, the launch of `claude`, the
accounts, the tools, the requests, the consents and the errors are in
`host/TECHNICAL.md` 19.

- A mod is a reactor: each call runs in a new instance and ends.
- The writes of a call commit only when the call succeeds. In a nested
  call, the writes of each mod commit when its own call succeeds.
- An event is a call to an interface of the mod's own package. The host
  queues it and, after the call succeeds, delivers it as one call into each
  granted subscriber.
- A collect (step 9) runs inside the call that makes it. The host invokes
  the same function in each granted subscriber, in local-name order, as a
  nested call, and returns the concatenation of their lists. A subscriber
  that fails is left out and logged; its writes and events are dropped.
  The chain's fuel and time stay shared, so an answer that spends them
  fails the collector too.
  The collector never learns who answered, as an emitter never learns its
  subscribers. A collect link is an edge from the collector to the
  subscriber, so a subscriber of a collect cannot also call its collector
  (no cycles, as for calls).
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
| `watane-dev` | the home host | the local API of the running app: `host` answers status, list, plan and install; `app` adds publish, search (step 4) and fetch (step 5) on the same port |
| app | `host`, `dist` | library calls in one process |
| `dist` | a static web host | HTTP GET to read; a folder of plain files to publish |

**Homes.** A home is one data directory with one running host. The app
uses the home of the participant, which holds the identity. A preview uses
a home of its own, with no identity and no data of the participant.

**Homes.** The home layout is `host`'s. It reserves `log/` for `log`,
`dist/` for `dist`, `app/` for `app` and `net/` for `net`; a preview home has none of them.

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
| Broker | Mods from each other | the caller comes from the transport, never from the request; a grant check at each call and at each answer of a collect; no cycles of call and collect links |
| Toolset | Mods and the participant from an agent | only the calls of the toolset; a token for each run; no request with an `Origin` header |
| Local API | The participant from web pages and local programs | no request with an `Origin` header; no request has authority without the confirmation floor |
| Builder | The maker's device from mod source | offline build, locked dependencies from a reviewed allowlist, no build script or procedural macro in the mod; unit tests run as Wasm with no capabilities |
| Verification | The participant from web hosts and other people | signatures, a hash chain, package hashes; fork detection |
| Confirmation floor | The participant from agents and tools | each install, grant, signature and pick of a provider needs a confirmation in the app (F-AGENT-4) |
| Manager (step 10) | Mods and the participant from a managing mod | the `Manage` grant checked at each function and before each effect; reads, three reductions (disable, withdraw a grant, close a page) and opening a page only; every install, update, enable, grant and removal is a request that the app decides in the consent view, which names the manager, one open at a time; effects only when the manager's call succeeds; no signature, no consent answer, no identity (W35; host 6.10, app 17) |
| Install (step 7) | The participant's machine from a tampered download | TLS to GitHub; the SHA-256 of the archive from the `SHA256SUMS` of the same release, checked before any file is written; the version checked against a pattern before it reaches a URL; removals only of paths that watane installs, whatever the receipt says; no root; the toolchain only after a typed yes; a release built with a read-only token and no cache (`install/TECHNICAL.md` 3.2, 5, 7, 8.5, 10) |
| Agent layer (step 11) | The participant, their folders and accounts, and other mods, from agents and from the mods that run them | the properties of agentic-harness 14 that watane files hold: a mod cannot widen `claude` past its launch (one argument per value, fixed flags, the host owns stdin, `initialize` and `init` checked); a mod cannot run a denied skill by name (a message that starts with `/` is refused, `slash-command`); a failing check cannot let a tool run (`onFailure: block` confirmed); a check answers `none` or `deny`, never `allow`; writes cannot plant code that runs later (protected paths); no mod sees a credential (tokens and keys stay with the app and the host; MCP tokens on fd 3; handle names, not handles; `claude`'s file tools denied the credential files and any `allow` answer outside the workspaces); a plan token goes only to OpenAI's agent endpoints and a judge key only to its judge (account binding, no redirects, `POST`, the origin of the provider's `[http]`); a mod in the middle gains no authority (consents per origin client, carried by identity and checked at apply, and no timer left carrying a removed mod; a subscriber is its own origin client; a toolset consent covers only the functions it named, less those of a removed mod or a new manager; the providers' `sessions` refuse any caller but the kernel); a session is private (one consumer, owner checks by `caller.identity`); a repeated call changes nothing (keys, numbered items); an ask shows the call that will run (the host builds each `ask`, `check` and `ask-resolved` item itself) (W38, W39; host 19, app 19) |
| Node (step 8) | The participant from other machines | the caller's node key from the QUIC handshake; class from the device list only; limits in bytes before work; pull only; pairing confirmed in the app (`net/TECHNICAL.md` 6) |

Not protected in the first version:
- another program of the same user. It can read the files of the home, and
  it can call the loopback addresses of the host as if it were a page;
- a compromised operating system;
- a fault in Wasmtime: all mods run in the host process (decision W7);
- a web host that withholds new statements. It can delay, but not forge;
- one identity on two devices. The two would fork the channel, so the
  first version supports one device for each identity. Step 8 keeps that
  rule: own machines hold no identity (W33);
- a compromised repository, maintainer account or release workflow: it can
  replace the archive and its checksums together, and the piped script is not
  covered by a checksum (`install/TECHNICAL.md` 5, 10);
- a compromised own machine, within its trust: it can withhold, and lie
  about completeness, until the participant removes it (`PEERS.md` 12);
- the agent layer (step 11), beyond its boundary (agentic-harness 14): the
  judge account reaches any `POST` path of the origin the participant
  entered; `claude` itself within its limits, and a prompt injection within
  what the profile allows; an allowed package-runner word (`test`,
  `install`, `add`, `ci`, `uv pip`, `sync`, ...) runs the project's own scripts, and an
  allowed command's own options (`git -c`, GNU `sed`'s `e`) or a first word
  that the allow list misses can run others, inside the shell grant's
  sandbox only, whose loopback reach is still to be probed (V27), and only for
  a profile the participant made or adopted with that allow rule; whether a
  subagent prompt that starts with `/` runs a skill (not probed, V26); a
  duplicate key nested in an ask's `input` is read last-wins, so the ask
  shows the call that will run only while the host serializes the item
  itself rather than passing `claude`'s bytes (host 19.3); the plan usage of
  the participant's own Claude or ChatGPT account. Open until the human
  answers (host 18): a workspace or account consent kept without its provider
  is reused by another provider or a mod in the middle with its own grant
  (Q38); under the shell grant, a `Bash` child reading
  `CLAUDE_CODE_OAUTH_TOKEN`, and `WebFetch` reaching loopback (Q39, probe
  V28); one session reading another's scratch or run folder (Q40).

## 7. Decisions of this fresh start

Each one closes a gap of the old specification. Veto any of them.

| # | Decision |
|---|---|
| W1 | Six subsystems (eight from step 8: `net`, `node`). `host` is one subsystem; its inner modules need no cross-subsystem contract. |
| W2 | The WIT world is the contract. `mod.toml` holds only what WIT cannot say. Grants come from the world. Consents (workspace, account, toolset) are stored with the grants (host 4.4; W38). |
| W3 | An installed mod has one model: a verified folder in the home. A mod has at most one logic part. |
| W4 | A local mod installs unsigned and is marked local. |
| W5 | A preview has no identity and no data of the participant. It grants each mod what the mod declares, with no question, except that it refuses the two Claude Code grants and the accounts: dev's fakes stand in (W38; dev 18). |
| W6 | The home host gives a toolset token only in the app, or from a toolset consent of the participant (an agent's handle, W38). A preview prints its token where it starts. |
| W7 | All mods run in the host process, in the Wasmtime sandbox. One process for each mod is a later hardening. |
| W8 | Pages are served only on the origins of the host, also inside the app. |
| W9 | An ungranted import is a stub that fails at the call, not a failure to load the mod. |
| W10 | Distribution is static files first. Direct transfer between own machines comes in step 8; between friends and strangers later (`PEERS.md` 10). |
| W11 | The first version has no trust policy. Safety is signatures, hashes, fork detection, isolation and consent. |
| W12 | An agent has a toolset and a token, and the confirmation floor holds. Finer tool controls come later; the agent layer's boundary is W39. |
| W13 | Only `dist` signs with the participant key, and only after a confirmation in the app. A publish needs the running app. `net` signs only with its own node key, and only the forms of `net/TECHNICAL.md` 4.3. |
| W14 | `deps.json` pins the publisher of each provider. |
| W15 | The statement kinds are `profile`, `release` and `withdraw`. Step 4 adds the kinds of an index: `list` (a mod), `include` (another index) and `unlist` (dist 4.1). |
| W16 | A release is named `<publisher key>/<slug>@<version>` outside its channel. |
| W17 | The slug and the version are in the WIT package line `mods:<slug>@<version>`, not in `mod.toml`. |
| W18 | After a restore of the identity, nothing is signed until the newest statement of the own channel is known. |
| W19 | A confirmation with no answer is a no: closing the consent view, quitting the app, or 10 minutes without an answer. |
| W20 | An index is a channel, not a new thing: any channel may hold `list`, `include` and `unlist` statements, and it is an index for a participant who chooses it from its link. The own channel always counts. A listing names a mod, never a version or bytes: those come only from the publisher's channel. (step 4, S26 draft) |
| W21 | Includes go one level deep: the listings of an included index count, its includes do not. A home reads a bounded number of channels because of listings (dist 5.9), and never a listed address on its own device or network. (step 4, S26 draft) |
| W22 | Search is local: a filter over the channels already read and verified, by words in the text, by publisher key and by interface (offered, sent as events, or used). No network, no global ranking; the order uses only the participant's own choices. (step 4, S26 draft) |
| W23 | watane ships with no index and no default channel. The participant adds the first index from a link that a person gave them. (step 4, S26 draft) |
| W24 | A derived mod names the release it was made from in `mod.toml` `[origin]`; the release body carries it (pkg 7, since step 3). It is the publisher's claim: watane shows it as such and never checks it against the origin's channel. The origin is the direct parent, not the first ancestor. (step 5, S32) |
| W25 | A provider's channel that this home has no record of is read from the bases of the live `list` and `include` statements that name its key in the channels the home already reads (dist 5.11). When a release depends on a provider of another publisher, the publish asks the participant to `list` that provider in the own channel, so the consumer's channel says where its providers live. No new format. (step 5, S32) |
| W26 | A mod **fits** in place of a named provider when it has the same slug and satisfies every interface that the consumer calls or subscribes to from it (pkg 5.2), whatever its publisher. It binds only after the participant picks it in the consent view; the host keeps the pick as a bind of that consumer, asks for each new bind, and deletes the bind when the picked mod is removed (host 4.5). (step 5, S32) |
| W27 | A preview matches a provider by slug and interface, not by publisher: it holds no participant data (W5), and dev installs a published provider there from a folder. (step 5, S32) |
| W28 | The mods that derive from a mod are a search over the channels already read, filtered by origin publisher and slug (dist 5.10); no new statement. (step 5, S32) |
| W29 | A mod with source has its own git repository; `mod.toml` `repository` names it, so the signed package carries the link. Packages stay in the channel's site folder: `dist`, `log` and the release body do not change. A fork on GitHub is never a trust fact: the origin is still the signed `[origin]` (W24). A derived mod's folder is a git clone of its origin's repository at the release's tag, used only when it holds the verified source; at its first `publish --push` its repository becomes a GitHub fork of the origin's where GitHub allows it, else a new repository with that public history (dev 10.4). (step 6) |
| W30 | GitHub is the default host, not a requirement (V4). `watane-dev publish --push` runs the maker's own `git` and `gh`; watane holds no account and no token (F-MAKE-6). The default base of the link is the raw URL of the site repository (`https://raw.githubusercontent.com/<owner>/<repo>/<branch>/`); GitHub Pages or any other static host also works, because the base is only where to read. Without `--push`, publish writes the folder as before; a maker on another host pushes it with their own git. (step 6) |
| W31 | A link names a channel by key and a **location**: a URI, read by the resolver of its scheme (`dist` 4.2). `http` and `https` are the only resolvers now; a node resolver comes with `PEERS.md` stage D3, and others (a purl, a content address) when a step needs them. A link whose scheme has no resolver here still parses: the listing stays live, and a read from it fails with the scheme named. So a later scheme never breaks an older reader's view of an index. |
| W32 | `net` is off until the participant turns it on. No relay, DHT, bootstrap host or public peer is on by default; each is one choice, named in its consent. A personal node serves strangers nothing in step 8. (step 8) |
| W33 | A replica holds no participant key: `log` and `dist` open a home with no identity, which can read, verify and store but never sign. Own machines are bound by a device list that the anchor's node key signs, never by a public statement. (step 8) |
| W34 | **Collect** (step 9): an own-package import whose function returns `list<T>` asks every granted subscriber and returns their answers concatenated, in local-name order; a function with no result stays an event; any other result is refused. It reuses the subscription link and the `Subscribe` grant: no new grant kind, no new file, no new host interface. A collect link is a cycle edge from collector to subscriber; event links stay no edges. A failing subscriber is skipped, not fatal, except when it spends the chain's shared fuel or time; the collector does not learn who answered. Answers carry no source: a collector that must address one answerer later says so in the answer type (for example a command id), and reaches it with an event. |
| W35 | **Management interface** (step 10): the host offers the management of mods through `watane:host/manage`; importing it is the grant `Manage`, the fifth kind, shown in the plan like any other. What a manager may do follows the direction of authority. It reads (the installed mods, their grants, links and origins) and reduces (disable, withdraw a grant, close a page) directly, because a hostile manager can then only make things do less. It asks for each addition (install, update, give a grant, pick a provider) and for a removal that deletes data; the request goes to the consent view, which decides it as if the participant had started it (F-AGENT-4). It never signs, answers a consent or reaches the identity. The app's own manager is a native view on the same functions. Designed in S73: the WIT and the effects in host 6.4 and 6.10, the grant in pkg 4.5 and 5.1, the consent words and requests in app 17. |
| W36 | **Main mod**: at start the app opens the main mod, else its manager (F-APP-1, F-APP-7). The choice is the participant's and lives in the app's part of the home; the host knows nothing of it. A mod proposes itself with `main = true` in `mod.toml` (pkg 4.2), and only a mod with a page may; the app takes the proposal at install when no main mod is set. A shell is a mod like any other: there is no application level above mods. |
| W38 | **The agent layer** (step 11; agentic-harness TECHNICAL, N3 accepted 2026-11-20): mods run agent sessions on the participant's own Claude or ChatGPT plan. It is a layer of mods on the host: the kernel `mods:agent` and the two providers `mods:claude-code` (unchecked, and checked by a judge) live in their own repositories. watane gives them the host interfaces of host 19 (streams, the launch of `claude`, HTTPS streams with accounts, agent tools, requests and timers, consents, errors), the two grants `ClaudeCode` and `ClaudeCodeShell` of pkg 5.1, the app's accounts and consents of app 19, and dev's fakes of dev 18 (the binary `fake-claude`, the test server `ScriptedResponses` with its judge origin). Consents (workspace, account `openai`, toolset) are kept by the host with the grants, per origin client, and withdrawn like grants. The host reaches tokens, keys and the `claude` path only through the in-process trait `AccountSource` that the app implements; with no app, or no entry, it fails `no-app` or `no-account`. watane holds no key and runs no service. |
| W39 | **The agent layer's boundary** (agentic-harness 14): a mod cannot widen `claude` past its launch, a failing check cannot let a tool run, no mod sees a credential, a token and a judge key go only to the endpoints they are bound to, a mod in the middle gains no authority (consent per origin client), a session is private to its owner, and the confirmation floor (F-AGENT-4) holds: no agent installs, grants or signs. The security row and its "Not protected" lines are in section 6. The host and app files that hold each property are named in agentic-harness 14; a slice that changes one gets the adversary step. |

## 8. Build order

| Step | Build | Done when |
|---|---|---|
| 1 Make | `pkg` (mod folder, world, grants, pack); `host` (run a mod, state, pages, toolset, preview); `dev` | J1 passes |
| 2 Run together | `host` (plan, install, links, grants, events, removal); `app` | J2 passes |
| 3 Publish | `log`; `dist` (own channel, publish folder, follow); the confirmation in `app`; `watane-dev publish` | J3 passes |
| 4 Find | `dist` (index, search, updates, withdrawals); the views in `app` | J4 passes |
| 5 Build on | `dev` (derive, depend on a release); `host` (binds); `dist` (provider bases, derived search); `app` (fetch, pick, derived view) | J5 passes |
| 6 Share on GitHub | `pkg` (`repository`); `dev` (`publish --push`, `new --from` forks the repository) | J3 and J5 pass with GitHub as the host (in tests, local git remotes and `fake-gh`, dev 15.4) |
| 7 The real world | `install` (a person can get watane: J7); then nothing new: each fake of the journeys gives way to the real thing (`factory/PLAN.md`) | J7 passes on a clean Mac; the journeys pass on real hosts and machines |
| 8 Own machines | `log` and `dist` (a keyless open; the holder methods); `net` (stage D1 of `PEERS.md`); `node` (`watane-node`); `app` (turn on, pair, devices, relay, DHT switch) | J8 passes |
| 9 Extension points | `pkg` (the event or collect rule of an outlet function); `host` (collect in the linker and the call algorithm; collect edges in the cycle check; fakes for collects in `Harness`); `dev` (collect fakes and answers in contract tests, the template text); `app` (the words of a subscription) | J9 passes |
| 10 Own manager | `pkg` (the `Manage` grant, stage 11); `host` (`watane:host/manage`, its requests to the confirmation, manage fakes in `Harness`; stage 11); `dev` (the manage fake and `manages` in contract tests; stage 13); `app` (the words of the grant and of each request, the windows a reduction closes; stage J, after stage I) | J10 passes |
| 11 Agent layer | `pkg` (two grants); `host` (interfaces of host 19, stages A1 to A5); `dev` (`fake-claude`, `ScriptedResponses`; dev 18); `app` (accounts, consents, the `claude` setting, the judge key; stage L) | the watane parts of stages A to E of agentic-harness 17 pass; J11 passes (on the real `claude` with the consent steps at stage C) |

A journey test drives the real programs. A step is not done on the tests of
one subsystem alone. J1 lives in `dev/tests/`; J2, J3 and J4 live in
`app/tests/`, because they drive the app. J5 lives in `app/tests/` too.
J7 lives in `install/tests/clean_mac.sh`, because it needs a clean Mac.

## 9. Not designed yet

Steps 1 to 3 are designed in the documents of the subsystems: the builder
allowlist and the contract test form in `dev`, the grant words in `app`,
the local name in `host`, the published folder, the link and the statement
bodies in `dist`, the backup format in `log`. These wait for their step:

Step 4 is designed (S26 draft, W20 to W23): the statements of an index,
the read set and the search in `dist/TECHNICAL.md` 4.1, 5.9 and 5.10; the
find view and `/api/search` in `app/TECHNICAL.md` 7, 8.2 and 9.6;
`watane-dev search` in `dev/TECHNICAL.md` 10.

Step 5 is designed (S32, W24 to W28): `new --from` and `add --release` in
`dev/TECHNICAL.md` 10; binds and the preview rule in `host/TECHNICAL.md`
4.2 and 4.5; provider bases, `Fetched.link` and the origin filter in
`dist/TECHNICAL.md` 5.7, 5.10 and 5.11; `/api/fetch`, the pick request, the
provider listings of a publish and the derived view in `app/TECHNICAL.md`
6, 8.1, 8.3 and 9.7.

Step 6 is designed (S40, W29, W30): the `repository` key and its grammar
in `pkg/TECHNICAL.md` 4.2 (pkg stage 8); `publish --push`, the fork of
`new --from`, their results, errors and warnings in `dev/TECHNICAL.md`
10.4 (dev stages 10 and 11). In the default suite, a local bare
repository per GitHub repository, git's own `insteadOf` rewrite of
`https://github.com/`, and `fake-gh` on `PATH` stand in (dev 15.4); J3 and
J5 each gain a journey on that stand-in (`app/tests/it/j3.rs::ac2_j3_github`,
`app/tests/it/j5.rs::ac2_j5_github`), served from a clone of the pushed site
repository. Only the opt-in `real-github` journeys reach GitHub, in
throwaway repositories they create and delete (app 11).

Step 7's first slice, installing, is designed (S50a): `install/` holds the
script, the release archive, the checksums, the Gatekeeper step, the release
workflow, upgrade and uninstall, and the clean-Mac test (`install/TECHNICAL.md`
1 to 12; build plan in 11). Linux is stated out of scope there (9).

Step 8 is designed (`PEERS.md`, `net/VISION.md`, `net/FUNCTIONAL.md`,
`net/TECHNICAL.md`; W31 to W33; S59): `node/` (the holder over `dist`, the
calls to `net`, `watane-node`; node stages 1 to 3), the keyless open in
`log/TECHNICAL.md` 5.5 (stage L6), the keyless open and the holder methods
in `dist/TECHNICAL.md` 5.12 (stage S8), and the views in
`app/TECHNICAL.md` 9.8 (stage G). W31 changes the link parser
now, before any second scheme is published.

Step 9 is designed (W34): the outlet rule in `pkg/TECHNICAL.md` 4.5; the
collect in `host/TECHNICAL.md` 4.2, 6.4, 6.6 and 9.4; fakes and answers in
`dev/TECHNICAL.md` 8.1; the grant words in `app/TECHNICAL.md` 4. It needs
nothing of steps 7 and 8, so it can be built beside them. J9 lives in
`app/tests/`.

Step 10 is designed (W35; S73): `watane:host/manage` in
`host/TECHNICAL.md` 6.4 and 6.10 (reads, reductions, requests, when they
take effect, the request table, the `catalog` page message, `Confirm::
{manage, reduced}`, `Requester::Mod`; host stage 11); the grant `Manage`
and its release encoding in `pkg/TECHNICAL.md` 4.5, 5.1 and 7.2 (pkg stage
11); the manage fake and `manages` in `dev/TECHNICAL.md` 8.1 (dev stage
13); the words, the consent requests and J10's tests in `app/TECHNICAL.md`
17 (app stage J). J10 lives in `app/tests/it/j10.rs`. The human answered
the six choices on 2026-11-16 (`host/TECHNICAL.md` 18, Q28 to Q33): a
removal that keeps the data, and an enable, are requests; a manager may
install from a channel the home does not read, after a first consent to
read its link; a manager may open or focus a page (`open-page`), never
while a consent is open; the consent view offers "Deny and stop asks
from <mod>", which withdraws `Manage`; and every party that may call a
manager uses its grant, like every grant (S82 builds Q30 to Q32).

Step 11 is designed (W38, W39; S95): the interfaces, streams, the launch of
`claude`, the accounts, the tools, the consents and the errors in
`host/TECHNICAL.md` 19 (host stages A1 to A5); the grants in
`pkg/TECHNICAL.md` 4.5, 5.1 and 7.2; the fakes in `dev/TECHNICAL.md` 18; Sign
in with ChatGPT, the consent words, the `claude` setting and the judge key in
`app/TECHNICAL.md` 19 (app stage L). Left open until the human answers (host
18): `caller.boot()` (B1), the `outcome` of `ask-resolved {by: answered}` and the
host-item rules of agentic-harness 4.5. The kernel and the providers are
specified and built in agentic-harness. J11 lives in `app/tests/`. It comes
before the factory (step 6) in the order of work, and numbers 11 only
because steps 7 to 10 were named first.

The main mod (W36) and the manager of the app are not a step: they are
app stage I (`app/TECHNICAL.md` 5.3, 5.4), with the `main` key in
`pkg/TECHNICAL.md` 4.2 (pkg stage 10).

## 10. Reference material

`../colos` holds the prototypes. They are reference, not a base: code is
written new here, against these specifications. Each subsystem names the
prototypes that taught the most.
