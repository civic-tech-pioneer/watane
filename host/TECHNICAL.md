# host: technical specification

How `host` does what `FUNCTIONAL.md` says. Requirement ids `HOST-F-<n>`
refer to `FUNCTIONAL.md`. `spec/TECHNICAL.md` sections are cited as
"spec §n". Where this file and `spec/` differ, section 18 says why.

## 1. Crate

One crate at the root of this folder.

```toml
[package]
name = "watane-host"
version = "0.1.0"
edition = "2024"
rust-version = "1.95"

[lib]
name = "watane_host"

[[bin]]
name = "watane"
path = "src/bin/watane.rs"

[dependencies]
watane-pkg = { path = "../pkg" }
watane-log = { path = "../log" }            # added in stage 8 (section 15)
wasmtime = "48.0.8"                          # LTS line; default features
wasmtime-wasi = "48.0.8"
wasmtime-wasi-http = "48.0.8"
wit-parser = "0.254.2"                       # doc comments; version already resolved in Cargo.lock
axum = "0.8.9"
tokio = { version = "1.53", features = ["rt-multi-thread", "macros", "net", "signal", "sync", "time"] }
futures-util = { version = "0.3", default-features = false, features = ["std"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
redb = "4.5"
blake3 = "1.8"
base64 = "0.22"
getrandom = "0.3"

[dev-dependencies]
tempfile = "3"                               # test homes; Harness uses getrandom
reqwest = { version = "0.12", default-features = false, features = ["json", "stream"] }
tower = { version = "0.5", features = ["util"] }
```

Evidence: wasmtime 48 (an LTS line) asks for `wit-parser` ^0.254.1 in its
`Cargo.toml`. Cargo.lock resolves it to 0.254.2, so `host` and `pkg` link
one copy. Since the colos prototype
(wasmtime 38): `LinkerInstance::func_new` takes a closure
`Fn(StoreContextMut<T>, ComponentFunc, &[Val], &mut [Val])`, and HTTP
interception is `WasiHttpHooks::send_request`. Local toolchain: rustc 1.97,
target `wasm32-wasip2`.

### 1.1 Layout

| Path | Job |
|---|---|
| `src/lib.rs` | `Host`, re-exports, `HOST_WIT`, `WATANE_JS`, `DEFAULT_PORT` |
| `src/error.rs` | `HostError`, `CallError`, codes (section 2.2) |
| `src/home.rs` | home layout, lock, `home.json`, `run.json`, `LocalName`, `default_home` (section 3) |
| `src/catalog.rs` | sources, plan, install, update, remove, grants, `installed.json` (section 4) |
| `src/verify.rs` | package, release and import checks (section 5) |
| `src/runtime/mod.rs` | engine, registry snapshot, the call algorithm (section 6) |
| `src/runtime/linker.rs` | host interfaces, links, stubs, outlets, fakes |
| `src/runtime/limits.rs` | `Limits`, epoch ticker, limiter, error classification |
| `src/runtime/json.rs` | JSON to WIT values, JSON Schema, WIT type text |
| `src/runtime/http.rs` | the network grant hooks |
| `src/state.rs` | redb store and transactions (section 7) |
| `src/bus.rs` | outbox records, delivery, page bus and SSE adapter (section 9) |
| `src/server.rs` | bind, routing by `Host` header, guards, `Served` (section 8) |
| `src/pages.rs` | files, headers, `/call`, `/events` |
| `src/watane.js` | the page library (section 8.5) |
| `src/tools.rs` | toolsets, tokens, MCP (section 10) |
| `src/api.rs` | local API (section 11) |
| `src/confirm.rs` | `Confirm`, `InstallRequest`, `Answer` (section 12) |
| `src/harness.rs` | `Harness` for contract tests (section 2.6) |
| `src/bin/watane.rs` | the program (section 12.2) |
| `wit/host.wit` | the `watane:host` package (section 6.4) |
| `tests/*.rs`, `tests/fixtures/` | section 13 |

## 2. Public API

Everything not listed here is private. All public types implement `Debug`
(`Mode` and `Host` by hand, without the `Confirm` object).
View types derive `Clone, Serialize` (and `Deserialize` where noted).

### 2.1 Host

```rust
pub use {watane_log, watane_pkg};   // the types of this API (section 17); app needs no direct dependency
pub const HOST_WIT: &str = include_str!("../wit/host.wit");
pub const WATANE_JS: &str = include_str!("watane.js");
pub const DEFAULT_PORT: u16 = 47341;  // of a participant home (section 12.2)

pub enum Mode {
    /// The home of a participant. `app` answers confirmations; `None` refuses them.
    Home { app: Option<Arc<dyn Confirm>> },
    /// A preview: grants what each mod declares, never asks.
    Preview,
}

pub struct Host { /* private; Send + Sync */ }

impl Host {
    pub fn open(home: &Path, mode: Mode, limits: Limits) -> Result<Arc<Host>, HostError>;
    pub fn home(&self) -> &Path;
    pub fn is_preview(&self) -> bool;

    // catalog (section 4)
    pub fn plan(&self, sources: Vec<Source>) -> Result<Plan, HostError>;
    pub async fn install(&self, plan: Plan, requester: Requester) -> Result<Vec<InstalledMod>, HostError>;
    pub fn mods(&self) -> Vec<InstalledMod>;                       // sorted by local name
    pub fn get(&self, local: &LocalName) -> Option<InstalledMod>;
    pub fn enable(&self, local: &LocalName) -> Result<(), HostError>;
    pub fn disable(&self, local: &LocalName) -> Result<(), HostError>;
    pub fn remove(&self, local: &LocalName, delete_data: bool) -> Result<(), HostError>;
    pub fn grant(&self, local: &LocalName, grant: &Grant) -> Result<(), HostError>;   // only a declared grant
    pub fn revoke(&self, local: &LocalName, grant: &Grant) -> Result<(), HostError>;

    // runtime (section 6)
    /// The callable functions: `world().exports` only, never a subscription export (6.6 step 2).
    pub fn functions(&self, local: &LocalName) -> Result<Vec<FunctionInfo>, HostError>;
    /// Blocking. Needs a thread with at least 16 MiB of stack.
    pub fn call(&self, caller: &Caller, local: &LocalName, function: &str, args: Value) -> Result<Value, CallError>;
    /// Runs `call` on a new thread with a 16 MiB stack.
    pub async fn call_async(self: &Arc<Self>, caller: Caller, local: LocalName, function: String, args: Value) -> Result<Value, CallError>;

    // tools (section 10)
    pub fn set_toolset(&self, name: &ToolsetName, entries: Vec<String>) -> Result<(), HostError>;
    pub fn remove_toolset(&self, name: &ToolsetName) -> Result<(), HostError>;
    pub fn toolsets(&self) -> BTreeMap<ToolsetName, Vec<String>>;
    pub fn issue_token(&self, toolset: &ToolsetName, handle: &Handle) -> Result<Token, HostError>;
    pub fn end_token(&self, id: &TokenId) -> bool;                  // false: no such live token
    pub fn tokens(&self) -> Vec<TokenInfo>;

    // server (section 8)
    pub async fn serve(self: &Arc<Self>, port: u16, extra_api: axum::Router) -> Result<Served, HostError>;
}

pub struct Served { pub port: u16, /* private: shutdown sender, join handle */ }
impl Served {
    pub fn page_url(&self, local: &LocalName) -> String;   // "http://<local>.localhost:<port>/"
    pub fn api_url(&self) -> String;                       // "http://localhost:<port>/api/"
    pub fn mcp_url(&self, toolset: &ToolsetName) -> String; // "http://localhost:<port>/mcp/<toolset>"
    pub async fn shutdown(self);                           // stops, removes run.json
}

pub fn default_home() -> Result<PathBuf, HostError>;       // section 3.1
pub struct RunInfo { pub watane: u32, pub pid: u32, pub port: u16, pub mode: String }  // Serialize, Deserialize
impl RunInfo { pub fn read(home: &Path) -> Result<Option<RunInfo>, HostError>; }

/// The ready line of the program `watane` (section 12.2). Serialize, Deserialize.
pub struct Ready {
    pub watane: u32, pub mode: String, pub home: PathBuf, pub port: u16, pub api: String,
    pub mcp: Option<String>, pub token: Option<String>, pub mods: Vec<ReadyMod>,
}
pub struct ReadyMod { pub local: LocalName, pub page: Option<String> }
```

Product callers pass `Limits::default()` to `Host::open`; tests lower one.

### 2.2 Errors

```rust
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    Busy(PathBuf),                                         // "busy"
    ModeMismatch { path: PathBuf, found: &'static str },   // "mode_mismatch"
    BadHome { path: PathBuf, reason: String },             // "bad_home"
    InvalidMod { input: String, reason: String },          // "invalid_mod"; input: folder path or slug; reason: pkg's error text
    Verify { check: VerifyCheck, reason: String },         // "verify"
    MissingProviders(Vec<MissingProvider>),                // "missing_providers"
    Cycle(Vec<LocalName>),                                 // "cycle"
    AmbiguousProvider { provider: Provider, a: LocalName, b: LocalName }, // "ambiguous_provider"
    Refused(String),                                       // "refused"
    StalePlan,                                             // "stale_plan"
    NoSuchMod(String),                                     // "no_mod"
    NotDeclared { local: LocalName, grant: Grant },        // "not_declared"
    BadName { value: String, rule: &'static str },         // "bad_name"
    BadToolsetEntry { entry: String, reason: String },     // "bad_toolset_entry"
    NoSuchToolset(String),                                 // "no_toolset"
    PortInUse(u16),                                        // "port_in_use"
    AlreadyServing,                                        // "already_serving"
    Internal(String),                                      // "internal" (I/O, redb, wasmtime)
}
impl HostError { pub fn code(&self) -> &'static str; pub fn status(&self) -> u16; }

pub enum VerifyCheck { Kind, ReleaseBody, PackageHash, Mismatch }
// serialized snake_case: "kind", "release_body", "package_hash", "mismatch" (section 5.2)

pub struct CallError { pub code: CallCode, pub message: String }
pub enum CallCode { BadRequest, NoMod, NoFunction, Disabled, NotGranted, Trap, Limit(Limit), Internal }
pub enum Limit { Fuel, Memory, Time, Depth, StateSize, StateQuota }
```

Each `Display` text names the cause and the thing (mod, grant, path,
check). JSON form of a `CallError`:
`{"code":"limit","limit":"fuel","message":"..."}`; `limit` is present only
for code `limit`. JSON form of a `HostError`:
`{"code":"<code>","message":"<Display>"}`, plus `"missing":[...]` for
`missing_providers`, `"check":"<check>"` for `verify`, `"cycle":[...]` for
`cycle`.

| HostError code | HTTP status |
|---|---|
| `bad_name`, `not_declared`, `bad_toolset_entry` | 400 |
| `refused` | 403 |
| `no_mod`, `no_toolset` | 404 |
| `busy`, `mode_mismatch`, `missing_providers`, `cycle`, `ambiguous_provider`, `stale_plan` | 409 |
| `invalid_mod`, `verify` | 422 |
| `bad_home`, `port_in_use`, `already_serving`, `internal` | 500 |

| CallCode (JSON `code`) | HTTP status on `/call` |
|---|---|
| `bad_request` | 400 (413 and 415 for size and type, section 8.2) |
| `no_mod`, `no_function` | 404 |
| `disabled` | 409 |
| `not_granted`, `trap`, `limit` | 422 |
| `internal` | 500 |

### 2.3 Names and callers

```rust
pub struct LocalName(String);   // grammar: section 3.2; Display, AsRef<str>, Ord, Hash, Serialize, Deserialize (validated)
impl LocalName { pub fn parse(s: &str) -> Result<LocalName, HostError>; }
pub struct ToolsetName(String); // [a-z][a-z0-9-]{0,30}
pub struct Handle(String);      // [a-z][a-z0-9-]{0,30}
// both: parse(&str) -> Result<Self, HostError>, Display, Ord, Serialize, Deserialize (validated)

pub enum Caller { Page, Agent(Handle), Mod(LocalName) }
// `watane:host/caller.who` returns "page", "agent:<handle>", "mod:<local>"
```

### 2.4 Catalog types

Types of `pkg` and `log` keep their own serde forms (section 17). A release
is named by its `ReleaseRef` text `<publisher key>/<slug>@<version>`; the
host never stores a log entry id. An identity is pkg's `Provider` text (4.1).

```rust
pub enum Source {
    Folder(PathBuf),                                  // a built mod folder; local, unsigned
    Release { package: Package, entry: Entry },       // a package and its signed release entry (stage 8, section 5.2)
}

pub struct Plan { /* private: entries, missing, staging dirs, generation */ }
impl Plan {
    pub fn entries(&self) -> &[PlanEntry];             // install order: providers first
    pub fn missing(&self) -> &[MissingProvider];
    pub fn needs_confirmation(&self) -> bool;          // section 12.1
}
// Serialize as {"entries":[PlanEntry], "missing":[MissingProvider]}. Drop removes the staging dirs.

pub struct PlanEntry {
    pub local: LocalName, pub identity: String, pub slug: Slug, pub name: String,
    pub summary: String, pub version: Version, pub publisher: Option<PublisherKey>, // None: local
    pub package: PackageId, pub release: Option<ReleaseRef>, // None: local
    pub origin: Option<ReleaseRef>,                          // mod.toml [origin] release
    pub change: Change, pub grants: Vec<Grant>, pub new_grants: Vec<Grant>,
    pub providers: Vec<LocalName>,
}
pub enum Change { New, Update { from_version: Version }, Unchanged }
// JSON: {"kind":"new"} | {"kind":"update","from_version":"0.1.0"} | {"kind":"unchanged"}

pub struct MissingProvider {      // also Deserialize
    pub provider: Provider, pub version: Version,   // what a Call or Subscribe grant needs
    pub release: Option<ReleaseRef>,                // `<publisher>/<slug>@<version>`; None for a local provider
    pub needed_by: String,                          // identity
}

pub struct InstalledMod {         // also Deserialize; fields as in PlanEntry
    pub local: LocalName, pub identity: String, pub slug: Slug, pub name: String,
    pub summary: String, pub version: Version, pub publisher: Option<PublisherKey>,
    pub package: PackageId, pub release: Option<ReleaseRef>, pub origin: Option<ReleaseRef>,
    pub enabled: bool, pub has_logic: bool, pub page: Option<String>, // URL once served and has ui/
    pub grants: Vec<GrantState>, pub links: Vec<LinkView>, pub error: Option<String>,
}
pub struct GrantState { pub grant: Grant, pub granted: bool }
pub struct LinkView {
    pub kind: LinkKind /* "call" | "subscribe" */, pub interface: InterfaceName,
    pub provider: Option<LocalName>, pub provider_interface: Option<InterfaceName>,
}
```

### 2.5 Runtime and tools types

```rust
pub struct Limits {
    pub fuel: u64, pub memory: usize, pub time: Duration, pub depth: u32,
    pub key_bytes: usize, pub value_bytes: usize, pub call_write_bytes: usize,
    pub quota_bytes: u64, pub hops: u8, pub deliveries: usize,
}
impl Default for Limits { /* section 6.2 */ }

pub struct FunctionInfo {
    pub interface: InterfaceName,       // "mods:shelf/items@0.1.0"; .interface() is the short name
    pub name: String,                   // "add"
    pub params: Vec<(String, String)>,  // name, WIT type text
    pub result: Option<String>,         // WIT type text
    pub docs: Option<String>,
}

pub struct Token(String);               // "wt_" + 43 base64url chars; as_str(), id()
pub struct TokenId(String);             // 16 hex chars
pub struct TokenInfo { pub id: TokenId, pub toolset: ToolsetName, pub handle: Handle }
```

### 2.6 Harness (contract tests for `dev`)

```rust
/// Interface without version ("mods:shelf/items") -> function -> JSON result.
pub type Fakes = BTreeMap<String, BTreeMap<String, Value>>;

pub struct Harness { /* private: Arc<Host> on a temporary preview home, LocalName, Fakes */ }
impl Harness {
    /// Installs the built folder alone in a new temporary preview home.
    /// Call imports use fakes; subscriptions need no installed provider.
    pub fn open(built_mod: &Path, fakes: Fakes) -> Result<Harness, HostError>;
    /// A top-level call as `Caller::Page`, on a new 16 MiB stack thread.
    pub fn call(&self, function: &str, args: Value) -> Result<Outcome, CallError>;
    /// Invoke a subscription export as mod:<provider slug>, using its short interface name.
    pub fn deliver(&self, function: &str, args: Value) -> Result<Outcome, CallError>;
}
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome { pub value: Value, pub events: Vec<EventRecord> }
#[derive(Debug, Clone, PartialEq)]
pub struct EventRecord { pub interface: InterfaceName, pub function: String, pub args: Value /* object by name */ }
```

`Harness` is `Send`; `call` can run on any thread. Each `open` creates a
directory with a random name under `std::env::temp_dir()` using `getrandom`,
then uses the preview `Host::open` and `install_folders` path. State starts
empty, and simultaneous harnesses never share a home. Drop closes the host
before removing the directory. An open failure also removes the directory.
The doc-hidden `home()` accessor exposes its path for cleanup tests.
No server listens. Events are recorded in emission order instead of published
to the page bus. A failed call returns only its `CallError`, commits no state,
and records no events.

`Harness::functions` waits for a later slice. `deliver` accepts only functions
of `world().subscriptions`, commits on success and records emitted events
under the same rules as `call`. A fake returns its JSON converted to the
import's result type. A conversion mismatch is `trap` naming the function.
A function with no fake fails with `trap`, "no fake for
<interface>.<function>". The network import will deny every request.

## 3. Home

### 3.1 Layout

```text
<home>/
  home.json        {"watane_home":1,"mode":"home"} or {"watane_home":1,"mode":"preview"}
  lock             empty file; held with File::try_lock while a host has the home open
  run.json         {"watane":1,"pid":4242,"port":47341,"mode":"home"}; exists only while serving
  installed.json   the catalog (section 3.3)
  toolsets.json    {"<toolset>":["<entry>", ...]}  (section 10.1)
  mods/<local>/    one unpacked package each, in pkg's layout
  state/state.redb the private state of every mod (section 7)
  staging/         plan scratch; emptied at open
  log/ dist/ app/  reserved for `log` (opened only by `dist`), `dist` and `app`
```

`host` never reads or writes the three reserved names, and never creates
another name. A preview home has none of them (W5).

`default_home()`: `$WATANE_HOME` if set and not empty, else
`$HOME/.watane`. It does not create the directory.

`Host::open(home, mode, limits)`:
1. Create the directory if absent. Hold `File::try_lock` on `lock` (std,
   Rust 1.89+) before reading or creating the marker; a held lock is `busy`.
2. If no entry other than `lock` exists, create `home.json` for `mode`.
   Else read `home.json`; a missing or unknown file is `bad_home`; another
   mode is `mode_mismatch`.
3. Empty `staging/`. Restore `mods/<local>.old` if it matches the committed
   package and `mods/<local>` does not. Delete each `mods/<x>` that
   `installed.json` does not name, and each remaining `mods/*.new` and
   `mods/*.old`.
4. Read each installed mod with `Mod::read`, compile its `logic.wasm` when
   `logic()` is `Some`, and build the registry (section 6.4). A mod that fails to
   load keeps its record and gets `error`; the host still opens.
5. Start the epoch ticker (section 6.3).

### 3.2 Local name grammar

```abnf
local-name = lower *(lower / digit) *("-" 1*(lower / digit))   ; 1 to 24 bytes
lower      = %x61-7A
digit      = %x30-39
```

A local name is a DNS label and contains no `_`, so a tool name
`<local>_<interface>_<function>` splits without doubt. 24 bytes keeps tool
names within the 64-byte tool-name limit of common agent clients.

Derivation for a new mod: a pkg `Slug` already fits this grammar, so take
the slug, truncate it to 21 bytes and trim a trailing `-`. Use it if no
installed mod and no other plan entry has it; else append `-2`, `-3`, ...
and take the first free name.
An update keeps the local name of the installed mod.

### 3.3 `installed.json`

```json
{
  "watane_installed": 1,
  "generation": 7,
  "mods": [
    {
      "local": "recs",
      "identity": "local/recs",
      "package": "<64 hex chars>",
      "release": null,
      "enabled": true,
      "grants": [{"grant": <Grant>, "granted": true}],
      "links": [{"kind": "call", "interface": "mods:shelf/items@0.1.0",
                 "provider": "shelf", "provider_interface": "mods:shelf/items@0.1.0"}]
    }
  ]
}
```

`release` is the `ReleaseRef` text, or `null` for a local mod; the
publisher is the release's. Slug, version, name, summary and origin are
read from `mods/<local>/` at open; they are not repeated here. Each write: serialize, write
`installed.json.tmp`, `sync_all`, rename over `installed.json`, sync the
directory. `generation` increases by one on each write.

## 4. Catalog

### 4.1 Identity

A mod's identity is the text of pkg's `Provider` for it:
`<publisher key>/<slug>`, or `local/<slug>` for a mod from a folder. A home holds
at most one mod per identity. Two folders with the same slug are one local
identity: the second is an update of the first.

### 4.2 Plan

`Host::plan(sources)` runs these steps and returns the first error:

1. For each source, get a package: `Folder(dir)` gives `pack(dir, false)`;
   `Release` runs the checks `kind`, `release_body` and `package_hash` of
   section 5.2. A `ModError` or `PackageError` is `invalid_mod`.
2. Create a random parent under `staging/`; unpack into its absent `mod/`
   child. `package.unpack(staging/<16 random hex>/mod/)` gives the `Mod`.
   Remove the parent and its contents when staging ends, including on error. For a
   release, run the check `mismatch` (5.2). Run the import check (5.3).
3. Two sources with one identity: `invalid_mod` ("twice in one plan").
4. `change`: no installed mod with the identity gives `New`; the same
   package id gives `Unchanged`; else `Update { from_version }`.
5. `local`: kept for an update; derived for a new mod (section 3.2).
6. `grants` = `mod.grants()`. `new_grants` = the declared grants with no
   `Grant::same_authority` match among the grants of the installed version
   (all of them for `New`).
7. Links, computed over the catalog as it would be after the install
   (installed mods, with plan entries replacing their identities). A grant
   `Call { interface, provider }` or `Subscribe { interface, provider }`
   carries the publisher that `deps.json` pins (`None` for local). The
   candidates are the mods with slug `provider.slug` and publisher
   `provider.publisher` that have an interface `I` with
   `interface.satisfied_by(I)`: `I` in `world().exports` for `Call`, in
   `world().events` (an outlet) for `Subscribe`.
   - No candidate: add a `MissingProvider` (one per provider and version;
     `release` = `<publisher>/<slug>@<version of interface>` when the
     publisher is set). Two or more: `ambiguous_provider`. One: a link.
8. Refuse a cycle of call links across the whole catalog (`cycle`, with
   the path, first name repeated at the end). Subscription links are no
   edges.
9. Order the entries topologically by their links (providers first); break
   ties by local name. `providers` lists the linked providers of each
   entry.
10. Record the catalog `generation`.

A plan with `missing` entries is valid. `install` refuses it.

### 4.3 Install

`Host::install(plan, requester)`:

1. Take the install mutex (one install at a time). If the generation
   changed since the plan: `stale_plan`. If `missing` is not empty:
   `missing_providers`.
2. Confirmation (section 12.1). It gives the withheld grants.
3. Granted set of each entry: `New`: `grants` minus withheld. `Update`:
   each grant of `grants` whose same-authority grant was granted, plus
   `new_grants` minus withheld; so a withdrawn grant stays withdrawn.
4. For each entry in order, skipping `Unchanged`: rename the staging dir
   to `mods/<local>.new`; for an update rename `mods/<local>` to
   `mods/<local>.old`; rename `.new` to `mods/<local>`.
5. Recompute the links of every installed mod (step 7 of 4.2). A link of
   another mod whose provider no longer fits becomes unbound.
6. Write `installed.json`. This is the commit point: a crash before it
   leaves the old catalog, and open (3.1 step 3) removes the leftovers.
7. Delete the `.old` dirs, build the new registry, swap it in, and send
   `{"type":"reload"}` to the pages of each updated mod.
8. Return the `InstalledMod` of each plan entry. If a step from 4 to 6
   fails, undo the renames in reverse order and return `internal`.

### 4.4 Enable, disable, remove, grant, revoke

Each runs under the install mutex, writes `installed.json` and swaps the
registry.

- `disable`: `enabled = false`. The mod gets no call, no tool and no
  delivery; a consumer's call into it fails with `not_granted` ("provider
  <local> is disabled"); its `/events` streams end; its page answers 404.
- `enable`: `enabled = true`; send `reload` to its pages.
- `remove(local, delete_data)`: wait for the mod's call lock; drop it from
  the registry; delete `mods/<local>/`; if `delete_data`, delete its state
  (section 7). Links to it become unbound; toolset entries stay and give no
  tool; kept state reattaches when the identity is installed again.
- `grant` and `revoke`: only for a grant in `mod.grants()`, else
  `not_declared`; they set `granted`. The app calls them in process after
  the participant acted, so the host asks nothing.

## 5. Verification

### 5.1 Folder

`pack` and `unpack` apply every rule of a mod (pkg), with
`check_component`. Rule `parts` gives F-RUN-1: at least one part, at most
one page and one logic part.

### 5.2 Release (stage 8)

`Source::Release { package, entry }` takes a `Package` and an `Entry`. Their
only constructors are the checked decoders of `pkg` and `log`, so the
package bytes parse and the entry's signature verifies before the host sees
them. The host then checks, in this order; the first failure is `verify`
with its check:

| Check | Fails when |
|---|---|
| `kind` | `entry.kind()` is not `RELEASE_KIND` |
| `release_body` | `Release::decode(entry.body())` (the `body`) fails |
| `package_hash` | `body.package` differs from `package.id()` |
| `mismatch` | after unpack, `body.check(&mod, &package)` fails; the message names the field |

The release is `ReleaseRef { publisher: PublisherKey::from_bytes(*entry.author().as_bytes()), slug: body.slug, version: body.version }`.
`mismatch` makes sure the unpacked mod has that slug and version, so the
ref names this package. The plan entry gets this `release` and its
publisher. Withdrawals and forks are checked by `dist` (step 4).

### 5.3 Import check

The host checks the component itself, also when pkg did, because it is the
security core. It reads the import and export names of the compiled
component against `mod.world()`. Allowed imports:

- `wasi:cli`, `wasi:io`, `wasi:clocks`, `wasi:random`, `wasi:filesystem`,
  `wasi:sockets` interfaces at `0.2.x`;
- `wasi:http/*@0.2.x`, only if `world().http`;
- `watane:host/state@0.1.0` only if `world().storage`; `watane:host/caller@0.1.0`;
- an interface in `world().events` whose functions all return nothing (an
  outlet);
- an interface in `world().calls`.

Allowed exports: the interfaces in `world().exports` (its calls) and
`world().subscriptions`. Anything else is `invalid_mod` with the name. A
world-level function (not in an interface) is never a call.

## 6. Runtime

### 6.1 Engine

One `wasmtime::Engine` per host:
`Config::new().consume_fuel(true).epoch_interruption(true).wasm_component_model(true).max_wasm_stack(1 << 20)`.
Components are compiled from `logic.wasm` at open and at install. No
compiled artefact is written to disk.

### 6.2 Limits

| Limit | Default | Scope | Error |
|---|---|---|---|
| `fuel` | 5 000 000 000 | one top-level call and all its nested calls | `limit`/`fuel` |
| `memory` | 64 MiB | each instance (linear memories together) | `limit`/`memory` |
| `time` | 10 s | one top-level call, nested calls and HTTP included | `limit`/`time` |
| `depth` | 8 | nested calls below a top-level call | `limit`/`depth` |
| `key_bytes` | 1 024 | one state key | `limit`/`state_size` |
| `value_bytes` | 4 MiB | one state value | `limit`/`state_size` |
| `call_write_bytes` | 16 MiB | keys + values put by one call | `limit`/`state_size` |
| `quota_bytes` | 256 MiB | stored keys + values of one mod | `limit`/`state_quota` |
| `hops` | 8 | event chain (section 9) | dropped, logged |
| `deliveries` | 256 | event deliveries per top-level call | dropped, logged |

Fixed, not in `Limits`: instances per store 64, tables 64, table elements
100 000, memories 16, captured stderr 64 KiB, confirmation wait 600 s,
`/call` and `/mcp` bodies 1 MiB, local API bodies 64 KiB.

### 6.3 Time

An epoch ticker thread calls `engine.increment_epoch()` every 10 ms. It
holds a `Weak` of the host and stops when the host drops. Each store sets
`set_epoch_deadline(1)` and an `epoch_deadline_callback` that returns
`Err(CallError limit/time)` when `Instant::now()` passes the chain deadline,
else `Ok(UpdateDeadline::Continue(1))`. A nested store uses the same
deadline.

### 6.4 Host interfaces and the linker

`wit/host.wit`:

```wit
package watane:host@0.1.0;

/// Private storage of this mod. The writes of a call become visible to
/// other calls only when the call succeeds.
interface state {
  get: func(key: string) -> option<list<u8>>;
  put: func(key: string, value: list<u8>);
  delete: func(key: string);
}

/// Who made the current call: "page", "agent:<handle>" or "mod:<local name>".
interface caller {
  who: func() -> string;
}
```

`delete` is a proposal to spec §4 (section 18, Q4).

The linker of a mod is built once per registry and kept as an
`InstancePre`. It holds:

| Import | Linked to |
|---|---|
| base WASI | `wasmtime_wasi::p2::add_to_linker_sync`, with the empty context of 6.5 |
| `wasi:http/*` | `wasmtime_wasi_http::p2::add_only_http_to_linker_sync`, hooks of 6.8 |
| `watane:host/state` | the state transaction; each function first checks the `Storage` grant, else `not_granted` ("<local> has no grant for watane:host/state") |
| `watane:host/caller` | the caller string of the store |
| outlet | `func_new`: append the event to the store's outbox (section 9) |
| call import | `func_new`: nested call (6.6) |
| in a `Harness`, a call import | `func_new`: the fake |

The link and the grant are not baked into the closure. The closure reads
them from the registry snapshot of the chain at each call. Thus a revoke
takes effect at the next top-level call, and a call never mixes two
registries.

### 6.5 Instance context

Each call gets a new `Store<Ctx>`. `Ctx` holds:
- a `WasiCtx` from `WasiCtx::builder()` with no preopened directory, no
  environment variable, no argument, empty stdin, stdout discarded, stderr
  in a 64 KiB `MemoryOutputPipe`, `allow_tcp(false)`, `allow_udp(false)`,
  `allow_ip_name_lookup(false)`; clocks and random are allowed;
- a `ResourceTable`;
- a `ResourceLimiter` with the memory limit and the fixed limits of 6.2;
  growth past a limit returns `Err(CallError limit/memory)`, so the call
  traps with that error;
- the state transaction of this mod (section 7);
- the caller string; the chain context (deadline, depth, hop, registry
  snapshot, effects); the outbox; the granted HTTP hosts.

### 6.6 The call algorithm

Top-level `call(caller, local, "<interface>.<function>", args)`:

1. Take the registry snapshot (`Arc`). The mod must exist (`no_mod`), be
   enabled (`disabled`) and have a logic part (`no_function`).
2. Find the function: `<interface>` is the short name
   (`InterfaceName::interface()`) of an interface in `world().exports`; a
   subscription export or a function with a resource type is `no_function`.
3. Convert `args` (section 6.9); a mismatch is `bad_request` with the path.
4. Create the chain context: deadline = now + `time`, fuel = `fuel`,
   depth 0, hop 0, empty effects.
5. Invoke (below). Convert the result to JSON.
6. Deliver the committed events (section 9), then send the `changed`
   messages. Return the result of step 5. The crate-private harness variant
   returns the result and outbox as `Outcome`, without page bus publication.

Invoke(mod, interface, function, values, ctx), used for top-level and
nested calls:

1. If `ctx.depth > limits.depth`: `limit`/`depth`.
2. Lock the mod's call mutex (one call per mod at a time). The call links
   are acyclic, so lock waits follow the link order and cannot deadlock.
3. New store; `set_fuel(ctx.fuel)`; deadline callback; instantiate from the
   `InstancePre`; call; `post_return`.
4. On success: read the fuel left; commit the state transaction; append
   the outbox to the chain effects; mark the mod changed if a value
   changed; unlock; return the values and the fuel left.
5. On failure: drop the transaction and the outbox; unlock; return the
   classified error (6.7).

A nested call (the `func_new` of a call import of consumer C):
1. From the snapshot: the link of C for this import must exist and point to
   an enabled provider P, and C must hold the `Call` grant. Else
   `not_granted` ("<C> may not call <interface>: not granted" or "... no
   provider is linked" or "... provider <P> is disabled").
2. Invoke(P, link.provider_interface, function, args, ctx with depth + 1,
   caller `mod:<C>`, fuel = the consumer store's fuel left).
3. Set the consumer store's fuel to the fuel that P left. Copy the results.
4. An error of P returns `Err` from the host function, so C traps; the
   error keeps its `CallError` (6.7).

The writes of P commit when P's call succeeds, even if C fails later
(section 18, Q3).

### 6.7 Error classification

Search the error and its causes for a `CallError`; the innermost one wins.
Else: wasmtime `Trap::OutOfFuel` is `limit`/`fuel`; any other trap or
error is `trap`, with the error text and the last 4 KiB of stderr. A
failure of the host itself (redb, I/O) is `internal`.

### 6.8 Network grant

Linked only when the component imports `wasi:http`. The hooks
(`WasiHttpHooks`):
- `send_request`: if the scheme is not `https`, return the error code
  `HTTP-request-denied` without a connection. Lowercase the URI host and
  strip one trailing `.`. If it is not exactly one of the mod's granted
  `Http` hosts, return `HTTP-request-denied` too. Else cap the connect,
  first-byte and between-bytes timeouts at the remaining chain time, and
  delegate to `default_send_request` (webpki roots).

HTTPS only also keeps a mod off the plain-HTTP loopback server of the host,
even through a DNS name that resolves to a loopback address.

### 6.9 JSON encoding of WIT values

One encoding for `/call`, MCP, fakes and `Harness`:

| WIT | JSON |
|---|---|
| `bool` | `true` / `false` |
| `u8` ... `u64`, `s8` ... `s64` | integer number, range-checked; `1.0` is refused |
| `f32`, `f64` | number; NaN and infinity come out as `null`; `null` is refused as input |
| `char` | string of exactly one Unicode scalar |
| `string` | string |
| `list<T>` | array |
| `tuple<...>` | array of exactly that length |
| record | object by field name; a missing field is allowed only for an `option` field; an unknown field is refused |
| `option<T>` | `null` for none, else the value of `T`; a function with `option<option<T>>` is not callable |
| enum | the case name |
| flags | array of names; unique; output in declaration order |
| variant | the case name for a case without payload, else `{"<case>": payload}` |
| `result<T, E>` | `{"ok": value}` or `{"err": value}`; `null` for no payload |
| resource, future, stream, error-context | not callable (`no_function`) |

Arguments: an object by parameter name (a missing `option` parameter is
none; an unknown name is refused), an array in order (exact length), or
`null` (same as `{}`). Result: `null` for no result, else the value. An
input error names its path: `$.items[2].status: expected enum {open, done}`.

JSON Schema (draft 2020-12) of a type, for MCP: integers carry `minimum`
and `maximum`; a record is `{"type":"object","properties":...,"required":
[non-option fields],"additionalProperties":false}`; `option` is
`anyOf [null, T]`; variant and result are `oneOf` of their cases; enum is a
string `enum`; flags an array of unique enum strings; char a string of
length 1; tuple an array with `prefixItems`, `minItems`, `maxItems`.

WIT type text (for `FunctionInfo` and descriptions): `u32`, `string`,
`list<u32>`, `option<string>`, `tuple<u8, string>`,
`result<u32, string>` (`_` for no payload), `record { a: u32, b: string }`,
`variant { a, b(u32) }`, `enum { a, b }`, `flags { a, b }`.

## 7. State

One redb database at `state/state.redb`.

| Table | Key | Value |
|---|---|---|
| `s/<id>`, one per mod | `&str` state key | `&[u8]` value |
| `sizes` | `&str` `<id>` | `u64` stored bytes (keys + values) |

`<id>` is the first 32 hex chars of `blake3(identity)`. State follows the
identity, not the local name (section 18, Q2).

A transaction lives for one invoke of one mod:
- It has an overlay `BTreeMap<String, Option<Vec<u8>>>` (`None` = deleted)
  and a redb read transaction opened at the first read.
- `get` reads the overlay first, then the read transaction.
- `put` checks `key_bytes`, `value_bytes` and `call_write_bytes`, then
  writes the overlay. `delete` writes `None`.
- Commit: if the overlay is empty, nothing. Else one write transaction:
  for each key, compare with the stored value; skip equal values; insert or
  remove and adjust the size. If the size passes `quota_bytes`, abort
  (nothing is written) and fail the call with `limit`/`state_quota`.
  Commit with redb's default durability. "Changed" is true when at least
  one value changed.
- Delete data: delete table `s/<id>` and remove `sizes[<id>]` in one write
  transaction.

A put of the stored value is no change, so a page that re-reads on
`changed` cannot loop through a read function that writes the same value.

## 8. Server and pages

### 8.1 Binding and routing

`serve(port, extra_api)` binds `127.0.0.1:<port>` and `[::1]:<port>`, and
serves both with one router (macOS resolves `<x>.localhost` to `::1`
first), and no other address. Port 0: `[::1]` takes the port that
`127.0.0.1:0` got; on a clash, try again, at most 8 times. A failed bind is
`port_in_use`. Then it writes `run.json`. A second `serve` is
`already_serving`.

Every request must have exactly one `Host` header. Repeated headers get
421, `text/plain; charset=utf-8`, "unknown host". The value is matched
byte for byte, with no case folding or normalization:

| Host header | Service |
|---|---|
| `localhost:<port>`, `127.0.0.1:<port>`, `[::1]:<port>` | API host: `/api/...` (section 11), `/mcp/...` (section 10) |
| `<label>.localhost:<port>` with `<label>` a valid local name | the mod origin of `<label>` |
| anything else, or a missing header | 421, `text/plain; charset=utf-8`, "unknown host" |

The exact host check stops DNS rebinding: a page of another site that
resolves its name to 127.0.0.1 sends its own name in `Host`.

No response carries an `Access-Control-Allow-Origin` header. Preflight
requests receive no `Access-Control-Allow-*` headers.

On the API host, a request with an `Origin` header gets 403 `forbidden`.
A transport refusal (a wrong `Origin`, method, type or size) answers
`{"error":{"code":"forbidden"|"bad_request","message":"..."}}` with its
status, before any handler runs.
On a mod origin of a mod that is not installed, or is disabled, every path
answers 404 `text/plain; charset=utf-8`.

`extra_api` is merged into the API host's router. Its routes must be under
`/api/` and must not use the paths of section 11 (axum panics on an
overlap, at `serve`). They get the host check and the `Origin` refusal.
The 64 KiB body limit applies only to the routes of section 11; an
`extra_api` route sets its own `DefaultBodyLimit` (app's publish takes a
package of up to 64 MiB). A mod origin never reaches them.

### 8.2 `POST /call`

| Check | Failure |
|---|---|
| method is `POST` | 405, `Allow: POST` |
| `Origin` is exactly `http://<local>.localhost:<port>` | 403 `forbidden` |
| `Content-Type` is `application/json` (parameters allowed) | 415 `bad_request` |
| body at most 1 MiB | 413 `bad_request` |
| body is `{"fn": "<interface>.<function>", "args": <object, array or null>}`, no other field | 400 `bad_request` |

A missing `Origin` gets the message "the Origin header is missing; it must
be http://<local>.localhost:<port>". A wrong, malformed, empty or repeated
`Origin` keeps "Origin must match the mod origin". Both use 403 `forbidden`.

A body-size refusal closes the HTTP/1 connection, so an unread body
cannot affect a later request on that connection.

The call runs as `Caller::Page` on a 16 MiB thread. Success: 200
`{"value": <json>}`. Failure: the status of section 2.2 and
`{"error": <CallError JSON>}`.

### 8.3 Page files and headers

`GET` and `HEAD` of any other path serve a file of `mods/<local>/ui/`:
- Percent-decode the path (it must be UTF-8). `/` is `index.html`.
- Refuse (404) a path with an empty segment other than the leading `/`, a
  segment that starts with `.`, a `\` or a NUL byte, or a path whose
  canonical form is outside the canonical `ui/`.
- `/watane.js`, `/call` and `/events` are the host's, also when `ui/` has
  such a file.

Content types: `.html` `text/html; charset=utf-8`; `.js` `.mjs`
`text/javascript; charset=utf-8`; `.css` `text/css; charset=utf-8`; `.json`
`.map` `application/json`; `.wasm` `application/wasm`; `.svg`
`image/svg+xml`; `.png` `image/png`; `.jpg` `.jpeg` `image/jpeg`; `.gif`
`image/gif`; `.webp` `image/webp`; `.ico` `image/x-icon`; `.woff2`
`font/woff2`; `.woff` `font/woff`; `.txt` `.md` `text/plain; charset=utf-8`;
others `application/octet-stream`. A 404 has `text/plain; charset=utf-8`.
A missing or refused file path returns 404 for every method. An existing
file, including `/watane.js`, returns 405 with `Allow: GET, HEAD` for
methods other than GET and HEAD.

Every response of a mod origin carries these headers:

```text
Content-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; media-src 'self' blob:; connect-src 'self'; worker-src 'self'; manifest-src 'self'; form-action 'self'; base-uri 'none'; frame-src 'none'; frame-ancestors 'none'; object-src 'none'
X-Content-Type-Options: nosniff
Referrer-Policy: no-referrer
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Resource-Policy: same-origin
Cache-Control: no-store
```

The policy allows no inline script and no network outside the origin. A
page runs as a top-level view, never in a frame.

### 8.4 `GET /events`

- If an `Origin` header is present and is not the exact mod origin: 403.
  A same-origin `EventSource` sends none (section 14, not protected).
- Response: `Content-Type: text/event-stream`, the headers of 8.3. Each
  message is `data: <json>\n\n`. Every 15 s without a message, the server
  sends `: ping\n\n`.
- Messages: `{"type":"changed"}`, `{"type":"reload"}`, and
  `{"type":"event","from":"<emitter local>","interface":"<full outlet name>","function":"<name>","args":{...}}`
  with `args` by parameter name in the encoding of 6.9.
- A receiver that lags on the bus (section 9.3) gets one
  `{"type":"changed"}` in place of the lost messages.
- The stream ends when the mod is disabled or removed, or the server stops.

### 8.5 `watane.js`

Served at `/watane.js` on each mod origin, `text/javascript; charset=utf-8`.
A page loads it with `<script src="/watane.js"></script>`. Exact source:

```js
// watane.js: the page side of a watane mod. It reaches only its own mod.
(() => {
  const handlers = new Set();
  let source = null;
  class WataneError extends Error {
    constructor(code, message, limit) {
      super(message);
      this.name = 'WataneError';
      this.code = code;
      this.limit = limit;
    }
  }
  const dispatch = (message) => {
    if (message.type === 'reload') { location.reload(); return; }
    for (const handler of handlers) {
      try { handler(message); } catch (error) { console.error(error); }
    }
  };
  window.watane = Object.freeze({
    WataneError,
    async call(fn, args) {
      const response = await fetch('/call', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ fn, args: args ?? null }),
      });
      const body = await response.json().catch(() => null);
      if (response.ok && body && 'value' in body) return body.value;
      const error = (body && body.error) || { code: 'internal', message: `HTTP ${response.status}` };
      throw new WataneError(error.code, error.message, error.limit);
    },
    on(handler) {
      handlers.add(handler);
      if (!source) {
        source = new EventSource('/events');
        source.onmessage = (event) => dispatch(JSON.parse(event.data));
        source.onopen = () => dispatch({ type: 'changed' });
      }
      return () => handlers.delete(handler);
    },
  });
})();
```

The stream opens at the first `watane.on`. A page that never calls
`watane.on` gets no `reload`.

## 9. Events

### 9.1 Emission

A call to an outlet function appends `(emitter, outlet interface,
function, values, hop)` to the outbox of the store; `hop` is the chain hop.
The outbox joins the chain effects only when that invoke succeeds (6.6). A
nested call that succeeded keeps its events even if its caller fails, as
it keeps its writes.

In a harness, a successful call returns the outbox as `EventRecord`s with
pkg's versioned `InterfaceName` and named JSON arguments. It sends no bus
messages. A failed call drops its outbox.

### 9.2 Delivery

After a top-level call returns from invoke, and with no mod lock held, the
host delivers the chain's events synchronously, before the top-level call
returns its result:

1. Queue the events in emission order.
2. Pop an event. If `event.hop >= limits.hops`, drop it and log. If
   `limits.deliveries` deliveries already ran for this top-level call, drop
   it and the rest of the queue, and log.
3. Send the event message to the emitter's pages.
4. Subscribers: the enabled mods with a subscription link to (emitter,
   outlet interface) and the `Subscribe` grant, in local-name order.
5. For each: a new chain (fresh fuel and deadline, depth 0, hop =
   `event.hop + 1`), caller `mod:<emitter>`, invoke the subscriber's
   subscription export, the same function name, the same values. On
   success, append its committed events to the queue and send the event
   message to the subscriber's pages. On failure, log
   `watane: event <interface>.<function> from <emitter> to <subscriber> failed: <code>: <message>`
   and go on. No retry.
6. When the queue is empty, send `{"type":"changed"}` to each mod whose
   state changed in the top-level call or any delivery, in local-name
   order.

Order: events of one emitter arrive in emission order. The emitter never
learns its subscribers. The queue is in memory; a crash loses undelivered
events.

### 9.3 Page bus

One `tokio::sync::broadcast` channel per host, capacity 1024, of
`(LocalName, Arc<str>)` messages. Each `/events` stream subscribes and
filters by its local name.

## 10. Tools and MCP

### 10.1 Toolsets

`toolsets.json` maps a toolset name to entries. An entry is
`<local>:<interface>` (every function of that own-package export) or
`<local>:<interface>.<function>`. `set_toolset` checks the grammar only
(`bad_toolset_entry`). The name `preview` is reserved: `set_toolset("preview")`
is `bad_name` in every home. In a preview it is the implicit toolset of
every function of every enabled mod.

At each request, the host resolves the toolset against the current
registry: entries that name no installed, enabled mod or function give no
tool; tools are deduplicated by name and sorted by name.

Tool name: `<local>_<interface>_<function>`, with the short interface name.
A name longer than 64 bytes gives no tool. Description: the WIT doc comment
of the function (`Resolve::push_dir` on `mods/<local>/wit/`, then
`Function::docs.contents`; both exist in wit-parser 0.254), else its
signature `<function>: func(<p>: <type>, ...) -> <type>`. Input schema:
`{"type":"object","properties":{<param>: <schema>},"required":[non-option
params],"additionalProperties":false}`.

### 10.2 Tokens

`issue_token(toolset, handle)`: 32 bytes from `getrandom::fill`, encoded
base64url without padding, with the prefix `wt_` (46 chars). The host
keeps only `blake3(token)` in memory, mapped to (toolset, handle).
`TokenId` is the hex of the first 8 bytes of that hash. A token lives until
`end_token` or until the host stops; tokens are never written to disk. In a
participant home, only the app issues tokens, in its process (W6). In a
preview, the program `watane` issues one token for `preview` with handle
`preview` at start.

### 10.3 Protocol

MCP over the Streamable HTTP transport, stateless, with JSON responses
only (no SSE stream, no `Mcp-Session-Id`).

| Check on `/mcp/<toolset>` | Failure |
|---|---|
| `Origin` header absent | 403 |
| method `POST` | 405, `Allow: POST` (also for `GET` and `DELETE`) |
| `Authorization: Bearer <token>` names a live token | 401, `WWW-Authenticate: Bearer` |
| the token's toolset equals `<toolset>` | 403 |
| `Content-Type: application/json`, body at most 1 MiB | 415 / 413 |
| body is one JSON-RPC 2.0 object (no batch) | 200 with error `-32600` |

A notification (no `id`) gets 202 with no body. A request gets 200 with one
JSON-RPC response:

| Method | Result |
|---|---|
| `initialize` | `{"protocolVersion": v, "capabilities": {"tools": {"listChanged": false}}, "serverInfo": {"name": "watane", "version": "<crate version>"}}`; `v` is the client's version if it is one of `2025-11-25`, `2025-06-18`, `2025-03-26`, `2024-11-05`, else `2025-06-18` |
| `ping` | `{}` |
| `tools/list` | `{"tools": [{"name", "description", "inputSchema"}]}`, all in one page |
| `tools/call` | below |
| other | error `-32601` |

`tools/call` with `{"name", "arguments"}`: a name not in the toolset is
error `-32602` "unknown tool <name>". Else a top-level call with
`Caller::Agent(<token's handle>)` (missing `arguments` is `{}`). Result
`{"content": [{"type": "text", "text": T}], "isError": E}`:
- a `CallError`: `T` = `<code>: <message>`, `E` = true;
- a function that returns `result<A, B>`: `ok` gives the text of `A`,
  `err` gives the text of `B` with `E` = true;
- the text of a JSON string is the string; of any other value its compact
  JSON; of `null` the empty string.

## 11. Local API

On the API host only. Every request: no `Origin` header (403), body at most
64 KiB (these routes only, 8.1), JSON in and out, `Cache-Control: no-store`. Errors:
`{"error": <HostError JSON>}` with the status of section 2.2.

| Method, path | Request | Response 200 |
|---|---|---|
| `GET /api/status` | none | `{"watane":1,"mode":"home"\|"preview","home":"<abs>","port":47341,"app":true}` |
| `GET /api/mods` | none | `{"mods":[InstalledMod, ...]}` |
| `POST /api/plan` | `{"folders":["<abs dir>", ...]}` | the `Plan` JSON (2.4); staging is removed after the answer |
| `POST /api/install` | `{"folders":["<abs dir>", ...]}` | `{"installed":[InstalledMod, ...]}` |

`app` is true when the host has a `Confirm`. A relative folder path is
`bad_request`. `/api/install` runs plan and install under the install mutex
with `Requester::LocalApi`; it waits for the confirmation (at most 600 s).
Released packages do not come over the local API: the app hands them to
`Host::plan` in process. Destructive operations and tokens have no route
(Q7).

## 12. Confirmation and the program `watane`

### 12.1 Confirmation

```rust
pub trait Confirm: Send + Sync + 'static {
    fn install(&self, request: InstallRequest) -> Pin<Box<dyn Future<Output = Answer> + Send>>;
}
pub struct InstallRequest { pub entries: Vec<PlanEntry>, pub requester: Requester }
pub enum Requester { App, LocalApi }
pub enum Answer {
    /// Install; leave these grants ungranted (by plan entry local name).
    Approve { withhold: Vec<(LocalName, Grant)> },
    Deny,
}
```

`Plan::needs_confirmation()` is true in a participant home when an entry
is `New`, or is an `Update` with a non-empty `new_grants`. Then:

| Home | Behaviour |
|---|---|
| preview | never asks; withholds nothing (W5) |
| participant home, no app | `refused` ("no app is attached to confirm the install") |
| participant home, app | calls `install` once with the entries that are not `Unchanged`; `Deny` or no answer in 600 s is `refused` |

A withheld grant that the entry does not declare is ignored. The host asks
nothing for a plan that needs no confirmation.

### 12.2 The program

```text
watane [--home <dir>] [--port <n>]
watane --preview <dir> [--port <n>] [--install <folder>]...
watane --version
```

- `--home`: a participant home with no app (`Mode::Home { app: None }`):
  it runs and serves the installed mods; every install that needs a
  confirmation is refused. Default dir `default_home()`, default port
  `DEFAULT_PORT` (47341).
- `--preview`: a preview home; default port 0. Each `--install` folder
  joins one plan, installed before the ready line. Then the host issues the
  preview token.
- `--version` prints `watane <crate version>` and exits 0. A bad argument
  prints an error line with code `usage` and exits 2.

After it listens, the program prints exactly one line on stdout and then
writes only to stderr:

```json
{"watane":1,"mode":"preview","home":"/abs/dir","port":53122,"api":"http://localhost:53122/api/","mcp":"http://localhost:53122/mcp/preview","token":"wt_...","mods":[{"local":"counter","page":"http://counter.localhost:53122/"}]}
```

In a participant home, `mcp` and `token` are `null`. `page` is `null` for a
mod without a page. A failure before the ready line prints
`{"watane":1,"error":<HostError JSON>}` and exits 1. `watane_host::Ready`
(Serialize, Deserialize) is the type of the ready line, for `dev`.

SIGINT or SIGTERM: stop accepting, let running requests end for up to 5 s,
remove `run.json`, exit 0. Log lines on stderr start with `watane: `.

## 13. Test plan

Tests are offline and follow the oracle rule of `AGENTS.md`. The HTTP tests
use `reqwest` with `ClientBuilder::resolve("<name>.localhost", addr)` so the
`Host` header is real.

### 13.1 Fixtures

`tests/fixtures/src/<name>/` holds the guest crates (Rust, `wit-bindgen =
"0.57"`, `crate-type = ["cdylib"]`). `tests/fixtures/build.sh` builds each
with `cargo build --release --target wasm32-wasip2` and copies the result
to `tests/fixtures/mods/<name>/logic.wasm`. The built files are committed,
so `cargo test` needs no wasm toolchain. Each `mods/<name>/` is a mod
folder in pkg's format, package `mods:<name>@0.1.0`.

| Fixture | World |
|---|---|
| `counter` | import state and caller; import own-package outlet `changes { counted: func(count: u32); }`, called by `add`; export `api { add: func() -> u32; add-then-fail: func(); get: func() -> u32; who: func() -> string; }`; `ui/index.html`, `ui/app.js` |
| `echo` | import state; export `api` with identity functions for bool, each integer width, f32, f64, char, string, list, tuple, record, variant, enum, flags, option and result (with and without payloads); `opt-opt: func(x: option<option<u32>>)`, `put`, `read`, `delete`, `put-twice` for state limits and isolation, `nan` and `infinity` for float output |
| `hog` | export `api { spin: func(); grow: func(pages: u32); probe: func() -> list<string>; ping: func() -> string; fresh: func() -> u32; }`; `ping` checks recovery after a limit; `fresh` checks that instances share no globals |
| `shelf` | import state, caller, outlet `events { added: func(title: string); }`; export `items { add: func(title: string); add-then-fail: func(title: string); all: func() -> list<string>; who: func() -> string; }` |
| `recs` | `deps.json` `{"mods:shelf": null}`; import state, `mods:shelf/items`, outlet `events { suggested: func(title: string); }`; export `mods:shelf/events` (subscription, stores each title), `api { suggest: func() -> string; received: func() -> list<string>; ping: func() -> string; who-of-shelf: func() -> string; }` |
| `loop-a`, `loop-b` | each: outlet `events { ping: func(n: u32); }`, subscribes to the other's `events`, counts receipts in state and re-emits; `loop-a` exports `api { start: func(); count: func() -> u32; }`, `loop-b` exports `api { count: func() -> u32; }` |
| `cyc-a`, `cyc-b` | each imports an interface that the other exports |
| `net` | `[http] hosts = ["example.org"]`; export `api { get: func(url: string) -> result<u16, string>; }` |
| `empty` | `mod.toml` and an empty world; no part |

The mutual fixtures copy only provider interfaces and outlets into
dependency worlds, without back-references (Q23). Their real components
run on the preview install path.

### 13.2 Tests

| File | Checks | Oracle |
|---|---|---|
| `src/home.rs` unit | local name grammar and derivation | a literal table: `notes` ok; `a`, 24 bytes ok; 25 bytes, `-a`, `a-`, `a--b`, `A`, `a_b`, `2x`, `é` refused; slug `reading-list` → `reading-list`; a 40-byte slug → its first 21 bytes; taken `notes` → `notes-2` |
| `src/runtime/json.rs` unit | each row of 6.9, both ways, and the error paths | literal JSON and literal `Val` trees |
| `tests/json.rs` | `echo` round trips through `Host::call` | literal JSON in and out; `opt-opt` is `no_function` |
| `tests/harness.rs` | S16 fake imports and conversion errors; subscription deliver and call separation; event records, independent homes, rollback, named and positional args, cleanup, invalid folders | literal values and events; directory absence after drop |
| `tests/runtime.rs` | FUNCTIONAL S4 to S9; depth with `Limits { depth: 1 }` over `recs` → `shelf` → ok and depth 0 → `limit`/`depth`; state isolation (`counter` and `shelf` with the same key) | literal values; after `drop(host)` the test opens `state/state.redb` itself and reads `s/<id>` with `id` computed from the literal identity string |
| `tests/events.rs` | S24, S26; order: `add("A")`, `add("B")`, `add("C")` → `received` = `["A","B","C"]`; a failed call emits nothing | literal lists and counts |
| `tests/pages.rs` | S11, S12, S13; a page answers alike over `127.0.0.1` and `[::1]`; path refusals (`/../x`, `/a//b`, `/.env`, `/%2e%2e/x`, `/a\b`); unknown `Host` → 421; `/watane.js` bytes equal `src/watane.js` read from disk; a 404 has `text/plain` | status codes, literal header strings, file bytes |
| `tests/mcp.rs` | S14, S15; `tools/call` output for ok, err and a trap; batch → `-32600`; notification → 202; `initialize` version choice | literal JSON-RPC responses |
| `tests/catalog.rs` | S16 to S23, S25, S28, S29; the pinned publisher (a provider of another publisher does not link); an update keeps a withdrawn grant withdrawn | a `Confirm` fake that counts calls and answers from a script; directory listings of `mods/`; `installed.json` parsed as plain JSON |
| `tests/api.rs` | the four endpoints; `Origin` → 403; S22; S30 | status codes, literal JSON |
| `tests/program.rs` | S1, S2, S3 through `env!("CARGO_BIN_EXE_watane")`; SIGTERM removes `run.json`; `--bad` exits 2 | the stdout line parsed as plain JSON; exit codes; file presence |
| `tests/hostile.rs` | `/call` with a subscription function → 404 (step 2); a body field `caller` → 400; `/call` with the origin of another mod → 403; `/api/...` with any `Origin` → 403; `Host: evil.example:<port>` → 421; a token of toolset `a` on `/mcp/b` → 403; a 2 MiB `/call` body → 413; `counter` without storage grant cannot read state that it wrote before the revoke (step 2); exact and repeated Host checks on both loopbacks; preflight without CORS; streamed and declared oversized bodies; mod/API route isolation; raw path tricks; cross-origin events; S11 to S13 through the program | status codes, literal headers and JSON, fixture page bytes; every step-1 probe uses `Preview` |
| `tests/net.rs` | S27: `https://example.com/` and `http://example.org/` fail without a socket; `https://example.org/` succeeds (`#[ignore]`, needs network) | literal error code |
| `tests/verify.rs` (stage 8) | S31, S32 | a `watane_log::Log` on a temporary directory signs the body with `append(RELEASE_KIND, ..)`; the expected `ReleaseRef` text is built from `Log::identity()` and literals; bytes are changed by the test |

## 14. Security properties

| Property | Enforced by | Tests |
|---|---|---|
| A logic part has no device access | empty `WasiCtx` (6.5); import check (5.3) | S9, S17 |
| Each call ends | fuel shared by the chain; epoch deadline; memory limiter; depth; state limits (6.2) | S6 to S8, runtime |
| An ungranted import never reaches a provider | link and grant read from the snapshot at each nested call (6.6); stub otherwise | S25 |
| The caller comes from the transport | `Caller` set by the entry point; `mod:` only by the linker; `/call` refuses unknown fields | S10, hostile |
| Only the host calls a subscription | `/call` and tools exclude subscription exports (6.6 step 2) | hostile |
| A page reaches only its own mod | one origin per mod; exact `Origin` on `/call`; CSP; COOP and CORP (8.2, 8.3) | S11, S12 |
| No DNS rebinding | exact `Host` check (8.1) | hostile |
| Only this machine reaches the server | binds only `127.0.0.1` and `::1`, never a wildcard address (8.1) | pages |
| The local API has no authority | `Origin` refused; install needs the app in a home; no destructive route (11, 12.1) | S21, S22, hostile |
| An agent reaches only its toolset | 256-bit tokens, stored as hashes, bound to toolset and handle; `Origin` refused (10.2, 10.3) | S15 |
| A tool name names its mod | local-name prefix; `_` in no local name and no WIT id (3.2) | S14 |
| A release is what its publisher signed | `Entry` and `Package` exist only after their checked decoders; the checks of 5.2 | S31, S32 |
| A provider is the pinned one | publisher equality in linking (4.2 step 7) | catalog |
| State is private | one table per identity; host functions touch only the calling mod's table (7) | runtime |
| A network grant is narrow | HTTPS only, exact host list (6.8) | S27 |
| A preview never holds participant data | mode fixed in `home.json`; one host per home (3.1) | S2, S3 |
| No path escapes `ui/` | path rules and canonical prefix (8.3) | pages |

Not protected (spec §6): another program of the same user, which can read
the home and call `/call` or `/events` with a forged `Origin`; a
compromised OS; a fault in Wasmtime.

## 15. Build plan

### S1 page-only cut

The first slice builds only preview homes, folder installation, static pages,
the status and install routes, and the program. Logic parts are refused with
`invalid_mod` and "logic parts arrive in a later step". There is no state,
toolset, token, MCP or home mode. The ready line has null `mcp` and `token`.
Until runtime limits and modes join, `Host::open(home: &Path)` opens a preview,
and `Host::serve(port: u16)` serves it. The full signatures in 2.1 join with
their stages. The fixed view types in 2.1 and 2.4 already keep their full fields.
`Host::install_folders(&[PathBuf], port: u16)` is the page-only catalog
transaction used by the program and install route. `Served::ready()` builds
the fixed Ready value from the running server and catalog.
The program accepts `--preview`, `--port`, repeated `--install`, and
`--version`; other arguments, including no arguments, are `usage` errors.

Install uses `pack(dir, false)` and `package.unpack(staging)` as in 4.2.
The package id is the canonical `package.id()`. Recovery compares installed
folders with `pack(dir, false)?.id()` before restoring committed `.old` bytes.
Grants and links are empty. Equal package ids do not write
the catalog. The only dependencies are pkg, axum, tokio (including signals),
serde, serde_json, thiserror, getrandom, and dev reqwest and tempfile.
S1 fixtures are plain page folders. The Wasm fixture build script of 13.1
joins with runtime fixtures; S1 requires build, test, clippy and fmt checks.

Startup holds the lock before reading or creating the home marker. `run.json`
is written atomically once the server listens; the `--install` folders are
installed after that, and the ready line follows them. So `run.json` signals a
listening server, and only the ready line signals that the initial mods are in. On recovery, restore a `.old` directory when it matches the
committed catalog package and the current directory does not; then remove
uncommitted directories. This preserves the commit point of 4.3.

### S2 cut

`Host::open(home: &Path, limits: Limits)` still opens only a preview.
Runtime core runs top-level calls with `Caller::Page`. Install stages
`logic.wasm` through pkg pack/unpack, checks the staged
component with pkg, and compiles it before the catalog commit. Open compiles
installed components again. S11 extends this path with preview links and
keeps the unpacked world check before compilation.
Preview grants each declared grant. The state import's Storage grant check
joins with revoke; FUNCTIONAL S5 waits for that step. Undeclared imports
fail installation.
The engine also sets `wasm_threads(false)`: Wasmtime's resource limiter
cannot bound shared memories. No accepted linear memory bypasses the limiter.
The added runtime dependencies are wasmtime and wasmtime-wasi 48.0.8,
redb 4.5 for state, and blake3 1.8 for state identity keys.
State opens lazily on the first logic call, or at open when its database
already exists, so an empty page-only home needs no state directory.

### S3 cut

Outlets queue named JSON arguments in the call's outbox. A successful state
commit publishes the events to the emitter's pages, then `changed` if bytes
changed. S11 serializes top-level calls through delivery and publication,
with no mod lock held while sending, so commits keep their order. The host
owns one broadcast channel with capacity 1024. `/events` filters by local
name, recovers from lag with `changed`, pings after 15 idle seconds, and ends
on server shutdown. Updates with a different package id keep identity and
local name, and send `reload` after the catalog commit and registry swap.
The SSE adapter uses `tokio-stream` 0.1 with `sync`: Axum requires a Stream
and does not re-export the trait; the wrappers supply it and channel polling.
S11 adds subscribers, links and hop and delivery counting. Lifecycle
operations still wait for participant catalog stages.

### S4 cut

Stage 5's Harness part uses the existing preview open and folder install
path, runs page calls without a server, and returns their values and emitted
events. S11 links fakes and adds `Harness::deliver` for subscriptions.
`FunctionInfo` joins with toolsets in S5; `Harness::functions` stays deferred.
Harness homes use `getrandom`;
`tempfile` stays a dev-dependency. Missing Origin errors name the absent header.

### S5 cut

Stage 4 adds `functions`, `set_toolset`, `issue_token`, `end_token`, and MCP
on the API host at `/mcp/<toolset>`. MCP has its own 1 MiB body limit;
the local API's 64 KiB limit does not apply. The preview program issues
one token with handle `preview` after installation and prints its MCP URL
and token in the existing Ready fields. Tokens stay in memory as hashes.
`remove_toolset`, `toolsets`, and `tokens` wait for the app in step 2.
`Harness::functions` remains deferred. The existing preview-only open and
serve signatures stay until participant homes arrive. `wit-parser` 0.254.2
reads only doc comments; compiled types supply function metadata and schemas.

### S9 cut

`Mode` is `pub enum Mode { Home, Preview }`. `Host::open(home, mode, limits)`
opens either kind of home and checks its fixed marker before recovery.
`Mode::Home` has no `app` field until confirmation joins in step 2. It always
behaves as no app: status reports `app: false`, and an install that needs
confirmation returns `Refused("no app is attached to confirm the install")`.
The program defaults to `default_home()` and `DEFAULT_PORT` (47341) in home
mode. Preview still defaults to port 0 and alone accepts `--install` and
prints an MCP address and token. Home ready lines have null MCP and token.

### S11 cut

Stage 2 now runs on preview folder installs. Preview links every declared
call and subscription to a compatible local provider, rejects missing
providers and call cycles, and persists links in provider-first order.
Subscription cycles are allowed; their order breaks ties by local name.
Calls share fuel and deadline across nested providers. Event delivery runs
without mod locks, with fresh invocation budgets and shared hop and delivery
limits. Top-level calls serialize through a deadline-bounded execution lock
to preserve commit and page publication order across event deliveries.
This ends the S2 link refusal, S3 subscriber deferral and S4 fake/deliver cuts.
Harness imports use fakes and subscriptions use `deliver`; Harness functions,
participant confirmation and revoke remain deferred.

| Stage | Build | Needs | Until it exists | Done when |
|---|---|---|---|---|
| 1 Runtime core | engine, limits, epoch ticker, `Ctx`, state store, `watane:host` interfaces, JSON encoding, top-level call; fixtures `counter`, `echo`, `hog` | none | load a mod through a crate-private `RawMod { local, identity, package, component bytes, grants, links }` built by the test | `tests/json.rs`, S4 to S9 pass |
| 2 Links and events | nested calls, stubs, outlets, delivery, hops, page bus; fixtures `shelf`, `recs`, `loop-*` | none | `RawMod` with hand-written links | S10 (through `recs`), S24 to S26 pass on `RawMod` |
| 3 Server and pages | routing, `/call`, `/events`, files, CSP, `watane.js` | none | `RawMod` | `tests/pages.rs`, page rows of `tests/hostile.rs` pass |
| 4 Tools | toolsets, tokens, MCP | none | `RawMod` | `tests/mcp.rs` passes |
| 5 Catalog and home | home layout, lock, `installed.json`, plan, install, update, remove, grants, confirmation, import check, local API, `Harness` | `pkg`: section 17 without `Release` | write the catalog logic against `RawMod` first; do not write a second `mod.toml` or WIT reader; wire `Mod::read` in when pkg lands | S16 to S23, S25, S28 to S30 pass |
| 6 Program | `watane` binary, ready line, `run.json`, signals | stage 5 | none | S1 to S3 pass; `dev`'s `serve` drives a preview (J1 with `dev`) |
| 7 Network grant | wasi:http hooks; fixture `net` | stage 5 | none | `tests/net.rs` passes |
| 8 Releases | `Source::Release`, section 5.2 | `log`: `Entry`, `Log` (tests); `pkg`: `Release` | none; the stage waits | S31, S32 pass; J3 with `dist` and `app` |

Step 2 is done when J2 passes with `app`. Steps 4 and 5 need only
HOST-F-42 and HOST-F-43, which stages 5 and 8 meet.

## 16. Provides to other subsystems

| To | Item |
|---|---|
| `app` | `Host::open` with `Mode::Home { app: Some(..) }`; the `Confirm` trait, `InstallRequest`, `Answer`, `Requester`; `plan`, `install`, `mods`, `get`, `enable`, `disable`, `remove`, `grant`, `revoke`; toolsets and tokens; `serve(port, extra_api)`, `Served::{page_url, mcp_url}`; `PlanEntry`, `InstalledMod`, `MissingProvider`; `default_home`; `DEFAULT_PORT`; the re-exports `watane_pkg`, `watane_log` (no direct dependency needed) |
| `app` | Existing items, nothing added: toolset UI = `mods()` + `functions(local)` (no subscription export), entries `<local>:<interface>.<function>`; give back a withheld grant = app's consent view, then `grant` (4.4); "replaces" = `Change::Update`; "derived from" = `InstalledMod.origin` |
| `app` | the page origin `http://<local>.localhost:<port>/` to load as a top-level view, on `127.0.0.1` and `::1` (8.1); CSP `frame-ancestors 'none'`; close a mod's view on disable or remove; `<home>/app/` reserved (3.1); `extra_api` routes set their own body limit (8.1) |
| `dist` (through `app`) | `Source::Release { package: Package, entry: Entry }` for `plan`, as `Fetched` gives them; `MissingProvider.release` (a `ReleaseRef`) to fetch; `InstalledMod.release` (a `ReleaseRef`) to compare with updates and withdrawals; `<home>/dist/` and `<home>/log/` reserved (3.1) |
| `dev` | the program `watane` (12.2), `Ready`, `RunInfo::read`, the local API (11), `default_home`; `Harness`, `Fakes`, `Outcome`, `EventRecord` (2.6; `FunctionInfo` joins S5); `HOST_WIT` for `wit/deps/watane-host/`; `WATANE_JS`; the JSON encoding (6.9); the CSP rule that a page has no inline script (the template must load its script from a file). The S11 cut lists deferred Harness items. |

## 17. Needs from other subsystems

From `pkg`, as `../pkg/TECHNICAL.md` sections 3 to 8 give it; the host
uses nothing else:

```rust
Slug, Version, InterfaceName /* namespace() package() interface() version() satisfied_by() */,
PackageId, PublisherKey /* from_bytes([u8; 32]) */, ReleaseRef, Provider, Grant /* same_authority() */;
// each: Display, serde (text, or pkg's form for Provider and Grant), Clone, Eq, Ord, Hash
Mod::read(dir) -> Result<Mod, ModError>;  mod.check_component() -> Result<(), ModError>;
mod.{slug(), version(), name(), summary(), http_hosts(), origin() /* Option<&ReleaseRef> */, world(), grants()};
mod.logic() -> Option<PathBuf>; mod.page() -> Option<PathBuf>;   // a part exists when Some
World { exports, events, calls, subscriptions: Vec<InterfaceName>, storage: bool, http: bool }
pack(dir, false) -> Result<Package, PackageError>;
package.{id() -> PackageId, unpack(dest) -> Result<Mod, PackageError>};
RELEASE_KIND; Release::decode(&[u8]) -> Result<Release, ReleaseError>;
release.check(&Mod, &Package) -> Result<(), ReleaseError>;   // Mismatch { field }
```

Errors are shown by their `Display` text. `Grant::same_authority` is
"(step 4)" in pkg; the host needs it from step 1 (Q8).

From `log` (stage 8, its section 2): `Entry` (built only by `Entry::decode`
or `Log::append`) with `author()`, `kind()`, `body()`; `PublicKey::as_bytes()`,
whose `Display` is pkg's `PublisherKey` text; `Log::{open, append, identity}`
in tests (13.2).

From `app`: an implementation of `Confirm`, and routes for `dist` under
`/api/`. From `dist`: nothing directly.

## 18. Questions for spec/

Each answer below is used in this file.

- **Q1** spec §2 says "`host` never fetches". A network grant makes the
  host carry a mod's HTTPS requests. Proposal: the rule means that the host
  never fetches channels or packages; a mod's granted request is the mod's
  authority.
- **Q2** spec §3 says the private state uses the local name. The host keys
  state by identity, so that kept data reattaches on a reinstall and never
  reaches a new mod that reuses a local name.
- **Q3** spec §4 "the writes of a call commit only when the call
  succeeds" is unclear for nested calls. Proposal: each mod's writes commit
  when its own call succeeds; a provider's write stays if its consumer
  fails later. Commit of a whole chain needs locks held to the end, which
  can deadlock.
- **Q4** spec §4 `watane:host/state` has no delete, so a mod cannot free
  its quota. Proposal: add `delete: func(key: string)` (section 6.4).
- **Q5** W9 and J2 ("the recommendations mod reports the missing grant").
  An ungranted call traps; the page reports `not_granted`. Catching it
  would need a result type on every imported function.
- **Q6** spec §6 "a token for each run". Proposal: a token lives until
  the app ends it or the host stops; it is kept in memory only.
- **Q7** spec §5 lists install, publish, search and fetch for the local
  API. The host adds status, mods, plan and install; app adds the others
  (`extra_api`). Remove, disable, grant and tokens stay in-process.
- **Q8** F-INST-7 is marked step 4, but J1 (a change in the preview) and a
  second `watane-dev install` need it now. Proposal: the rule holds from
  step 1 for every update.
- **Q9** (decided: D-A1, D-A2) Identity is pkg's `Provider` text; a local
  mod is `local/<slug>`, so a second folder with one slug is an update. A
  release is named by its `ReleaseRef`, never by a log entry id.
- **Q10** W8 needs the app's web view to resolve `<local>.localhost` to
  the loopback address. macOS resolves it to `::1` first, so the server
  listens on both (8.1). Verified for WKWebView at app stage A (it loads
  over `::1`, given the ATS exception of app 12); WebKitGTK not yet.
- **Q11** Two mods that use each other need each other's WIT in
  `wit/deps/`. If `wit-parser` refuses that cycle, the cycle check (4.2
  step 8) and the hop limit stay as defence. Confirm at pkg stage 3.
- **Q12 (host, dev, pkg)** S8 replaces S1/S2 folder copying and the temporary
  digest with `pack(dir, false)`, `Package::id` and `Package::unpack`.
  Host and dev use the same canonical package id. Recovery also uses pack.
  The preview has no token; dev consumes the fixed Ready and RunInfo types.
- **Q13 (host)** Recovery in 3.1 must restore committed `.old` bytes before
  deleting leftovers; otherwise an interrupted update can lose the old mod.
- **Q14 (host)** Hold the home lock before marker initialization, so two
  simultaneous starts cannot race to create `home.json`.

- **Q15 (host, dev)** S2 adds the runtime cut above. Page-only homes create state only when logic first runs; S3 adds outlet delivery to pages.

- **Q16 (host)** Wasmtime 48 runs canonical post-return inside `Func::call`; its deprecated `post_return` method has no effect. S2 relies on the automatic cleanup.

- **Q17 (host)** Disable Wasm threads in the S2 engine. Wasmtime 48 does not invoke `ResourceLimiter` for shared memories, so accepting them would bypass the memory bound of section 14.

- **Q18 (host)** S3 uses a call-local JSON outbox and a shared page bus; subscribers and catalog lifecycle operations wait for links. State commit returns whether stored bytes changed, and bus publication preserves commit order.

- **Q19 (host, dev)** S4 fixes the Harness API to open/call with recorded events and temporary-home cleanup. Fakes stay unlinked, deliver waits for links, and functions joins S5; section 2.6 and the S4 cut define the available API. Missing `/call` Origin errors name the missing header without changing the origin check.

- **Q20 (host, dev)** S5 serves stateless MCP on the API host, reserves preview in every home, and defers app toolset management and Harness functions. The lockfile resolves wit-parser to 0.254.2.

- **Q23 (host, dev)** S11 renames the invalid keyword fixture `list` to
  `shelf` throughout its calls, subscriptions and fakes. Mutual fixture
  dependency WIT includes only the provider's offered interfaces and outlets,
  with no back-reference in the copied world. Subscription cycles do not
  constrain install order; call links do. Short subscription names that
  match multiple providers are `no_function` in the harness. The harness
  uses the preview staging and compile path with unbound providers and
  fake imports.
