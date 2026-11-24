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
wasmtime = "48.0.10"                          # LTS line; default features
wasmtime-wasi = "48.0.10"
wasmtime-wasi-http = "48.0.10"
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
# step 11 (19): A2 adds the tokio feature `process` and a process-group crate; A4 adds
# `sse-stream` and an HTTPS client with no redirects (reqwest, rustls), chosen then.

[features]
test-net = []                                # step 11 (19.9): test routes for https-stream; tests only
test-streams = []                            # step 11 (19.9): test source, held consumer, clock and writer stats for streams; tests only (A1)

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
| `src/catalog.rs` | sources, plan, install, update, remove, grants, `installed.json` (section 4); package and release checks (5.1, 5.2) |
| `src/manage.rs` | the manager's functions, request table, manage effects (6.10) |
| `src/runtime/mod.rs` | engine, registry snapshot, the import check (sections 5.3, 6) |
| `src/runtime/call.rs` | the call algorithm (6.6), event delivery (9.2) |
| `src/runtime/linker.rs` | host interfaces, links, stubs, outlets, fakes |
| `src/runtime/limits.rs` | `Limits`, epoch ticker, limiter, error classification |
| `src/runtime/json.rs` | JSON to WIT values, JSON Schema, WIT type text |
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
| `wit/host.wit` | the `watane:host` package (section 6.4; step 11: 19.1) |
| `src/agent/mod.rs` | the host functions of `streams` and `timers` at the call (19.2, 19.6), the effects of a chain and their application after its commit |
| `src/agent/streams.rs` | H1: stream table, FIFO per consumer, batches, backpressure, ends (19.2) |
| `src/agent/claude/` (`launch.rs`, `run.rs`, `control.rs`, `init.rs`, `mod.rs`) and `src/agent/accounts.rs` | H2: launch parsing, flags, the process, stdin and stdout, `initialize` and `init` checks (19.3) |
| planned `src/agent/https_stream.rs` | H3: one request, SSE splitting, idle timeout, account binding (19.4) |
| `src/agent/timers.rs` | H7: timers per mod in host state (19.6); H5 effects are in `src/runtime/call.rs` |
| `src/agent/consent.rs` | consents per origin client in `installed.json` (19.7) |
| `src/agent/tools.rs` | H4: the handles, their toolset consents, tokens and jobs (19.5) |
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
    pub fn plan(&self, sources: Vec<Source>) -> Result<Plan, HostError>;      // plan_with(sources, vec![])
    /// Step 5: a plan in which each bind replaces the pinned provider of one consumer (4.5).
    pub fn plan_with(&self, sources: Vec<Source>, binds: Vec<Bind>) -> Result<Plan, HostError>;
    pub async fn install(&self, plan: Plan, requester: Requester) -> Result<Vec<InstalledMod>, HostError>;
    pub fn mods(&self) -> Vec<InstalledMod>;                       // sorted by local name
    pub fn get(&self, local: &LocalName) -> Option<InstalledMod>;
    pub fn enable(&self, local: &LocalName) -> Result<(), HostError>;
    pub fn disable(&self, local: &LocalName) -> Result<(), HostError>;
    pub fn remove(&self, local: &LocalName, delete_data: bool) -> Result<(), HostError>;
    pub fn grant(&self, local: &LocalName, grant: &Grant) -> Result<(), HostError>;   // only a declared grant
    pub fn revoke(&self, local: &LocalName, grant: &Grant) -> Result<(), HostError>;
    // step 11 (19.4, 19.7, 19.9)
    pub fn withdraw_consent(&self, local: &LocalName, consent: &Consent) -> Result<(), HostError>;
    pub fn account_changed(&self, account: &Account);
    pub fn set_fake_claude(&self, program: &Path) -> Result<(), HostError>;   // a preview only
    #[cfg(feature = "test-net")]
    pub fn set_test_net(&self, net: TestNet);
    #[cfg(feature = "test-streams")]   // 19.9, stage A1
    pub fn test_stream(&self, consumer: TestConsumer) -> Result<TestStream, StreamError>;
    #[cfg(feature = "test-streams")]
    pub fn set_test_clock(&self, clock: TestClock);
    #[cfg(feature = "test-streams")]
    pub fn test_stats(&self) -> TestStats;

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
pub fn judge_origin(text: &str) -> Result<String, HostError>; // step 11 (19.4)
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

`call_async` returns only after its worker releases every host reference,
including those of the event deliveries it waits for. Within the shutdown
bound of 12.2 (5 s in all: routers get 4 s, then connections, requests and
blocking work get the rest), `Served::shutdown` waits for them to release
their host references. If they finish within that bound, dropping the last
caller handle releases the home synchronously. If the bound expires, shutdown returns anyway; running work can still hold
the home until it ends.

### 2.2 Errors

```rust
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    BadRequest(String),                                    // "bad_request"
    Busy(PathBuf),                                         // "busy"
    ModeMismatch { path: PathBuf, found: &'static str },   // "mode_mismatch"
    BadHome { path: PathBuf, reason: String },             // "bad_home"
    InvalidMod { input: String, reason: String },          // "invalid_mod"; input: folder path or slug; reason: pkg's error text
    Verify { check: VerifyCheck, reason: String },         // "verify"
    MissingProviders(Vec<MissingProvider>),                // "missing_providers"
    Cycle(Vec<LocalName>),                                 // "cycle"
    AmbiguousProvider { consumer: LocalName, provider: Provider, candidates: Vec<LocalName> }, // "ambiguous_provider"
    // message: "<consumer> needs <provider> and <n> installed or planned mods fit: <c1>, <c2>; pick one to settle it"
    // candidates are distinct, in natural order (digit runs by number, the rest by bytes).
    // A message names a provider's publisher by the 10-character fingerprint (`<fp>/<slug>`,
    // `local/<slug>` for none); MissingProviders reads "<fp>/<slug> at <version> needed by <mod>".
    // The JSON `missing` array keeps the full provider.
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
impl Serialize for HostError   // the JSON form below

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
| `bad_request`, `bad_name`, `not_declared`, `bad_toolset_entry` | 400 |
| `refused` | 403 |
| `no_mod`, `no_toolset` | 404 |
| `busy`, `mode_mismatch`, `missing_providers`, `cycle`, `ambiguous_provider`, `stale_plan` | 409 |
| `invalid_mod`, `verify` | 422 |
| `bad_home`, `port_in_use`, `already_serving`, `internal` | 500 |

The local API has one more code, `not_found` (404), for an unknown API route
(8.1). It is not a `HostError`: the transport layer answers it before any
handler runs.

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
// `watane:host/caller.who` returns "page", "agent:<handle>", "mod:<local>" (and "host" for the
// host's own calls, step 11: 19.1); `caller.identity` returns the identity text (19.1)
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
    pub binds: Vec<Bind>,                                    // step 5: the binds in force for this entry (4.5)
}
pub enum Change { New, Update { from_version: Version }, Unchanged }
// JSON: {"kind":"new"} | {"kind":"update","from_version":"0.1.0"} | {"kind":"unchanged"}

// Clone, Debug, PartialEq, Eq, Serialize, Deserialize; re-exported at the crate root.
pub struct Bind {                 // step 5 (4.5); also Deserialize
    pub consumer: String,         // identity of the consumer: "<key>/<slug>" or "local/<slug>"
    pub provider: Provider,       // the provider that deps.json names; pkg's serde form {"slug","publisher"}
    pub to: String,               // identity of the picked mod, same slug
}

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
    #[serde(default)]
    pub binds: Vec<Bind>,         // step 5 (4.5); [] when none
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
/// Step 9: a collect is faked under the mod's own outlet ("mods:menu/menu").
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
pub struct Outcome { pub value: Value, pub events: Vec<EventRecord>, pub manages: Vec<ManageRecord> /* step 10 */,
                     pub requests: Vec<RequestRecord> /* step 11 (19.1) */ }
#[derive(Debug, Clone, PartialEq)]
pub struct EventRecord { pub interface: InterfaceName, pub function: String, pub args: Value /* object by name */ }
/// Step 10: a reduction or request of `watane:host/manage` (6.10), e.g. "disable", {"local":"weather"}.
#[derive(Debug, Clone, PartialEq)]
pub struct ManageRecord { pub function: String, pub args: Value /* object by name */ }
/// Step 11: a request of 19.6 (e.g. "timers.after", {"id":"t","delay-ms":5}); same shape as `ManageRecord`.
#[derive(Debug, Clone, PartialEq)]
pub struct RequestRecord { pub function: String, pub args: Value /* object by name */ }
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

Step 10: the import `watane:host/manage` is faked as 6.10 says, under
the `Fakes` key `watane:host/manage`; its reductions and requests are
recorded in `Outcome.manages`.

Step 9: a collect of the mod under test returns the JSON of its fake as
the whole joined list, converted like a call fake; with no fake it returns
the empty list, because a home with no subscriber answers so too. `deliver`
of a collect function (a subscription export) returns the mod's own answer
in `Outcome.value`.

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
  runs/            step 11 (A2): one folder per `claude` run and `runs/pids`; emptied at open (19.3)
  runs.*.gone/     a former `runs/` moved aside at open, then removed (3.1 step 3)
  scratch/         step 11 (A2): scratch workspaces of sessions (19.3)
  log/ dist/ app/  reserved for `log` (opened only by `dist`), `dist` and `app`
  net/             reserved for `net` (step 8, ../net/TECHNICAL.md 4.5)
```

`host` never reads or writes the four reserved names, and never creates
another name. A preview home has none of them (W5); it does have `state/`,
which `Host::open` creates (3.1 step 6).

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
   `mods/*.old`. Step 11 (A2): kill the process groups recorded in
   `runs/pids` that are still this home's runs, then delete `runs/` (19.3).
   A record is a pgid with the boot id and the process's start time (Linux:
   `/proc/sys/kernel/random/boot_id` and field 22 of `/proc/<pid>/stat`;
   macOS: `sysctl -n kern.boottime` and `ps -o lstart= -p <pid>`, both run in
   the C locale, since `kp_proc.p_starttime` needs `unsafe`; `lstart` has a
   one-second resolution, so a pid reused within the same second as the
   recorded start is the limit of the check there).
   A group is killed only when the boot id matches and its leader still has
   that start time; a record without them, or with no live leader, is
   dropped unkilled (S100-CAP, item 5). A record is validated before any process
   handle is opened: a pgid of at least 2 and a start time and boot id that parse
   (Linux: a boot id of the form of `/proc/sys/kernel/random/boot_id`, a UUID;
   macOS: a non-empty token); anything else is discarded and logged, and the
   cleanup goes on (review r7, F7.2). `runs/pids` is read as bytes and split
   into lines; each line is parsed into a full record on its own, so a line that
   is not UTF-8 or fails to parse is logged and skipped and the next line is
   still handled; `runs/` is removed only after every line was handled (review
   r8, F8.1 and F8.2; the deputy's ruling). `runs` is opened as a directory
   without following a link (`O_DIRECTORY|O_NOFOLLOW`), and `pids` is opened
   through `openat` on that descriptor, without following a link and without
   blocking (`O_NOFOLLOW|O_NONBLOCK`); a `runs` that is a link, or not a
   directory, is moved aside unread (review r9, F9.1). `pids` is read only
   when it is a regular file, at most 64 KiB; anything else, or the part past
   64 KiB, is dropped with one log line. A bad line is logged as at most its
   first 64 bytes, and at most 16 bad lines are logged at one open; one more
   line gives the count of the rest (adversary S100 pass 13). The 16 lines are
   shared by every log line of the cleanup (bad lines, leftovers, a read error),
   and a repeated record (the same parsed pgid, boot id and start time) is
   handled once. After the records, `runs` (whatever it is) is renamed to
   `runs.<random>.gone`, so `runs/` is free at once whatever it holds, and each
   `runs.*.gone` in the home is removed: a directory with
   `std::fs::remove_dir_all`, which does not follow a link inside the tree,
   anything else with `remove_file`. The time this takes is in step with what the
   same user put in the private home (accepted by the deputy, review r9, F9.2); a
   removal that fails is logged once and tried again at the next open
   (adversary S100 passes 14 and 15). A leader is
   verified as a leader: on Linux, with the pidfd open, field 5 (`pgrp`) of
   `/proc/<pid>/stat` must equal the pid, read with the start time. The check and the signal are one step
   (review r6, F6.3; the deputy's ruling): on Linux the host opens a pidfd of
   the leader, checks the start time and boot id with it open, and sends
   SIGKILL to that exact process through the pidfd (`pidfd_send_signal`), so
   the signal can reach no other process. An open pidfd does not keep the pid
   number from being reused once the process is reaped, so the host does not
   signal the group by its number at open: other members of a crashed run's
   group are left and logged. Where no pidfd exists (macOS, where `lstart`
   also resolves to a second), nothing is signalled at open: the record is
   dropped and the leftover is logged. A leftover process is a lesser harm than
   a signal to an unrelated group. A record is
   identified by its whole line (pgid, boot id, start time); a run removes
   only its own line (review r6, F6.4).
4. Read each installed mod with `Mod::read`, compile its `logic.wasm` when
   `logic()` is `Some`, and build the registry (section 6.4). A mod that fails to
   load keeps its record and gets `error`; the host still opens.
5. Start the epoch ticker (section 6.3).
6. Step 11 (A2): raise `generation` in the `host` table of state by one
   (19.1, 7). This creates `state/state.redb` at every open, a preview's too.

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
      "consents": [{"kind": "workspace", "folder": "/Users/a/code"}],
      "links": [{"kind": "call", "interface": "mods:shelf/items@0.1.0",
                 "provider": "shelf", "provider_interface": "mods:shelf/items@0.1.0"}],
      "binds": [{"consumer": "local/recs", "provider": {"slug": "shelf", "publisher": "<key>"},
                 "to": "<other key>/shelf"}]
    }
  ]
}
```

`release` is the `ReleaseRef` text, or `null` for a local mod; the
publisher is the release's. `binds` (step 5, serde default `[]`, so a step 4
file reads unchanged and `watane_installed` stays 1) lists the mod's binds
in the JSON form of `Bind` (2.4), the same in `/api/plan` and `/api/mods`. Recovery checks that its publisher and slug
form the saved identity. It repacks release folders with source included,
so a package that shipped source keeps the same hash (Q26). Local folders
still use `pack(dir, false)`. `consents` (step 11, serde default `[]`, so an older file reads unchanged
and `watane_installed` stays 1) lists the consents the mod holds as an
origin client (19.7; omitted from the file while empty): `{"kind":"workspace","via":"<identity>","folder":"<canonical path>"}`,
`{"kind":"account","via":"<identity>","account":"openai"}`, `{"kind":"toolset","toolset":"<name>","functions":["<local>_<interface>_<function>", ...]}` (the functions the participant consented to, 19.5).
They follow the record's life like grants: kept across an update, deleted by
`remove`. Slug, version, name, summary and origin are
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
   `world().events` (an outlet) for `Subscribe`. Step 5: when a bind of
   this consumer names `provider` and its `to` is installed or in the plan,
   the candidates are only the mod with identity `to` (4.5). In a preview
   the publisher is not compared: every mod with that slug and interface
   is a candidate (W27).
   - No candidate: add a `MissingProvider` (one per provider and version;
     `release` = `<publisher>/<slug>@<version of interface>` when the
     publisher is set). Two or more: `ambiguous_provider`. One: a link.
8. Refuse a cycle of call links across the whole catalog (`cycle`, with
   the path, first name repeated at the end). Step 9: a subscription link
   to an outlet in the provider's `world().collects` is an edge from the
   provider (the collector) to the subscriber, because a collect calls
   into the subscriber while the collector's lock is held (6.6). Other
   subscription links are no edges.
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
4. For each entry in order, skipping `Unchanged` and bind-only entries (4.5): rename the staging dir
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

Each serializes its catalog write and registry swap under the catalog
mutex. Install serializes confirmation requests separately. A lifecycle
write invalidates an outstanding plan, including while consent is pending.
Install checks the generation again before committing (Q24).

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
- Step 11 (19): `disable`, `remove`, an update and a `revoke` of `ClaudeCode`
  or `ClaudeCodeShell` end the streams and close the handles that depended on
  the mod or the grant, also those whose origin client it is (`gone`; 19,
  Terms; 19.2, 19.5). A disabled mod's own timers wait and a removed mod's are
  deleted (19.6); another mod's timer that carries it as origin client fires
  with its own mod as origin client. `remove` also rewrites, in the same
  write, the origin of every other mod's timer that carries the removed mod's
  identity to that timer's own mod: a local mod's identity is `local/<slug>`,
  so a later local mod with the same slug has the same identity and must not
  find the removed mod's authority waiting in a timer (S62). `remove`, a
  `grant` of `Manage` and an update whose approval grants `Manage`, drop the names `<local>_…` of that mod (3.2: no `_` in
  a local name) from the `functions` of every stored toolset consent (19.5,
  19.7), in the same write, so that a later mod under that local name, or a
  mod that became a manager, reaches no agent until a new consent (S65).
  `remove` runs in this order: the catalog change, then everything that
  protects authority (the streams end, the timers are rewritten, the state is
  deleted), then the folder; a failure of a later step is reported and undoes
  none of the earlier. An install waits for a `remove` in progress (one lock
  held for the whole removal), so no install sees the name free while a row or
  a stream still carries the removed install.
  `withdraw_consent(local,
  consent)` removes one consent, like `revoke` for a grant, and ends its
  dependent streams and closes its dependent handles (19.7). A consent is
  given only through `Confirm::consent`; the app never writes it.
- Step 10: a manager's `disable` and `withdraw` (6.10) call `disable` and
  `revoke` here, so the same rules hold; it reaches `enable`, `grant` and
  `remove` only through a request that the app decides.

### 4.5 Binds (step 5)

A bind is the participant's pick of a provider (F-BUILD-3, W26). Only the
app makes one: it calls `plan_with` in process after the participant
answered a pick request in the consent view (app 9.7). The local API has no
bind field (section 11), so `/api/install` can never pass one.

`plan_with(sources, binds)` runs 4.2 with these rules:

1. Each bind must name a consumer that is a source of the plan or an
   installed mod, a `provider` that is in that consumer's `deps.json`, and a
   `to` whose slug is `provider.slug`, whose identity differs from the
   pinned identity of `provider`, and that is installed or a source of the
   plan (so a pick always takes effect in the install that confirms it); at
   most one given bind per consumer and `provider`; else `bad_request`
   naming the bind.
2. The binds of an entry are the given binds for its identity, then the
   binds of the installed mod with that identity (an update keeps its
   picks), one per `provider`, given ones first. A bind whose `provider` is
   no longer in `deps.json` is dropped.
3. Step 7 of 4.2 uses the bind while `to` is installed or in the plan.
   The usual check holds: `to` must offer each interface that the consumer
   needs (`satisfied_by`), or the provider stays missing. A bind never
   widens a grant: the consumer's grant keeps the pinned `Provider` as its
   key, and `revoke` and `grant` act on it as before.
4. `remove(to)` deletes every bind whose `to` it is, in the same catalog
   write; the links recompute (4.3 step 5) with the pinned rule, so the
   named provider binds if it is installed. A later install of `to` brings
   no bind back: only a new pick does. `remove` never fails on a link: a
   consumer whose provider now has no candidate or two or more (possible in
   a preview, W27) keeps that link unbound.

`PlanEntry.binds` and `InstalledMod.binds` list the binds in force, so the
consent view shows each one (app 6.2). An entry with the installed package
whose binds differ from the installed ones is `Update { from_version }`
with `from_version` equal to its version (a **bind-only** change), not
`Unchanged`: install step 4 renames nothing for it, and step 6 writes its
new binds. `needs_confirmation` (12.1) is also true for an entry whose
binds differ from the installed ones, so every new bind is confirmed.

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
  `watane:host/manage@0.1.0` only if `world().manage` (step 10);
- an interface in `world().events` whose functions each return nothing
  (an event) or a `list` (a collect, step 9; pkg 4.5);
- an interface in `world().calls`;
- step 11 (19.1): `watane:host/streams`, `claude-types`, `claude-session`,
  `agent-tools`, `timers`, `https-stream`, always; `claude-code` only if
  `world()` holds `Grant::ClaudeCode`, `claude-code-shell` only if it holds
  `Grant::ClaudeCodeShell`; `watane:host/caller` includes `identity`.

Allowed exports: the interfaces in `world().exports` (its calls),
`world().subscriptions`, and the host-only `watane:host/stream-sink`,
`stream-producer` and `timer-sink` (19.1). Anything else is `invalid_mod` with the name. A
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

/// Who made the current call: "page", "agent:<handle>" or "mod:<local name>"
/// ("host" for the host's own calls, step 11).
interface caller {
  who: func() -> string;
  /// Step 11 (19.1): the caller's identity: "<publisher>/<slug>", "local/<slug>",
  /// "page", "agent:<handle-name>" or "host". A pure read, no grant.
  identity: func() -> string;
}

/// Step 10: the management of mods (W35). Importing it is the grant
/// `manage`. Reads and reductions act when the call succeeds; anything
/// more is a request that the participant decides in the consent view.
/// A mod is named by its local name.
interface manage {
  /// A grant as pkg names it. `provider` is "<publisher key>/<slug>" or "local/<slug>".
  record provided { %interface: string, provider: string }
  variant grant { storage, http(string), call(provided), subscribe(provided), manage, claude-code, claude-code-shell }
  record grant-state { grant: grant, granted: bool }
  enum link-kind { call, subscribe }
  /// `provider` is the local name of the linked mod; none while unbound.
  record link { kind: link-kind, %interface: string, provider: option<string>, provider-interface: option<string> }
  /// Identities: the consumer, the provider that deps.json names, the picked mod.
  record bind { consumer: string, provider: string, to: string }
  record mod-info {
    local: string, identity: string, slug: string, name: string, summary: string,
    version: string, publisher: option<string>, release: option<string>, origin: option<string>,
    enabled: bool, has-page: bool, has-logic: bool,
    grants: list<grant-state>, links: list<link>, binds: list<bind>, error: option<string>,
  }
  /// `code`: bad_request, no_mod, not_declared, request_open or refused.
  record failure { code: string, message: string }
  variant request-state { pending, approved, denied, failed(string) }

  /// Every installed mod, by local name.
  mods: func() -> list<mod-info>;
  /// Make things do less, at once.
  disable: func(local: string) -> result<_, failure>;
  withdraw: func(local: string, grant: grant) -> result<_, failure>;
  close-page: func(local: string) -> result<_, failure>;
  /// Open or focus the page of a mod (Q31). Gives no authority.
  open-page: func(local: string) -> result<_, failure>;
  /// Ask the participant. Each returns the id of the request.
  request-install: func(release: string, link: option<string>) -> result<string, failure>;
  request-update: func(local: string) -> result<string, failure>;
  request-enable: func(local: string) -> result<string, failure>;
  request-grant: func(local: string, grant: grant) -> result<string, failure>;
  request-remove: func(local: string, delete-data: bool) -> result<string, failure>;
  /// The state of a request of this mod; none for an unknown id.
  request: func(id: string) -> option<request-state>;
}
```

Step 11 adds the interfaces `streams`, `claude-types`, `claude-code`,
`claude-code-shell`, `claude-session`, `https-stream`, `agent-tools`,
`timers` and the host-only exports `stream-sink`, `stream-producer` and
`timer-sink` to this package, written in 19.1; the `grant` variant above
gains `claude-code` and `claude-code-shell`, as pkg's `Grant` does.

`delete` is a proposal to spec §4 (section 18, Q4). `manage` (step 10,
S73) is 6.10. It joins the package at `0.1.0`: an interface added to a
package that mods already copy changes no import of an existing mod,
and pkg accepts only `watane:host@0.1.0` (pkg 4.7). A mod that imports
`manage` on a host without it is refused at its plan by pkg (rule
`import`), so it never half-runs.

The linker of a mod is built once per registry and kept as an
`InstancePre`. It holds:

| Import | Linked to |
|---|---|
| base WASI | `wasmtime_wasi::p2::add_to_linker_sync`, with the empty context of 6.5 |
| `wasi:http/*` | `wasmtime_wasi_http::p2::add_only_http_to_linker_sync`, hooks of 6.8 |
| `watane:host/state` | the state transaction; each function first checks the `Storage` grant, else `not_granted` ("<local> has no grant for watane:host/state") |
| `watane:host/caller` | the caller string and, step 11, the identity of the caller (19.1) |
| step 11 interfaces (19.1) | `streams`, `claude-session`, `agent-tools`, `timers`: `func_new` of 19.2, 19.3, 19.5, 19.6, no grant check; `claude-code`, `claude-code-shell`: each function first checks its grant, else `not_granted` (the stub of W9); `https-stream`: 19.4; in a `Harness`, the fakes of 19.1 |
| `watane:host/manage` (step 10) | `func_new` for each function of 6.10; each first checks the `Manage` grant, else `not_granted`; in a `Harness`, the fakes of 6.10 |
| outlet | `func_new`: an event function appends the event to the store's outbox (section 9); a collect function runs the collect (9.4) |
| in a `Harness`, a collect | `func_new`: the fake, or the empty list (2.6) |
| call import | `func_new`: nested call (6.6) |
| in a `Harness`, a call import | `func_new`: the fake |

The link and the grant are not baked into the closure. The closure reads
authority from the current catalog at every nested call and storage access.
Event delivery checks current subscriptions. Compiled code stays in the
chain snapshot. A revoked grant cannot authorize a later host import in
an already running chain (Q24).

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
   subscription export, a host-only export (`stream-sink`, `stream-producer`,
   `timer-sink`, 19.1) or a function with a resource type is `no_function`.
3. Convert `args` (section 6.9); a mismatch is `bad_request` with the path.
4. Create the chain context: deadline = now + `time`, fuel = `fuel`,
   depth 0, hop 0, empty effects.
5. Invoke (below). Convert the result to JSON.
6. Apply the chain's requests (19.6) and manage effects (6.10) in order,
   deliver the committed events (section 9), then send the `changed`
   messages. Return the result of step 5. The crate-private harness variant
   returns the result and outbox as `Outcome`, without page bus publication.

Invoke(mod, interface, function, values, ctx), used for top-level and
nested calls:

1. If `ctx.depth > limits.depth`: `limit`/`depth`.
2. Lock the mod's call mutex (one call per mod at a time). The call and
   collect links are acyclic (4.2 step 8), so lock waits follow the link
   order and cannot deadlock.
3. New store; `set_fuel(ctx.fuel)`; deadline callback; instantiate from the
   `InstancePre`; call; `post_return`.
4. On success: read the fuel left; commit the state transaction; append
   the outbox to the chain effects (step 11: its requests and stream
   effects go instead to the outbox of the calling invoke, and reach the
   chain effects only from a succeeded top-level invoke, whose commit also
   writes the chain's timers, 19.6); mark the mod changed if a value
   changed; unlock; return the values and the fuel left.
5. On failure: drop the transaction and the outbox; unlock; return the
   classified error (6.7).

A nested call (the `func_new` of a call import of consumer C):
1. From the current catalog: the link of C for this import must exist and point to
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

### 6.10 Manage (step 10)

The interface `watane:host/manage` (6.4) is the grant `Manage` (W35,
pkg 5.1). A mod that imports it is a **manager**. Its functions follow the
direction of authority: reads and reductions act at once, because a
hostile manager can then only make things do less; every addition is a
**request** that the app decides in the consent view (F-INST-9 to 11).

**Grant check.** Each function first checks, from the current catalog,
that the calling mod holds `Manage` granted and is enabled; else the call
traps with `not_granted` ("<local> has no grant for
watane:host/manage"), as `state` does (6.4). A manager has a logic part:
a page cannot import anything (pkg 4.6).

**Names.** A function names a mod by its local name (3.2) and a grant by
the WIT `grant` variant, which maps one to one to pkg's `Grant`
(`provider` is the identity text of pkg's `Provider`, 4.1). A name that
does not parse is the failure `bad_request`.

| Function | Kind | Does | Failures (`failure.code`) |
|---|---|---|---|
| `mods` | read | the `mod-info` of every installed mod, in local-name order, from `Host::mods` (2.1); the manager itself included | none |
| `disable(local)` | reduction | `Host::disable` (4.4), after `Confirm::reduced` (12.1); a disabled mod stays disabled (ok) | `no_mod` |
| `withdraw(local, grant)` | reduction | `Host::revoke` (4.4), after `Confirm::reduced`; an ungranted grant stays ungranted (ok) | `no_mod`, `not_declared` |
| `close-page(local)` | reduction | `Confirm::reduced` with `ClosePage`: the app closes the mod's window, if one is open; the host itself has nothing to close | `no_mod` |
| `open-page(local)` | direct (Q31) | `Confirm::reduced` with `Reduction::OpenPage`: the app opens or focuses the window `mod-<local>`, and opens none while a consent request is open; the call gives no authority | `no_mod`, `bad_request` (the mod is off or has no page) |
| `request-install(release, link)` | request | asks the app to install the release (`Ask::Install`); a `link` of a channel the home does not read gets a first consent to read it (Q30, app 17.3) | `bad_request`, `request_open`, `refused` |
| `request-update(local)` | request | asks the app to install the update that it offers for the mod (`Ask::Update`) | `no_mod`, `request_open`, `refused` |
| `request-enable(local)` | request | asks to enable a disabled mod (`Ask::Enable`) | `no_mod`, `bad_request` (enabled), `request_open`, `refused` |
| `request-grant(local, grant)` | request | asks to give a declared, ungranted grant (`Ask::Grant`) | `no_mod`, `not_declared`, `bad_request` (granted), `request_open`, `refused` |
| `request-remove(local, delete-data)` | request | asks to remove the mod, and with `delete-data` its state (`Ask::Remove`) | `no_mod`, `request_open`, `refused` |
| `request(id)` | read | the state of a request of this manager; `none` for an id it never got, or that a failed call made | none |

`mod-info` is `InstalledMod` (2.4) field by field: `identity` as is;
`slug`, `version`, `publisher`, `release`, `origin` and the interface
names of `grants` and `links` as their `Display` texts; `has-page` is
true when the mod has `ui/` (`page` may still be `None` before
`serve`); `grant` as in Names above.

`release` is a `ReleaseRef` text; `link` is the link text of the
channel or index that lists it, or `none`. A removal is a request with or
without `delete-data`, and so is `enable`: section 18 Q28 and Q29 hold the
human's choice, and both answers keep this interface.

**When effects happen.** A reduction, `open-page` or a request is not applied
inside the call. Like an event (9.1), it goes to the store's outbox in order,
and joins the chain effects only when the manager's invoke succeeds. A failed
call reduces nothing, opens no page and asks nothing. After the invoke that
started the chain returns (a top-level call, a tool call, or one delivery of
9.2), and
before the chain's events are delivered, the host applies the chain's
manage effects in order. Before each one it checks again that the manager
holds `Manage` and is enabled, and drops the effect if not (Q24's rule).
An effect whose host function fails then (its mod was removed meanwhile)
is dropped and logged; a dropped request ends `denied`.
A read inside the call sees the catalog as it is at that moment, not the
effects queued before it. A `request(id)` for an id that this call queued
returns `pending`.

**Requests.** A request id is 32 lowercase hex characters from 16 random
bytes; the host returns it at once and the request starts when its effect
is applied. In a preview, or a participant home without an app, every
`request-*` fails with `refused` ("a preview has no consent view" or "no
app is attached to decide the request") and queues nothing. Then:

1. At most one request of a manager is pending. A second fails with
   `request_open` ("<local> already has a request open: <id>"), checked
   at the call against pending and queued requests.
2. The host builds `ManageRequest { id, by, ask }`, calls
   `Confirm::manage` when the effect is applied, and polls the future on
   its own thread with a small tokio runtime, so a request needs no
   served host. The future resolves to `ManageOutcome`; the request becomes `approved`, `denied`
   or `failed(<message>)`.
3. When the manager loses `Manage` (withdraw, disable or remove of the
   manager, by anyone), the host drops the futures of its pending
   requests, which removes their consent requests (app 6.1: a dropped
   `ask` leaves the queue); each becomes `denied`.
4. The host keeps the 32 newest finished requests of each manager, in
   memory only. A restart forgets them; a pending request ends `denied`
   with the app (W19).

For an install or an update that the app runs for a request, it calls
`Host::install(plan, Requester::Mod(by))`. With that requester the host
checks at step 1 of 4.3, and again before the commit, that `by` holds
`Manage` and is enabled (else `refused`), and calls `Confirm::install` for
every plan, also one that needs no confirmation (12.1). So an update with
no new grant still reaches the consent view when a manager asked for it.

**The catalog message.** After each catalog write (install, update,
enable, disable, remove, grant, revoke, bind) and each change of a request
state, the host sends `{"type":"catalog"}` (8.4) to the pages of every
enabled mod that holds `Manage` granted. A manager page re-reads through
its own logic.

**Who reaches a manager.** The grant is the manager's, whoever calls it:
its page, an agent with a toolset that names its functions, a mod with a
call grant to it, or a delivery. The host does not narrow this; a manager
reads `watane:host/caller` when it cares (dev's template says so). A
request still needs the participant in the consent view (F-AGENT-4).

The refusal of a preview or of a home without an app comes before every
other check of a `request-*`, so a preview answers `refused` even for an
enabled mod.

**In a harness** (2.6), the manage import is linked to fakes:
`mods` and `request` return the file's fake for
`watane:host/manage`, else `[]` and `pending`; every reduction returns
`ok` (`open-page` too) and every request `ok("r1")`, `ok("r2")`, ... in call order across
the calls of one harness (a failed call takes no number), unless
the file fakes it. Each reduction, `open-page` and request is recorded in
`Outcome.manages` as a `ManageRecord`, in call order; reads are not
recorded. A failed call records none.

## 7. State

One redb database at `state/state.redb`.

| Table | Key | Value |
|---|---|---|
| `s/<id>`, one per mod | `&str` state key | `&[u8]` value |
| `sizes` | `&str` `<id>` | `u64` stored bytes (keys + values) |
| `timers` (step 11) | `&str` `<id>/<timer id>` (`timers/<mod>/<id>` in 19.6) | `&str`: JSON text `{"due":<unix ms>,"origin":"<identity of the origin client>"}`; not counted in `sizes` |
| `host` (step 11, stage A2) | `&str` name; the one key `generation` | `u64`: the start number that `host-info.generation()` returns (19.1); not counted in `sizes`; stage A2 (S99) creates the table |
| `scratch` (step 11, stage A2) | `&str` the 32 hex chars of a scratch workspace id (19.3, Folders) | `&str`: the identity of its owner, the origin client of the launch that made it; not counted in `sizes` |
| `sessions` (step 11, stage A3) | `&str` a Claude Code session id, as `system/init` reports it (19.3, Folders, "Sessions are the origin client's") | `&str`: the identity of the origin client whose run reported it; not counted in `sizes` |

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

Step 11: the `timers.after` and `timers.cancel` of a chain that reach its
top-level invoke (19.6, the chain rule) are written to the `timers` table in
the write transaction of that invoke's commit, also when its own overlay is
empty; so a top-level call's writes and the chain's timers commit whole or
not at all, and a rolled-back call's timers are never written. Every commit keeps redb's default
durability (full: one fsync per write); a request is applied only after its
chain's commits (N4, 19.6).

A put of the stored value is no change, so a page that re-reads on
`changed` cannot loop through a read function that writes the same value.

## 8. Server and pages

### 8.1 Binding and routing

`serve(port, extra_api)` binds `127.0.0.1:<port>` and `[::1]:<port>`, and
serves both with one router (macOS resolves `<x>.localhost` to `::1`
first), and no other address. Port 0: `[::1]` takes the port that
`127.0.0.1:0` got; on a clash, try again, at most 8 times. A failed bind is
`port_in_use`, with one exception: when `[::1]` fails because the machine has
no `::1` (`AddrNotAvailable`, or the OS error `EAFNOSUPPORT`: a kernel
without IPv6, as in a Linux container), `serve` serves `127.0.0.1:<port>`
alone and prints one line on stderr, `watane: no IPv6 loopback (::1); serving
127.0.0.1 only`. Every other `[::1]` failure stays `port_in_use`, and the
`Ready` line does not change. The choice is made once, at bind time: if a
privileged change adds `::1` later, the running host does not bind it
(section 14, not protected). Then it writes `run.json`. A second `serve` is
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
Any other path outside `/api/` and `/mcp/`, and any unknown path under
`/api/`, gets 404 `not_found` ("unknown API route").
A transport refusal (a wrong `Origin`, method, route, type or size) answers
`{"error":{"code":"forbidden"|"not_found"|"bad_request","message":"..."}}`
with its status, before any handler runs.
A 413 refusal does not read the refused body to its end: the server answers,
discards through the body decoder what has already arrived, and closes the
connection when more is still to come. A client that is still writing may
see a reset before it reads the 413; the status is enough for the client to
know that the body was too large, and no unread byte is ever read as a
request (S91 adversary: a request hidden in the overflow is not served).
On an uninstalled mod origin, every path answers 404 `text/plain; charset=utf-8`.
A disabled mod answers 409 `disabled` on a valid `/call`; its pages and
events answer 404.

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
| body is `{"fn": "<interface>.<function>", "args": <object, array or null>}` and, step 11, optionally `"connection": "<32 hex chars>"`; no other field | 400 `bad_request` |

A missing `Origin` gets the message "the Origin header is missing; it must
be http://<local>.localhost:<port>". A wrong, malformed, empty or repeated
`Origin` keeps "Origin must match the mod origin". Both use 403 `forbidden`.

A body-size refusal closes the HTTP/1 connection, so an unread body
cannot affect a later request on that connection.

`connection` is the id of one `/events` connection of this same mod origin
(8.4). The host keeps it as the chain's page connection, never as a caller
field: it makes that connection the consumer of a stream that a mod opens in
the chain (19.2). An id that names no live connection of this origin is
ignored (a stream open then fails `not-yours`), so a reconnect never fails an
ordinary call.

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
- Messages: `{"type":"changed"}`, `{"type":"reload"}`, from step 10
  `{"type":"catalog"}` to the pages of a manager (6.10), and
  `{"type":"event","from":"<emitter local>","interface":"<full outlet name>","function":"<name>","args":{...}}`
  with `args` by parameter name in the encoding of 6.9. A page gets only
  events emitted by its own mod, never events received from another mod.
- Step 11: the first message of each connection is
  `{"type":"connection","id":"<128 random bits as 32 hex chars>"}`, the id
  that `watane.js` sends with `/call` (8.2). A stream whose consumer is the
  connection delivers `{"type":"stream","stream":"<id>","items":[{"seq":<n>,"body":"<text>"}]}`
  (the item bytes as a string, invalid UTF-8 as U+FFFD) and, at its end,
  `{"type":"stream-end","stream":"<id>","outcome":<outcome in the encoding of 6.9>}`.
  These come through the connection's own lossless queue (9.3), never the bus.
- A receiver that lags on the bus (section 9.3) gets one
  `{"type":"changed"}` in place of the lost messages.
- The stream ends when the mod is disabled or removed, or the server stops;
  its streams then end `gone` (19.2). The same holds for one connection: when
  it closes, the streams whose consumer it is end `gone` (19.2, Ends), and
  they are not carried over to a later connection; a page that opens a new
  connection opens its streams again (8.5, Connection replaced).

### 8.5 `watane.js`

Served at `/watane.js` on each mod origin, `text/javascript; charset=utf-8`.
A page loads it with `<script src="/watane.js"></script>`. Exact source:

```js
// watane.js: the page side of a watane mod. It reaches only its own mod.
(() => {
  const handlers = new Set();
  let source = null;
  let connection = null;
  class WataneError extends Error {
    constructor(code, message, limit) {
      super(message);
      this.name = 'WataneError';
      this.code = code;
      this.limit = limit;
    }
  }
  const dispatch = (message) => {
    if (message.type === 'connection') { const previous = connection; connection = message.id; if (previous === null) return; message = { type: 'connection', id: message.id, previous }; }
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
        body: JSON.stringify(connection ? { fn, args: args ?? null, connection } : { fn, args: args ?? null }),
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
`watane.on` gets no `reload` and no connection id. The id is that of the
latest `connection` message; a call made before the first one carries none.

**Connection replaced** (stage A2, S99). An `EventSource` that reconnects opens a
new `/events` connection (8.4) with a new id, and the streams of the old
connection ended `gone` when it closed. The first `connection` message of a
page stays private, as above. A later one also goes to every `watane.on`
handler, after `connection` is set, as `{"type":"connection","id":"<new
id>","previous":"<old id>"}`, so a call made in the handler already carries
the new id, and a page that sees it opens its streams again. The one line of
`dispatch` that does this is the first one of the code above. `tests/it/program/pages.rs`
compares the block with `src/watane.js`, and `tests/it/program/connection.rs`
runs the source under `node` (when it is installed) with a stand-in
`EventSource`.

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
   `event.hop + 1`), caller `mod:<emitter>` (step 11: origin client the
   subscriber, never the emitter; 19, Terms), invoke the subscriber's
   subscription export, the same function name, the same values. On
   success, append its committed events to the queue. Only the emitter's
   pages get each event message; the subscriber's pages do not get the
   provider's event. On failure, log
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

Step 11: stream items for a page do not use the bus, which loses messages
for a lagging receiver. Each `/events` connection has its own lossless
queue (19.2), written to the same response in order with the bus messages;
a batch counts as taken when its write completes. The queue holds at most
16 MiB of items; past it the connection's streams end `lagging` and the
connection ends. A closed connection ends its streams `gone`.

### 9.4 Collect (step 9)

A call by collector C to a collect function of its outlet O runs inside
C's invoke, from the `func_new` of the outlet:

1. Subscribers: from the current catalog, the enabled mods with a
   subscription link to (C, O) and the `Subscribe` grant, in local-name
   order: the set of 9.2 step 4.
2. For each subscriber S: Invoke(S, its subscription export of O, the same
   function, the same values, ctx with depth + 1, caller `mod:<C>`, fuel =
   C's store's fuel left), as a nested call (6.6). Step 11: S and every call
   nested in it run with S as origin client, never C's (19, Terms); C's
   origin client is back when the collect returns.
   - On success: set C's store fuel to the fuel that S left, and append
     S's list to the answer. S's writes commit and its events join the
     chain effects, as for a nested call.
   - On a `limit`/`time` or `limit`/`fuel` error: the budget of the whole
     chain is spent (6.2), so return the error, and C traps with it.
   - On any other error: S's writes and events are dropped (6.6 step 5).
     A failed answer's fuel stays spent.
     Log `watane: collect <interface>.<function> of <C> from <S> failed:
     <code>: <message>`, and go on. No retry.
3. Return the joined list to C. Lowering it into C counts against C's
   memory limit (6.2).

C learns nothing of who answered: the list has no source, and the order
is the subscribers' local-name order. No new limit: the subscribers are
bounded by the catalog, and their work by the chain's fuel, time and
depth. A collect is not an event: it is never queued, never counts as a
hop or a delivery, and sends no page bus message. A subscriber cannot call
C (4.2 step 8), so C's held lock is never awaited inside the collect.

Known limit: one budget for the whole chain, so one subscriber that spins
fails the collector. The way up is a share of the fuel and time for each
answer, when a real shell meets it.

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
A name longer than 64 bytes gives no tool. Step 11: a tool mod may mark a
function `@parallel` or `@idempotent` (19.5); `agent-tools.describe` carries
both. The name `agent` is reserved as `preview` is (`bad_name`): `/mcp/agent`
is the route of handle tokens (19.5). Description: the WIT doc comment
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
participant home, only the app issues toolset tokens, in its process (W6);
the host also issues a token for each handle that `agent-tools.open` opens,
when it opens; `/mcp/agent` answers it 401 until the toolset consents are held (19.5, 19.7;
W6 as changed in spec §7). In a
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
| the token's toolset equals `<toolset>` (a handle token, 19.5, is accepted only on `/mcp/agent` and lists the union of the handle's toolsets) | 403 |
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
    /// Step 10 (6.10): a request of a manager. The app decides it in the consent view and acts.
    /// Default: `Failed("this app takes no requests of mods")`.
    fn manage(&self, request: ManageRequest) -> Pin<Box<dyn Future<Output = ManageOutcome> + Send>> { .. }
    /// Step 10 (6.10): a manager reduces authority or opens a page; called before the host acts. Must not block.
    /// Default: nothing.
    fn reduced(&self, by: &LocalName, reduction: &Reduction) {}
    /// After an install has written the catalog, before `Host::install` returns; every requester
    /// (`App`, `LocalApi`, `Mod`) and every home. Must not block. Default: nothing. (W37: the app
    /// makes a `main = true` mod main however it was installed, app 5.4.)
    fn installed(&self, report: &Installed) {}
    /// Step 11 (19.4, 19.7): `accounts` hands over the in-process account source; `consent` asks
    /// the participant one of the three agent consents; the host may drop the future (the request
    /// ended first). Defaults: no accounts, `Refused`.
    fn accounts(&self) -> Option<Arc<dyn AccountSource>> { None }
    fn consent(&self, request: ConsentRequest) -> Pin<Box<dyn Future<Output = ConsentAnswer> + Send>> { .. }
}
/// Debug, Clone, PartialEq. `new`: the local names of the plan entries that were `New`, in install
/// order; `updated`: those that were `Update`. `Unchanged` entries are in neither.
pub struct Installed { pub requester: Requester, pub new: Vec<LocalName>, pub updated: Vec<LocalName> }
pub struct InstallRequest { pub entries: Vec<PlanEntry>, pub requester: Requester }
pub enum Requester { App, LocalApi, Mod(LocalName) /* step 10: the manager that asked (6.10) */ }
pub enum Answer {
    /// Install; leave these grants ungranted (by plan entry local name).
    Approve { withhold: Vec<(LocalName, Grant)> },
    Deny,
}

// Step 10 (6.10). Debug, Clone, PartialEq.
pub struct ManageRequest { pub id: RequestId, pub by: LocalName, pub ask: Ask }
pub struct RequestId(String);   // 32 lowercase hex; Display, as_str()
pub enum Ask {
    Install { release: ReleaseRef, link: Option<String> },
    Update(LocalName),
    Enable(LocalName),
    Grant { local: LocalName, grant: Grant },
    Remove { local: LocalName, delete_data: bool },
}
pub enum ManageOutcome { Approved, Denied, Failed(String) }
pub enum Reduction { Disable(LocalName), Withdraw { local: LocalName, grant: Grant }, ClosePage(LocalName), OpenPage(LocalName) }   // OpenPage: S82, Q31
```

The default bodies keep an earlier `Confirm` compiling; a host whose app
uses them refuses nothing it did before and answers every request
`failed`.

`installed` (W37) is called exactly once per `Host::install` that wrote
the catalog (10 of 4.2 done), after the registry swap and before the
call returns, with the same `Requester`. It is not called on `refused`, on
any error, or for a plan whose entries are all `Unchanged`. It is how the
app learns of an install over `POST /api/install` (11), whose answer it
does not see; the host gives no other meaning to it.

`Plan::needs_confirmation()` is true in a participant home when an entry
is `New`, or is an `Update` with a non-empty `new_grants`, or (step 5) has
binds that differ from the installed ones (4.5). Step 10: `install` with
`Requester::Mod(_)` asks the app for every plan that has an entry other
than `Unchanged`, whatever `needs_confirmation` says (6.10). Then:

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
watane --preview <dir> [--port <n>] [--install <folder>]... [--fake-claude <path>]
watane --version
```

- `--home`: a participant home with no app (`Mode::Home { app: None }`):
  it runs and serves the installed mods; every install that needs a
  confirmation is refused. Default dir `default_home()`, default port
  `DEFAULT_PORT` (47341).
- `--preview`: a preview home; default port 0. Each `--install` folder
  joins one plan, installed before the ready line. Then the host issues the
  preview token. Step 11: `--fake-claude` calls `Host::set_fake_claude`
  before the ready line (19.9); it is a `usage` error without `--preview`.
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

SIGINT or SIGTERM: stop accepting, let running requests end for up to 5 s
(routers 4 s, then connections and blocking work until 5 s in all), remove
`run.json`, exit 0. Log lines on stderr start with `watane: `.

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
| `menu` (step 9) | outlet `menu { ask: func() -> list<string>; clicked: func(id: string); }`; export `api { items: func() -> list<string>; ping: func() -> string; click: func(id: string); }`, where `items` returns `ask()` and `click` calls `clicked`; its world is named `shell`, because the outlet already has the name `menu` |
| `item-a`, `item-b`, `item-c` (step 9) | `deps.json` `{"mods:menu": null}`; export `mods:menu/menu`: `ask` returns `["a"]`, `["b"]`, `["c"]` and puts the state key `asked`; `clicked` puts the state key `clicked` |
| `item-bad`, `item-spin` (step 9) | as `item-a`; `ask` of `item-bad` puts a state key, burns a fixed loop of about 19.5M fuel and then traps; `ask` of `item-spin` spins |
| `item-back` (step 9) | as `item-a`, and imports `mods:menu/api` |
| `manager` (step 10) | `mod.toml` `name = "Desk"`; import caller and `watane:host/manage`; export `api` with one function per function of 6.10, each passing its arguments through and returning the result as it is (`mods`, `disable`, `withdraw`, `close-page`, `open-page`, `request-install`, `request-update`, `request-enable`, `request-grant`, `request-remove`, `request`), plus `open-page-then-fail(local: string)`, which calls `open-page` and then traps, `close-open-close(local: string)`, which queues those three page effects in order, `disable-then-fail(local: string)`, which calls `disable` and then traps, and `disable-then-read(local: string) -> bool`, which calls `disable` and returns the `enabled` of that mod from `mods` |
| `latch` (step 11, A1) | import state; export `api { trip: func(key: string) -> bool; }`: `true` and puts the key when it is absent, else `false`. Its write commits when its call succeeds, even if the caller then traps (6.6, Q3), so it remembers across a rolled-back call what `relay` cannot |
| `relay` (step 11, A1) | `deps.json` `{"mods:latch": null}`; import state, caller, `streams`, `timers`, `mods:latch/api`; export `api { open: func() -> result<string, string>; push: func(s: string, text: string) -> result<_, string>; close: func(s: string); cancel: func(s: string); hand-off: func(s: string) -> result<_, string>; resume: func(s: string) -> result<_, string>; after: func(id: string, ms: u64) -> result<_, string>; cancel-timer: func(id: string); after-then-fail: func(id: string, ms: u64); mode: func(m: string); forward: func(up: string, down: string, write: bool); received: func() -> list<string>; ended: func() -> list<string>; fired: func() -> list<string>; who: func() -> string; }` (each `result` error is the WIT error case's name, e.g. `"not-yours"`) and the host-only exports `stream-sink`, `stream-producer`, `timer-sink`. `close(s)` closes with `closed`, or with `gone` while `mode` is `close-gone` (the test of the host-only outcome, 19.2). It keeps an `index` key of the `got/` keys, since `state` cannot list. `items(s, batch)`: when `forward` named `s` as an `up`, it pushes each body to that `down` and puts nothing else (with `write`, it also puts `last/<s>` = the last seq: one write per batch, for V8), and answers `defer` when a push fails `full`; else it puts each body at `got/<s>/<seq as 20 digits>` and the key `who/<s>` = `caller.identity()` (S56; a forwarded batch writes no key, so the load test sees no state bytes written per item). Then it answers by `mode` (state key, default `taken`): `taken`, `defer`, `trap` (traps), `trap-once` (calls `latch.trip("<s>/<first seq>")` and traps when it returns `true`, else answers `taken`). `received` returns the values of `got/` in key order. `stream-sink.ended` and `stream-producer.ended` add the line `<s> <outcome case>` (e.g. `"<s> consumer-failed"`) to `ended`; `ready(d)` adds `<d> ready` and calls `resume` for each `up` forwarded to `d`, and `stream-producer.ended(d, ..)` resumes them too (the forwarding rule of 19.2); `fired` adds the id to `fired`. `who` returns `caller.identity()` |
| `relay-front` (step 11, A1) | import `mods:relay/api`; export `api { after-then-fail: func(id: string, ms: u64); push-then-fail: func(s: string, text: string); who: func() -> string; }`; the first two call that function of `relay` (`after`, `push`), which succeeds, and then trap: the nested request and push that 19.6 drops; `who` returns `relay`'s `who` (S56) |
| `tool`, `tool-plus`, `tool-manage` (S103, 19.5) | package `mods:tool`; import caller; export `api { hello: func(name: string) -> string; double: func(n: u32) -> u32; who: func() -> string; slow: func(ms: u32) -> string; refuse: func() -> result<string, string>; boom: func(); }`: `hello` has the doc lines `Says hello.` and `@parallel`, `double` has `Doubles a number.` and `@idempotent`, `who` returns `caller.identity()`, `slow` sleeps `ms`, `refuse` returns `err("refused")`, `boom` traps. `tool-plus` is the same package with one more function, `extra() -> string`, so that installing it is an update that gains a function (S65). `tool-manage` is the same package and functions with the import of `manage` (used by `boom`), so that installing it is an update that gains `Manage` |
| `toolbox` (S103, 19.5) | import state, caller, streams, `agent-tools`, `claude-types`, `claude-code`; export `api { open: func(toolsets: list<string>) -> result<string, string>; describe: func(handle: string) -> result<list<string>, string>; call: func(handle, key, tool, args: string) -> result<string, string>; call-twice: func(..) -> list<string>; open-describe: func(toolsets: list<string>) -> string; open-many: func(toolsets: list<string>, n: u32) -> list<string>; close: func(handle: string); items: func(s: string) -> list<string>; ended: func() -> list<string>; who: func() -> string; start: func(launch: string) -> string; }` and the host-only `stream-sink`. An error is the case's name (`unknown-handle`; `consent-pending:<what>`). `describe` gives one line per tool, `name\|parallel\|idempotent\|description\|input-schema`. `call-twice` calls twice in one chain and returns both stream ids; `open-describe` opens and describes in one chain; `open-many` opens `n` handles in one chain (each a handle or the error's name); `items` and `ended` read what its sink took |
| `toolbox-front` (S103, 19.5) | `deps.json` `{"mods:toolbox": null}`; import caller, `mods:toolbox/api`; export `api { open, describe, call }` passing through, so that `toolbox` runs with the front as origin client |

The mutual fixtures copy only provider interfaces and outlets into
dependency worlds, without back-references (Q23). Their real components
run on the preview install path.

### 13.2 Tests

| File | Checks | Oracle |
|---|---|---|
| `src/home.rs` unit | local name grammar and derivation | a literal table: `notes` ok; `a`, 24 bytes ok; 25 bytes, `-a`, `a-`, `a--b`, `A`, `a_b`, `2x`, `é` refused; slug `reading-list` → `reading-list`; a 40-byte slug → its first 21 bytes; taken `notes` → `notes-2` |
| `src/runtime/json.rs` unit | each row of 6.9, both ways, and the error paths | literal JSON and literal `Val` trees |
| `tests/it/json.rs` | `echo` round trips through `Host::call` | literal JSON in and out; `opt-opt` is `no_function` |
| `tests/it/harness.rs` | S16 fake imports and conversion errors; subscription deliver and call separation; event records, independent homes, rollback, named and positional args, cleanup, invalid folders | literal values and events; directory absence after drop |
| `tests/it/runtime.rs` | FUNCTIONAL S4 to S9; depth with `Limits { depth: 1 }` over `recs` → `shelf` → ok and depth 0 → `limit`/`depth`; state isolation (`counter` and `shelf` with the same key) | literal values; after `drop(host)` the test opens `state/state.redb` itself and reads `s/<id>` with `id` computed from the literal identity string |
| `tests/it/program/events.rs` | S24, S26; order: `add("A")`, `add("B")`, `add("C")` → `received` = `["A","B","C"]`; a failed call emits nothing | literal lists and counts |
| `tests/it/program/pages.rs` | S11, S12, S13; a page answers alike over `127.0.0.1` and `[::1]`; path refusals (`/../x`, `/a//b`, `/.env`, `/%2e%2e/x`, `/a\b`); unknown `Host` → 421; `/watane.js` bytes equal `src/watane.js` read from disk; a 404 has `text/plain` | status codes, literal header strings, file bytes |
| `tests/it/mcp.rs` | S14, S15; `tools/call` output for ok, err and a trap; batch → `-32600`; notification → 202; `initialize` version choice | literal JSON-RPC responses |
| `tests/it/program/catalog.rs` | S16 to S23, S25, S28, S29; the pinned publisher (a provider of another publisher does not link); an update keeps a withdrawn grant withdrawn | a `Confirm` fake that counts calls and answers from a script; directory listings of `mods/`; `installed.json` parsed as plain JSON |
| `tests/it/program/api.rs` | the four endpoints; `Origin` → 403; S22; S30; W37: a `POST /api/install` of two new folders calls `Confirm::installed` once with `requester` `LocalApi` and both names in `new` in install order; a denied install and a failing plan call it never; an unknown API route → 404 `not_found`; a `/call` dropped mid-spin leaves the server answering and `Served::shutdown` within its 5 s bound | status codes, literal JSON; the `Confirm` fake's record of calls |
| `tests/it/program.rs` | S1, S2, S3 through `env!("CARGO_BIN_EXE_watane")`; SIGTERM removes `run.json`; `--bad` exits 2 | the stdout line parsed as plain JSON; exit codes; file presence |
| `tests/it/program/hostile.rs` | `/call` with a subscription function → 404 (`ac12_s11_page_cannot_call_a_subscription_function`); a body field `caller` → 400; `/call` with the origin of another mod → 403; `/api/...` with any `Origin` → 403; `Host: evil.example:<port>` → 421; a token of toolset `a` on `/mcp/b` → 403; a 2 MiB `/call` body → 413; `counter` without storage grant cannot read state that it wrote before the revoke (`ac8_s12a_storage_after_revoke_cannot_read_old_state`); exact and repeated Host checks on both loopbacks; preflight without CORS; streamed and declared oversized bodies; mod/API route isolation; raw path tricks; cross-origin events; S11 to S13 through the program | status codes, literal headers and JSON, fixture page bytes; every step-1 probe uses `Preview` |
| `tests/it/binds.rs` | S33 to S35: signed picks, persistence, bind-only updates, removal, validation, preview matching and local API rejection | literal binds, links, JSON, caller and confirmation count |
| `tests/net.rs` | S27: `https://example.com/` and `http://example.org/` fail without a socket; `https://example.org/` succeeds (`#[ignore]`, needs network) | literal error code |
| `tests/it/manage.rs` (stage 11) | S40 to S47 with `manager`, `counter`, `shelf`, `recs` in a participant home whose `Confirm` fake records `reduced`, `manage` and `install` calls and answers `manage` from a script; the harness rows of 6.10; `manager` refused at plan when the world lacks `manage` but the component imports it (5.3) | literal `mod-info` JSON; `installed.json` parsed as plain JSON; the fake's recorded calls; request ids matched by the regex `^[0-9a-f]{32}$`; page-bus messages read from `/events` of a served manager |
| `tests/it/streams.rs` (A1) | agentic-harness cases 7, 36 and 51 (host parts) and 58, with `relay`, `latch` and the test streams of 19.9 (feature `test-streams`: `Host::test_stream`, `TestConsumer::Held`, `TestClock`; no H2 to H4): 19.2 and FUNCTIONAL S48 to S52; a page consumer through a served `relay` and a `reqwest` reader of `/events`; a page that stops reading is a reader that stops polling the body, with items of 64 KiB so the socket buffers fill | literal item numbers and bodies read from the consumer's state; `ended` lines; `TestStream::deliveries()` |
| `tests/it/requests.rs` (A1) | agentic-harness cases 20 and 21 with `relay`; the rows of 19.6 (limits, replacement, rollback); with `relay-front` for the nested rollback; FUNCTIONAL S53 to S56 | literal `fired` lines; `timers` table read from `state/state.redb` after `drop(host)`; a restart on the same home |
| `tests/it/agent_tools.rs` (A3, S103) | 19.5, 19.7, 10.2, 10.3; FUNCTIONAL S56 (agent), S60, S65, S68: a home whose `Confirm` fake holds each consent answer, the fake `claude` that copies fd 3, and `toolbox`, `toolbox-front`, `tool`, `tool-plus`, `manager`; `open` `not-granted`; the question's fields; `consent-pending` and `consent-refused`; the stored list in `installed.json`; annotations; keys and jobs; foreign and same-chain handles; the manager's functions; a gained function; `remove` and a `grant` of `Manage`; `--mcp-config=/dev/fd/3` and the token on `/mcp/agent` (listed tools, 403 on another route, 401 after `close` and after either mod ends, never in an item, argument or file); withdrawal | literal strings and JSON; `installed.json` parsed as plain JSON; the ended lines of `toolbox`; HTTP status codes |
| `tests/it/agent_load.rs` (A1, `#[ignore = "slow: stage A budgets and V8"]`) | the load row of stage A (15): 8 `Host::test_stream(TestConsumer::Mod(relay))`, each forwarded by `relay.forward(up, down, false)` to a stream that a served page of `relay` opened with its connection id; each body starts with its send time; 60 s at 100 items/s each; then `api.cancel` of one page stream from the page; then the V8 run (15) again with `write` true | p95 item delay (send time to the `/events` read); `cancel` `/call` to its `stream-end` message; writer busy share and state bytes per item from two `Host::test_stats()` readings; every page stream's seqs read in order with no gap |
| `tests/it/verify.rs` (stage 8) | S31, S32 | a `watane_log::Log` on a temporary directory signs the body with `append(RELEASE_KIND, ..)`; the expected `ReleaseRef` text is built from `Log::identity()` and literals; bytes are changed by the test |

## 14. Security properties

| Property | Enforced by | Tests |
|---|---|---|
| A logic part has no device access | empty `WasiCtx` (6.5); import check (5.3) | S9, S17 |
| Each call ends | fuel shared by the chain; epoch deadline; memory limiter; depth; state limits (6.2) | S6 to S8, runtime |
| An ungranted import never reaches a provider | link and grant read from the current catalog at each nested call (6.6); stub otherwise | S25 |
| The caller comes from the transport | `Caller` set by the entry point; `mod:` only by the linker; `/call` refuses unknown fields | S10, hostile |
| Only the host calls a subscription | `/call` and tools exclude subscription exports (6.6 step 2) | hostile |
| A page reaches only its own mod | one origin per mod; exact `Origin` on `/call`; CSP; COOP and CORP (8.2, 8.3) | S11, S12 |
| No DNS rebinding | exact `Host` check (8.1) | hostile |
| Only this machine reaches the server | binds only `127.0.0.1` and `::1`, never a wildcard address (8.1) | pages |
| The local API has no authority | `Origin` refused; install needs the app in a home; no destructive route (11, 12.1) | S21, S22, hostile |
| An agent reaches only its toolset | 256-bit tokens, stored as hashes, bound to toolset and handle; `Origin` refused (10.2, 10.3) | S15 |
| A tool name names its mod | local-name prefix; `_` in no local name and no WIT id (3.2) | S14 |
| A release is what its publisher signed | `Entry` and `Package` exist only after their checked decoders; the checks of 5.2 | S31, S32 |
| A provider is the pinned one | publisher equality in linking (4.2 step 7); the one exception is a bind, which only the app passes after the participant picked it, and which the consent view shows (4.5); a preview compares no publisher (W27) | catalog, `tests/it/binds.rs` |
| State is private | one table per identity; host functions touch only the calling mod's table (7) | runtime |
| A collect asks only granted subscribers, and does not name them | the set of 9.2 step 4, read from the current catalog at each collect; the answer list carries no source (9.4) | S36 |
| A collect cannot deadlock | collect links are cycle edges (4.2 step 8) | S38 |
| A manager only reduces on its own | reads and the three reductions are the only acting functions; each addition is an `Ask` that only the app decides; a manager-started install always asks (6.10, 12.1) | S41 to S44 |
| A manager acts only while it holds the grant | the grant check at each function and again before each effect; pending requests dropped when it loses the grant (6.10) | S45 |
| A failed manager call changes nothing | manage effects join the chain only on the manager's success (6.10) | S46 |
| A manager holds at most one request in the consent view | one pending request per manager (6.10); it may ask again after an answer, until the participant withdraws `Manage` | S44 |
| A manager reaches no identity, signature or consent answer | `watane:host/manage` has no such function; the host never signs; only the app answers `Confirm` (W35) | review rule |
| A network grant is narrow | HTTPS only, exact host list (6.8) | S27 |
| A preview never holds participant data | mode fixed in `home.json`; one host per home (3.1) | S2, S3 |
| No path escapes `ui/` | path rules and canonical prefix (8.3) | pages |
| A mod cannot widen `claude` past its launch (step 11) | the grammar and one argument per value; `--restricted`; fixed flags; closed subagent keys; the host owns stdin; the `initialize` and `init` checks (19.3). Files: `src/agent/claude/launch.rs`, `init.rs`, `control.rs`, `run.rs` | A2: agentic-harness cases 12 to 17 |
| A mod cannot run a denied skill by name | `slash-command` (19.3). File: `src/agent/claude/control.rs` | A2: case 11 |
| A failing check cannot let a tool run | `onFailure: block` confirmed in `initialize` (19.3). Files: `init.rs`, `control.rs` | A2: cases 10, 14 |
| "Safe" cannot widen the mode | a check is answered `none` or `deny`, never `allow` (19.3). File: `control.rs` | A2: case 15 |
| Writes cannot plant code that runs later | the protected paths of the host's `--settings=` (19.3). File: `run.rs` | A2: case 18 |
| No mod sees a credential | tokens, keys and the `claude` path only through `AccountSource`; MCP tokens on fd 3; handle names, not handles (19.4, 19.5); the protected reads of 19.3 (deny rules, relative file-tool allows and no bare one, no `allow` answer outside the workspaces by path or pattern). Files: `src/agent/https_stream.rs`, `src/agent/tools.rs`, `src/agent/claude/launch.rs`, `run.rs`, `control.rs`, app account files | A2: S63, S67; A3: S68; A4 |
| A plan token goes only to OpenAI's agent endpoints | the account binding of `openai`, the forbidden headers, no redirects (19.4). File: `https_stream.rs` | A4: case 19 |
| A judge key goes only to its judge | the binding of `judge`: identity, value, one origin parse, `POST`, `[http]`, no redirects, `no-account` (19.4). File: `https_stream.rs` | A5: cases 19, 33 |
| A mod in the middle gains no authority | consent per origin client, carried with the chain by identity (19.7); a subscriber is its own origin client (19, Terms); the grants and the origin client checked again at apply (19.6); a toolset consent covers only the functions it named, and `remove` or a `grant` of `Manage` drops that mod's names from it (19.5, 4.4); `remove` leaves no timer carrying the removed mod (4.4). Files: `src/agent/consent.rs`, `src/runtime/call.rs`, `src/runtime/events.rs` | A3: S61, S62, S65, S66, S68 |
| A stream has one consumer and cannot starve the host | consumer rules, byte budgets, the lag clock and the deferral backstop (19.2). File: `src/agent/streams.rs` | A1: cases 36, 51, 58 |
| A repeated call changes nothing | job keys and numbered items (19.5, 19.2). Files: `tools.rs`, `streams.rs` | A1, A3 |
| An ask shows the tool call that will run | the host serializes each `ask` and `check` from its own parsed value, with the call's `tool_use_id` and `expires-at`; no `claude` line passes as a host item (19.3). File: `control.rs` | A2 |
| A rolled-back call has no outside effect | requests and stream effects ride up the chain and apply only after its durable commit; a deferred or failed call drops its own and those of the calls nested in it (19.6). File: `src/runtime/call.rs` | A1: cases 21, 36; S53 |

Not protected (spec §6): another program of the same user, which can read
the home and call `/call` or `/events` with a forged `Origin`, and can write
`runs/pids` naming a live group leader of that user with its true start time
and boot id, which the host then kills at open, or plant `runs/` as a link to a
folder it controls after the cleanup, or mount a file system under `runs/`
that the cleanup walks or that stalls it (3.1 step 3; adversary S100 passes 13,
14 and 16), or build a tree under `runs/` deeper than about 10,000 levels,
which can overflow `remove_dir_all` in the cleanup at open (same user, outside the
mod sandbox; S100's adversary pass 17, accepted by the deputy); a
compromised OS; a fault in Wasmtime; a privileged network change that adds
`::1` after a host started without it (8.1): `[::1]:<port>` is then free for
another program until the host restarts. Step 11 (agentic-harness TECHNICAL
14): `claude` itself within its limits; a prompt injection within what a
launch allows; an allowed command word that the list of 19.3 misses, or a
package-runner word that runs the project's own scripts, inside the shell
grant's sandbox only (whether that sandbox blocks loopback is not yet
probed); the judge account reaches any `POST` path of the origin the
participant entered; plan usage; a launch is as safe as the grant, and the
grants are the participant's (W38, W39). The protected-reads answer rule
(19.3) checks a file tool's path when `answer` is called; a link swapped
between that check and the moment `claude` opens the file (by a sandboxed
`Bash` under the shell grant, or another program) is not caught there, and
the deny rules of the host's `--settings=` are the limit (S100's adversary;
accepted by the product owner, S100-CAP). Resource use under a compromised
`claude` (S100's fifth adversary pass): a process that keeps `claude`'s
stdout open and never ends it (a `setsid` grandchild) keeps that run's reader
thread, pipes and run folder until it exits; the stream itself still ends at a
cancel, disable or app quit (accepted by the product owner, S100-CAP). One
stream's kept ask input (up to 64 asks of up to 4 MiB) can hold its origin
client's whole 128 MiB budget, so the owner's other streams get `full` and end
`lagging` (S100's seventh pass; accepted by the product owner, S100-CAP). And
(S100's eighth pass): each `claude` stdout line takes the host's chains gate
and sums the pending pushes of every chain in flight, so a mod that keeps many
chains with many queued pushes makes every line slower, and an apply that holds
the gate delays them; a running per-origin total of pending push bytes would
make it constant time. A cost, not a wrong decision; not built.

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
changed. Each mod queues its publication before releasing its call mutex.
Publication keeps that mod's commit order; unrelated mods run independently.
The host owns one broadcast channel with capacity 1024. `/events` filters by local
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
`remove_toolset`, `toolsets`, and `tokens` are available in the app agents view (S58 completes host stage 4).
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
limits. Each emitter queues committed events before releasing its call
mutex. A queue head becomes ready after its invoking chain releases its
mod locks. Callers drain their own ready events, one at a time per emitter.
They release queue locks before invoking a subscriber and hold no emitter
lock. Subscriber invokes take their normal per-mod call locks. A caller
waits when another call has an earlier event of the same emitter.
Each original call waits for its own deliveries. Page batches retain each
mod's commit order and become publishable when that call's event chain
finishes. Each changed mod gets one `changed` message at its last page
batch of that chain. Only changed state bytes add this message. Queue locks
only protect enqueue, dequeue and publication; they never cover a call.
This keeps nested calls and event cycles free of publication lock waits.
This ends the S2 link refusal, S3 subscriber deferral and S4 fake/deliver cuts.
Harness imports use fakes and subscriptions use `deliver`; Harness functions,
participant confirmation and revoke remain deferred.

### S12a cut

Stage 5 adds participant folder plans, scripted app confirmation, persisted
grants and lifecycle operations. `Mode::Home { app }` attaches `Confirm`;
`serve(port, extra_api)` accepts the app router. `install_folders` stays a
synchronous preview and quiet-update wrapper. Participant installs that
need consent use async `install`. Plan staging ends on drop or install.
Toolsets and live token metadata complete stage 4. Release sources, network
grants and Harness functions remain deferred. Axum middleware applies the
common host and Origin guards before the merged API router. No dependency
is added.

| Stage | Build | Needs | Until it exists | Done when |
|---|---|---|---|---|
| 1 Runtime core | engine, limits, epoch ticker, `Ctx`, state store, `watane:host` interfaces, JSON encoding, top-level call; fixtures `counter`, `echo`, `hog` | none | load a mod through a crate-private `RawMod { local, identity, package, component bytes, grants, links }` built by the test | `tests/it/json.rs`, S4 to S9 pass |
| 2 Links and events | nested calls, stubs, outlets, delivery, hops, page bus; fixtures `shelf`, `recs`, `loop-*` | none | `RawMod` with hand-written links | S10 (through `recs`), S24 to S26 pass on `RawMod` |
| 3 Server and pages | routing, `/call`, `/events`, files, CSP, `watane.js` | none | `RawMod` | `tests/it/program/pages.rs`, page rows of `tests/it/program/hostile.rs` pass |
| 4 Tools | toolsets, tokens, MCP | none | `RawMod` | `tests/it/mcp.rs` passes |
| 5 Catalog and home | home layout, lock, `installed.json`, plan, install, update, remove, grants, confirmation, import check, local API, `Harness` | `pkg`: section 17 without `Release` | write the catalog logic against `RawMod` first; do not write a second `mod.toml` or WIT reader; wire `Mod::read` in when pkg lands | S16 to S23, S25, S28 to S30 pass |
| 6 Program | `watane` binary, ready line, `run.json`, signals | stage 5 | none | S1 to S3 pass; `dev`'s `serve` drives a preview (J1 with `dev`) |
| 7 Network grant | wasi:http hooks; fixture `net` | stage 5 | none | `tests/net.rs` passes |
| 8 Releases | `Source::Release`, section 5.2 | `log`: `Entry`, `Log` (tests); `pkg`: `Release` | none; the stage waits | S31, S32 pass; J3 with `dist` and `app` |
| 9 Binds (step 5) | `Bind`, `plan_with`, 4.5, `binds` in `installed.json`; the preview rule of 4.2 step 7 | stage 8 | none | S33 to S35 pass (`tests/it/binds.rs`); J5 with `dev`, `dist` and `app` |
| 10 Collect (step 9) | 9.4; the outlet row of 6.4; collect edges in 4.2 step 8; the 5.3 rule; collect fakes in `Harness` (2.6); fixtures `menu`, `item-*` | stage 5; `pkg` stage 9 (`World::collects`) | none; the stage waits for pkg | S36 to S39 pass (`tests/it/collect.rs`); J9 with `dev` and `app` |
| 11 Manage (step 10) | `manage` in `wit/host.wit`; the linker row and 6.10 (reads, reductions, the request table, effects after the chain, the grant checks); `Confirm::{manage, reduced}`, `Requester::Mod`, `ManageRequest`, `Ask`, `ManageOutcome`, `Reduction`; the 12.1 rule for `Requester::Mod`; the `catalog` page message; the 5.3 rule; manage fakes and `Outcome.manages` in `Harness`; fixture `manager` | stage 5; `pkg` stage 11 (`Grant::Manage`, `World::manage`) | none; the stage waits for pkg | S40 to S47 pass (`tests/it/manage.rs`); J10 with `dev` and `app` |
| 12 Installed hook (W37) | `Confirm::installed` and `Installed` (12.1), called from `install` | stage 5 | none | the W37 row of `tests/it/program/api.rs` passes; every earlier test passes unchanged (the default body) |
| A1 Streams, requests, timers (step 11) | `streams`, `timers`, `caller.identity`, the host-only exports in `wit/host.wit` (19.1); H1 (19.2) with the page connection id (8.2, 8.4, 8.5, 9.3); H5 (19.6, the chain rule of requests and stream effects, 6.6 step 4) and H7 timers with the `timers` table (7); the linker rows and fakes (6.4, 19.1); fixtures `latch`, `relay`, `relay-front`; `Outcome.requests`; the feature `test-streams` (19.9) | stage 5; `pkg` stage 12 (the step 11 rows of 4.5, `World::host_exports`, 4.6) | `RawMod` and the test streams of 19.9 for host streams | agentic-harness cases 7, 20, 21; 36 and 51 (host parts); 58 (host part); S48 to S56 of FUNCTIONAL; the budgets of stage A; V8 recorded (`tests/it/streams.rs`, `requests.rs`, `agent_load.rs`) |
| A2 Claude Code (step 11) | H2 (19.3): launch, run, control protocol, folders, the `ask-resolved` and `wrapped` items (19.3; Q35, Q36); `host-info` (19.1, Q34 decided: the interface in `wit/host.wit`, its linker row, the `host` table of 7, the fake); the doc comment of `caller` in `wit/host.wit` lists `host` among `who()`'s values (19.1; S96's user-sim; with it every fixture copy of `host.wit` and the plan hashes that include one); the connection-replaced message of `watane.js` (8.5); R7: a stream that ends from the producer side while its consumer holds a deferred batch delivers the held and queued items after `resume`, then `ended` (19.2, Deferred batch; S69); `claude-code`, `claude-code-shell`, `claude-session`; `claude-types`; `AccountSource::claude_binary` (19.4); the grant checks and stubs; dev's `fake-claude`; `set_fake_claude` and `watane --fake-claude` (19.9) | A1; `pkg` stage 12 (`Grant::ClaudeCode`, `ClaudeCodeShell`; the `host-info` row of pkg 4.5, added in the same slice); dev 18 | `Mode::Home` with no app: `no-app` | agentic-harness cases 10 to 18 (host parts) and the host parts of the other stage B cases; no workspace consent yet (scratch workspace only); `host-info` and the restart rule: `generation()` is raised by one at each `Host::open` and is the same between; S63, S64 (`say` part), S67, S69 |
| A3 Consents and tools (step 11) | 19.7 (`Confirm::consent`, `consents` in `installed.json`, withdrawal); H4 (19.5); `consent-pending` | A2; app 19.2 | a fake `Confirm` | the workspace consent held before `initialized`; withdrawal ends dependent streams and closes handles; case 31 `consent` part at the host; the budgets of stage C; S61, S62, S65, S66, S68 |
| A4 HTTPS streams, `openai` (step 11) | H3 (19.4) with the account `openai`; `AccountSource::openai_token`, `AccountError::Unavailable`; `Account`, `account_changed(&Account::OpenAi)`; the feature `test-net` and `set_test_net` (19.9); `sse-stream`; dev's `ScriptedResponses` | A3; app 19.1; dev 18 | a fake `AccountSource` | agentic-harness cases 3, 19 (`openai`); the budgets of stage D; S64 (header part) |
| A5 Judge (step 11) | the account `judge` (19.4): binding, `no-account`, `judge_origin`, `account_changed(&Account::Judge { .. })` | A4; app 19.4; dev 18 judge origin | the same fakes | agentic-harness cases 19 (`judge`), 32, 33 (host parts); the budgets of stage E |

Step 10 is done when J10 passes with `dev` and `app` (app 17).

Step 11 (W38): A1 to A5 map to stages A to E of agentic-harness TECHNICAL
17 (A1 is its stage A; A2 its B; A3 its C; A4 its D; A5 its E). Each is done
only when its tests pass; the agentic-harness slice that uses a stage names
the same interface and fakes it until the watane stage merges.

**A1 (S96), sized.** One slice builds: the WIT of 19.1 for `streams`,
`timers`, the three host-only exports and `caller.identity`; the table, FIFO
delivery, batches, `taken`/`defer`, `resume`, the lag clock, the deferral
backstop, the byte budgets and the ends of 19.2; page connection ids and their
lossless queues (8.2, 8.4, 8.5, 9.3); request effects after a durable commit
and timers (19.6, 7); the fixtures `latch`, `relay`, `relay-front`;
`Outcome.requests`; the feature `test-streams` (19.9). No `claude`,
HTTPS, handle or consent code. Its tests: agentic-harness case 7 (a failed
batch is redelivered once, same numbers, duplicates dropped), 20 (a timer due
during downtime fires after start; a firing for a cancelled id is ignored), 21
(kill after the commit of a request and before its effect: the effect happens
once after restart; the effect tested is a timer, the only request that
outlives a restart, 19.6), 36 (host part: `defer` gives the same batch after
`resume` and nothing before; two defers never end the stream `consumer-failed`;
a deferred call's pushes and writes are gone; a deferred host stream stops
reading its source), 51 (host part: a page that stops reading ends `lagging`
after 10 s; a page that takes a few items and stops: the same; a consumer that
never resumes ends `consumer-failed` after 5 minutes, with a clock the test
controls), 58 (host part: producers past an origin client's 128 MiB or the
host's 512 MiB get `full`; a page connection past 16 MiB ends and its streams
end `lagging`).

**A2 in two slices (S99's foreman, 2026-11-22).** S99 (agentic-harness S35,
one turn) builds: `host-info` (19.1, 7); the doc comment of `caller`; the
connection-replaced message of `watane.js` (8.5); R7 (19.2); the WIT linking of
`claude-types`, `claude-code`, `claude-code-shell` and `claude-session`; the
launch document of 19.3 with every field parsed and checked; the scratch
workspaces (Folders); the run (binary, version, process group, `runs/pids`,
the fixed environment, the host's `--settings=` with the protected paths, the
protected reads and `sandbox.allowUnsandboxedCommands: false` for every start);
steps 1 to 4 of the control protocol for lines (`initialize`, the `init`
match, `initialized`, `wrapped`, lines unchanged); `say`, `interrupt` and
`compact`; `AccountSource` (19.4, `claude_binary` the only method the host
calls yet), `Confirm::accounts`, `set_fake_claude` and `watane --fake-claude`;
dev's `fake-claude`. S99 left the asks and checks cut: `ask: true`, a
`pre-tool-check`, an `agent-handle` (A3) and a non-empty `skills` were
`bad-launch {field, reason}` naming the stage, `answer` was `unknown-ask`, and a
`can_use_tool` or `hook_callback` was answered deny at once. `claude-code-shell.start`
checks its grant and its launch, then its stream ends `failed("the shell grant waits
on section 18, Q39 and Q42")`.
**S100 (agentic-harness S36, 2026-11-22) lifts that cut for asks and checks.** It
builds: `ask: true` (`--permission-prompts=host --permission-prompt-tool=stdio`) and
`pre-tool-check {timeout-s: 5..120}` (the PreToolUse hook of `initialize`, and the
step 2 check of its answer: otherwise the stream ends `mismatch(hooks)`); the `ask` and
`check` items, their `expires-at` on the host clock and the one `ask-resolved` that
closes each (`answered`, `expired`, `cancelled`); `claude-session.answer` with the
checks of 19.3 step 5 made at the call, the third rule of the protected reads (a path
resolved with symlinks, `src/agent/claude/paths.rs`), `control_cancel_request`, and
`interrupt` closing the open asks and checks. A request that the launch did not ask for
(a `can_use_tool` without `ask`, a `hook_callback` without `pre-tool-check`) or that
is malformed is still answered deny at once and pushes nothing; `answer` to an id that
is not open is `unknown-ask`. Still cut: `agent-handle` (A3), a non-empty `skills`,
the shell grant's sandbox; each still names its stage. A `hook_callback` is taken
for the host's whatever its `callback_id`: the host registers one callback.

**A3 in two tasks (S103).** S103's first task (host) builds the workspace half of A3:
`Consent`, `ConsentRequest`, `ConsentAnswer` (19.7) for all three kinds, `Confirm::consent`
(default `Refused`), `InstalledMod.consents` (3.3), `Host::withdraw_consent`; picked folders in
`workspace` and `extra-workspaces` (19.3, Folders: resolved at the call, the consent asked at
apply before the process, `consent-pending`, `consent-refused`); the protected paths once per
workspace root (Q49); the `sessions` table and the session rule (Q50, 7); `via` in the workspace
key (Q38). The account consent is only a type until A4. The second task (agent tools) builds
the toolset consent and the handles of 19.5, and fills `agent::consent::withdrawn` for them.
**Built (S103, second task):** `agent-tools` (`open`, `describe`, `call`, `close`) in
`src/agent/tools.rs`: the handle table, the toolset consent asked on a thread of its own, one
token per handle (`Host::issue_agent_token`, bound to the toolset name `agent`), the jobs with
their keys, and the annotations `@parallel` and `@idempotent` read from the doc comment;
`/mcp/agent` (10.3); `agent-handle` in a launch with the MCP config on fd 3 (19.3, The run);
the toolset rows of withdrawal (`withdrawn` closes the handles and ends their jobs `gone`);
`remove` and a `grant` of `Manage` drop a mod's names from the stored lists (4.4); the
reserved toolset name `agent` (10.1). `agent-handle` is no longer cut.

**Budgets of stage A** (the "Load" kind; all p95; a slow test on a stage's
fakes, recorded with the date and the machine below; a stage is done only
within them). Load: 8 host streams × 100 items/s of 1 KiB from a test source
in the host's own tests, through a mod consumer (`relay`) to a page
connection. Item delay, source to page: at most 50 ms. `cancel` to `ended`:
at most 250 ms. Writer busy share (the share of wall time the host's state
writer spends in commits, fsync included, over the run): at most 10%. State
bytes written per item: none (the forwarding `relay` reads its two routing keys
per batch, about 100 bytes, which `Host::test_stats` also counts and the load
test records apart).

**V8 record.** V8 (the probe of agentic-harness TECHNICAL 16) runs the A1
prototype through three layers (host stream, a mod consumer, a page consumer)
and measures order, delay and the cost of one fsync per commit (N4, 19.6). The
numbers, the date and the machine go here; whether to change 19.6 "Durable
first" is the human's. The run is the load of `tests/it/agent_load.rs`
twice: `relay` forwarding with no write, then with one state write per batch
(`forward(.., true)`: one commit, so one fsync, per `items` call). Record for
each: p50 and p95 item delay source to page, items out of order or missing
(must be 0), commits, and the mean time per commit (`commit_time / commits`
of `Host::test_stats`). agentic-harness keeps its own copy of the record (its
TECHNICAL 16); its slice copies these numbers.

Run of 2026-11-21 (`cargo test --release --test it -- --ignored ac13`; 60 s per
run, 8 streams × 100 items/s of 1 KiB, 48 000 items each; 4 cores,
`Linux 6.18.44-fc-v726 #1 SMP PREEMPT_DYNAMIC x86_64`, a virtual machine, real
fsync on its disk):

| Run | p50 delay | p95 delay | out of order or missing | commits | mean per commit |
|---|---|---|---|---|---|
| `relay` forwarding, no write | 1.72 ms | 3.25 ms | 0 | 0 | none |
| one state write per batch | 3.39 ms | 6.36 ms | 0 | 47 483 | 0.31 ms |

With one write per batch the writer was busy 24.5% of the wall time (one
fsync per commit, N4): over the 10% of the stage A budget, which only the run
with no write must meet. The human decides whether 19.6 "Durable first" changes.
The first runs of the test showed a 45 ms p95: Nagle's delay on the server's
sockets (small SSE writes). The server now sets `TCP_NODELAY` (`src/server.rs`).

**Budget record (stage A).** The numbers of the stage A budgets above, from the
run of 2026-11-21 on the machine of the V8 record (4 cores, a virtual machine),
the run with no write: item delay source to page p95 3.25 ms (budget 50 ms);
`cancel` to `stream-end` 0.5 ms (budget 250 ms; one measurement, not a
percentile); writer busy share 0.0% (budget 10%); state bytes written per item
0, read per item 103 (the routing keys, see above). All within the budgets.

**Budgets of stage B, the host part** (deputy's ruling on review r1 of S100,
2026-11-23, after agentic-harness TECHNICAL 17's stage B row: 8 fake `claude`
sessions, 2 owners, each 50 deltas/s and 5 finished entries/s of 4 KiB; item delay
source to page at most 100 ms, `interrupt` to `idle {interrupted}` at the page at most
1 s, writer busy share at most 50%, state bytes per finished entry at most 2 MiB within
10% between transcripts of 10 and 10 000 entries). That row is a kernel-plus-host
budget, measured end to end in agentic-harness S36; source to page, interrupt to idle
at the page and the state bytes per entry are S36's. The host's share, a slow test
`tests/it/agent_load.rs::stage_b`, at that load (8 streams on host's fixture fake, 4
started and consumed by each of 2 consumer mods, each line stamped with its write time
on the host's clock): item delay host to consumer at most 100 ms; an `ask` taken at
most 100 ms; `answer` to the `control_response` on stdin at most 50 ms; `interrupt` to
its `ask-resolved` and to the turn's `result` taken at most 1 s; `cancel` to `ended` at
most 250 ms; writer busy share at most 50% (the host's own share recorded apart from
the consumers'). Every line arrives once and in order. dev's `fake-claude` `load` step
produces the same load for S36 (dev 18.2).

**Stage B host record** (S100, 2026-11-23; `Linux vm 6.18.44-fc-v726 #1 SMP PREEMPT_DYNAMIC
x86_64`, `nproc` 4, a virtual machine with a real fsync; nothing else running, load average
0.5 to 0.7; `cd host && cargo test --test it agent_load::stage_b:: -- --include-ignored` three
passes in a row, every budget met in each; re-run by the foreman 2026-11-23 after review r3
and s100-fix14, nothing else running, load average 0.8 to 0.9). The load of
`tests/it/agent_load.rs::stage_b::ac1_stage_b_load`: 8 sessions, 4 on each of the consumer mods
`claude` and `claude-b`, on the fixture fake `claude` in its rate mode (50 deltas and 5 entries
of 4 096 bytes a second, each line stamped with its write time; 10.6 s before the interrupt,
about 4 660 lines, none out of order or missing, every one once). The test asserts, per stream,
the delta rate and the entry rate (counts of each kind over one window, the turn's first to last item of either kind on
the fake's clock, within 10% of 50 and 5 a second; measured 50.1 and 5.00 over 10.6 s), the size of every entry (the
length of its `message.content[0].text`, which the fake pads to 4 096 bytes, read by the consumer), that the turn's
`result` is the interrupted one (subtype `error_during_execution`, written by the fake after it
read the interrupt) and that `ended` came after the `cancel` that it times; the workload
`55 0 4096 10.2` fails it, and so does one whose first entry comes late. The counters (`before`) are read
only after every stream's `initialized` reached its consumer, so the scratch row commits of the starts
are outside the window; the host counter counts every host-owned commit (scratch add, delete,
remove, the generation, timers), and the figures cover the window from that reading to the last `cancel`. One `ask` answered and one left open for the `interrupt`; the turn's
`result`, then `cancel`. Figures, pass 1 / 2 / 3 (the p95 over the 8 sessions is the worst,
one value per kind): item delay host to consumer, the worst session's p95, 3 / 6 / 4 ms (budget
100; the histogram's ms upper edge, each session's p95 over its own 580 lines); `ask` taken 3.8 /
2.6 / 3.3 ms (100); `answer` to the fake's stdin 13.9 / 5.0 / 7.4 ms (50); `interrupt` to
`ask-resolved` 10.7 / 18.3 / 14.9 ms and to the turn's `result` 14.6 / 18.3 / 15.8 ms (1 000);
`cancel` to `ended` 9.0 / 8.3 / 7.9 ms (250); writer busy 20.2 / 20.3 / 19.1% (50), measured by
owner on the commit path (`TestStats.host_commits`, `host_commit_time`): the host's own commits
(timer and scratch writes) 0 of about 4 240, so 0.0% of the share, and the mods' (the consumers' one small
tally write per batch, mean commit 0.48 to 0.51 ms) all of it. Three busy loops beside it (load
average 2) left every figure within budget (interrupt and cancel up to 24 ms). With another build
running (load average 5 to 9 on the 4 cores) the same load took the writer to 53 to 59% (a commit
took 1.4 ms, not 0.6), and a build beside it once took the first `ask` to 260 ms: the budgets need
a quiet machine.

Step 2 is done when J2 passes with `app`. Steps 4 and 5 need only
HOST-F-42 and HOST-F-43, which stages 5 and 8 meet.

## 16. Provides to other subsystems

| To | Item |
|---|---|
| `app` | `Host::open` with `Mode::Home { app: Some(..) }`; the `Confirm` trait, `InstallRequest`, `Answer`, `Requester`; `plan`, `install`, `mods`, `get`, `enable`, `disable`, `remove`, `grant`, `revoke`; toolsets and tokens; `serve(port, extra_api)`, `Served::{page_url, mcp_url}`; `PlanEntry`, `InstalledMod`, `MissingProvider`; `default_home`; `DEFAULT_PORT`; the re-exports `watane_pkg`, `watane_log` (no direct dependency needed) |
| `app` | Existing items, nothing added: toolset UI = `mods()` + `functions(local)` (no subscription export), entries `<local>:<interface>.<function>`; give back a withheld grant = app's consent view, then `grant` (4.4); "replaces" = `Change::Update`; "derived from" = `InstalledMod.origin` |
| `app` | the page origin `http://<local>.localhost:<port>/` to load as a top-level view, on `127.0.0.1` and `::1` (8.1); CSP `frame-ancestors 'none'`; close a mod's view on disable or remove; `<home>/app/` reserved (3.1); `extra_api` routes set their own body limit (8.1) |
| `dist` (through `app`) | `Source::Release { package: Package, entry: Entry }` for `plan`, as `Fetched` gives them; `MissingProvider.release` (a `ReleaseRef`) to fetch; `InstalledMod.release` (a `ReleaseRef`) to compare with updates and withdrawals; `<home>/dist/` and `<home>/log/` reserved (3.1) |
| `app` (step 10) | `Confirm::{manage, reduced}`, `ManageRequest`, `RequestId`, `Ask`, `ManageOutcome`, `Reduction`, `Requester::Mod` (12.1); install with `Requester::Mod(by)` for a manager's install and update (6.10) |
| `app` (W37) | `Confirm::installed`, `Installed` (12.1): one call after each install that wrote the catalog, any requester |
| `dev` (step 10) | the manage fakes of 6.10 under the `Fakes` key `watane:host/manage`; `Outcome.manages`, `ManageRecord` (2.6); `HOST_WIT` with `manage` |
| `dev` | the program `watane` (12.2), `Ready`, `RunInfo::read`, the local API (11), `default_home`; `Harness`, `Fakes`, `Outcome`, `EventRecord` (2.6; `FunctionInfo` joins S5); `HOST_WIT` for `wit/deps/watane-host/`; `WATANE_JS`; the JSON encoding (6.9); the CSP rule that a page has no inline script (the template must load its script from a file). The S11 cut lists deferred Harness items. |
| `pkg` (step 11) | the `watane:host` interfaces of 19.1 as imports and exports, for pkg's import check (pkg 4.5): imports `streams`, `claude-types`, `claude-code`, `claude-code-shell`, `claude-session`, `https-stream`, `agent-tools`, `timers`, `caller.identity`, and `host-info` from stage A2 (19.1, the row of pkg 4.5); exports `stream-sink`, `stream-producer`, `timer-sink`; the grants `ClaudeCode`, `ClaudeCodeShell` (the WIT `grant` variant of 6.4) |
| `app` (step 11) | `Confirm::{accounts, consent}` (one consent method for the three consents), `AccountSource`, `JudgeAccount`, `AccountError::{NoAccount, Unavailable}`, `Account::{OpenAi, Judge}`, `Consent` (with its `what` text as the key), `ConsentRequest` (`origin`, `via`, `consent`, `profile`, `functions`), `ConsentAnswer`; `Host::{withdraw_consent, account_changed}`; `judge_origin`; `InstalledMod.consents`; the reserved `<home>/app/accounts/` is the app's (3.1) |
| `dev` (step 11) | `Outcome.requests`, `RequestRecord`, the harness fakes of 19.1; `HOST_WIT` with the new interfaces; `watane.js` with the connection id; the `claude` argument and stdin rules of 19.3 for `fake-claude`; the account binding of 19.4 for `ScriptedResponses`; `Host::set_fake_claude` and `watane --preview .. --fake-claude <path>`; the feature `test-net` with `Host::set_test_net(TestNet { routes, root_pem })` (19.9) |
| the agent layer mods (W38) | the interfaces of 19.1 to 19.8; `caller.identity()` values; the grants `claude-code`, `claude-code-shell` |

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
World { exports, events, collects, calls, subscriptions: Vec<InterfaceName>, storage: bool, http: bool, manage: bool /* step 10 */ }
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

Step 11 (19): from `pkg`, `Grant::ClaudeCode` and `Grant::ClaudeCodeShell`
(text `claude-code`, `claude-code-shell`; JSON `{"kind":"claude-code"}`,
`{"kind":"claude-code-shell"}`; ordered after `Manage`; release CBOR `[5]`
and `[6]`; release grant limit 84) and `World` flags for the new imports and
the three exports, among them `World::host_exports` (pkg 4.5, 5.1, 7.2, 8).
The annotations of 19.5 come from WIT doc comments only (pkg question 21):
pkg adds no `[tools]` table. From `app`: `Confirm::consent` and
`Confirm::accounts` with an `AccountSource` (19.4, 19.7; app 19), which owns
the consent words, the consent view, Sign in with ChatGPT, the account files
`<home>/app/accounts/openai.json` and
`<home>/app/accounts/judge/<provider mod id>.json`, and the `claude` binary
setting. From `dev` (18): the programs `fake-claude` and `scripted-responses` for tests
(no watane crate links `watane-dev`, spec §2).

From `app`: an implementation of `Confirm` (from step 10 with `manage`
and `reduced`, app 17), and routes for `dist` under `/api/`. From `dist`: nothing directly.

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

- **Q23 correction (host)** Validator round 1 removes global call serialization.
  Per-emitter queues preserve event order; queued page batches preserve each
  mod's commit order. Invocations never wait for publication while holding
  a mod lock. Unrelated mods can execute concurrently.

- **Q24 (host, app, dev)** S12a fixes the Home app field and two-argument
  serve interface. A valid disabled-mod call returns 409; its page and
  events return 404. Calls keep compiled snapshots but read current grants
  at host imports and delivery. Lifecycle writes invalidate pending plans;
  an install checks generation again after consent and refuses stale plans.
  The synchronous folder helper cannot request app consent; async install
  owns that path. Provider uses pkg's object serde form, not its Display text.

- **Q25 (host)** S12b limits page event messages to events emitted by the
  page's own mod. Delivering a provider event invokes the subscriber but
  does not forward the raw event to its pages. State changes still send
  `changed`, once per changed mod in the completed chain.

- **Q26 (host)** Release packages may include source. Recovery uses
  `pack(dir, true)` for a release record when comparing current and `.old`
  folders with the committed package id. Files absent from a package remain
  absent after unpack, so this also matches a release without source.
  Recovery derives the publisher from the saved release and checks that
  its publisher and slug form the saved identity.
- S45: `HostError::AmbiguousProvider` has `consumer`, `provider` and `candidates` (distinct, from 2 mods up); 2.2 now shows this shape.
- S48: messages name a publisher by its 10-character fingerprint, not the full key; `candidates` are in natural order; the `ac5_extra_api...` test sends 403/421/404 with a small body and checks 413 over a raw socket (the guards still do not drain a refused body).

- **Q27 (S73, step 10)** Decided here: the manage functions name a mod
  by local name and a grant by a WIT variant that maps to pkg's `Grant`;
  reductions and requests apply when the manager's call succeeds, as
  events do; requests live in memory; one pending request per manager;
  the app, not the host, acts on an approved `enable`, `grant` and
  `remove`, as for its own buttons; a manager-started install always asks.
  A standalone pick of a provider is not a function: a pick arises inside
  an install request, through the app's pick view (app 9.7) with the
  manager named. A separate `request-pick` can join later as an added
  function.

Open for the human (S73). Each one is built so that either answer keeps
the interface of 6.10; the answer changes only which consent the app shows
or whether one is shown.

- **Q28** A removal that keeps the data. W35 lists "a removal that
  deletes data" as a request and does not list removal among the direct
  reductions. (a) Every removal is a request (built). (b) A removal that
  keeps the data is a direct reduction. Recommended: (a); a removal
  drops the package and its grants, which only a new consent brings back.
  Cost to undo (a) to (b): the app skips one consent; no interface or
  format changes. (b) to (a) later breaks managers that rely on it.
- **Q29** Enable. Turning a disabled mod back on makes things do more,
  so it is a request (built: `request-enable`). (b) Direct for a mod that
  this manager disabled itself, which needs a record of who disabled it.
  (c) Direct always. Recommended: (a); revisit if shells find it heavy.
  Cost to undo: (b) adds an in-memory or `installed.json` field; (c)
  drops a consent. Neither changes the WIT.
- **Q30** Answered (b) by the human, 2026-11-16; built in S82 (app 17.3, 17.4). Install from a channel that the home does not read yet. A link
  would make the app fetch a URL that the manager chose, which is a path
  to a host that no grant names. (a) Only a release that the home can
  fetch through the channels and bases it already reads (built; the app
  refuses others, app 17.3). (b) Any link, after a first consent "Desk
  asks to read <link>". Recommended: (a). Cost to undo: (b) adds one
  consent kind in the app; `request-install` keeps its arguments.
- **Q31** Answered (b) by the human, 2026-11-16; built in S82 as `open-page` (6.4, 6.10), carried by `Reduction::OpenPage` through `Confirm::reduced`. Open a page. A shell launches mods, but W35 does not list it.
  (a) Not in this step (built). (b) A direct `open-page(local)`: the app
  opens or focuses the mod window, and never while a consent request is
  open (app 5.1). Recommended: (b), in the slice that builds app stage J,
  if the human agrees: it gives no authority, only a window of a mod the
  participant installed. Cost: one added function in `manage` and one
  `Reduction`-like hook; existing managers are unaffected.
- **Q32** Answered (b) by the human, 2026-11-16; built in S82 (app 17.4). Asking again after a deny. One request is open at a time, but a
  manager may ask again at once; only withdrawing `Manage` stops it, the
  same bound that a local program has over `/api/install`. (a) No more
  (built). (b) The consent view offers "Deny and stop asks from <mod>",
  which withdraws `Manage`. (c) A pause of 60 s after a deny. Recommended:
  (b), in app stage J; it reuses `revoke`. Cost: one button and words in
  the app; the interface of 6.10 does not change.
- **Q33** Who uses a manager's grant. Any party that may call the manager
  (its page, an agent whose toolset names its functions, a mod with a call
  grant to it, a delivery) acts with its grant, as for every grant of a
  provider (6.10). (a) The host does not narrow it (built); the manager
  reads `watane:host/caller`, and the toolset view lists its functions.
  (b) The host allows manage functions only in a chain started by the
  manager's own page. Recommended: (a) for now, with the toolset view
  naming a manager's functions as such. Cost to undo: (b) is one check at
  each function in 6.10, but breaks a manager that an agent drives.

Answered by the human on 2026-11-21 (S95, agentic-harness `factory/PLAN.md`,
"Waiting on the human"; written into 19 by S97). Each was left out of 19 until
then; the questions stay here as the record, and the rule is in 19.1 (Q34) and
19.3 (Q35, Q36), built by stage A2.

- **Q34** Answered (a) by the human, 2026-11-21; but as a new interface
  `host-info` with `generation()`, not a function of `caller` (19.1); built in
  stage A2. `caller.boot()` (source row: "`caller.boot()`, a new watane host
  import", S17). The kernel cannot tell a restart from its own state, so it
  needs the host's start number. (a) `caller.boot() -> u64`: raised once each
  time the host starts, kept in host state, the same for every call until the
  next start; a pure read, no grant, not a request. (b) A host export
  `started()` called once per start into each mod. Recommended: (a); no new
  call into mods at start. Cost to undo: one line of 19.1, one row of 15, and
  the kernel's restart rule. Blocks only A2.
- **Q35** Answered (a) by the human, 2026-11-21 (19.3 step 4); built in stage
  A2. `outcome` on `ask-resolved {by: answered}` (source row: "`answered`
  carries `outcome`", S18). Without it the kernel cannot record what a checked
  provider decided (`allow`, `deny` or `none`). (a) The item carries
  `"outcome":"allow"|"deny"|"none"`, the answer the host sent to `claude`
  (`none` only for a check). (b) No field; the kernel learns the outcome
  another way at stage E. Recommended: (a). Cost to undo: one field. Blocks
  only A2.
- **Q36** Answered (option 1) by the human, 2026-11-21, with the `ask-resolved`
  and `expired` rules as the source writes them (19.3 steps 4 and 5); built in
  stage A2. The host-item rules of 19.3 step 4 and 5 (source row: "4.5 host
  items", S8, S22). Undecided until then: the item `ask-resolved` and its `by` values
  (`answered`, `expired` for an ask the host denied at its `expires-at`,
  `cancelled` for a `control_cancel_request`); whether the host also closes a
  check with `ask-resolved {by: expired}` at its `expires-at` and writes
  nothing to stdin for it; and what goes on the stream in place of a `claude`
  line typed like a watane item (`ask`, `check`, `ask-resolved`, `initialized`,
  `wrapped`, `consent-pending`, `verdict`, `checking`): option 1, the item
  `{"type":"wrapped","line":"<the line as a JSON string>"}`, also for a line
  that is not UTF-8, not one JSON object, or has a duplicate top-level key; 2,
  leave it to A2's slice; 3, a source marker on host items (a wire change in
  both repositories). Recommended: option 1 with the `ask-resolved` and
  `expired` rules as the source writes them. Cost to undo: a few lines here
  and one adapter field in the kernel. Blocks only A2.

Raised by S95 itself (not one of the three above):

- **Q37** Requests ride with the chain, writes do not (19.6; source:
  agentic-harness TECHNICAL 7, "applied only after the whole call chain
  commits", and its "A call commits whole", which assume that a chain commits
  as one). The host commits each invoke on its own success (Q3), so a nested
  call P that succeeds keeps its writes when its caller C later fails or
  defers, but (19.6, built) loses its requests and stream effects. A mod whose
  state records that it made a request (an `answer` sent, a session started)
  can then hold a record of an effect that never happened, and on redelivery
  must not trust it. (a) Built: requests and stream effects ride up the chain
  and are dropped with any rolled-back call above them; writes stay per invoke
  (Q3). A mod reached through a call link makes each request again on
  redelivery, or keeps its record of a request in the caller's chain. Keeps the
  boundary: a rolled-back call's `claude-session` request never reaches
  `claude`. (b) Requests per invoke like events (9.1): P's requests stay with
  P's writes. Consistent per mod, but a deferred `items` call whose nested
  provider call succeeded sends that provider's `say` or `answer` to
  `claude`, and the redelivery sends it again. (c) Chain commit for writes too
  (the source's model): one transaction held to the end of the chain, which
  Q3 refused for its locks and deadlocks. Recommended: (a), because it keeps
  the boundary of 19.3 and 14; its cost falls on the agent layer: a mod
  reached through a call link (the kernel under a client's `say`, a provider
  under the kernel) counts a request as made only when its effect shows (an
  item, an `ended`, a fired timer), never from its own state, or a client
  that fails after the kernel's `say` succeeded leaves an input recorded and
  never sent. Cost to undo: (b) is one line in 6.6 step 4 and 19.6, and a
  weaker 14 row; (c) rewrites 6.6 and 7. Does not block A1, which builds (a).

Raised by S95's adversary step (none blocks A1):

- **Q38** The key of a workspace or account consent names no provider and no
  grant (19.7; source row: agentic-harness TECHNICAL 8, "Kept for (origin
  client, folder)" and "(origin client, account)"). The words say "through
  {provider}", but the consent is kept without it. So a consent Alice gave
  for Notes through the checked provider, which has no shell, is reused
  without a question by the unchecked provider, by any mod with
  `ClaudeCodeShell` that Notes calls, or by a tool mod that Notes' agents
  call, as long as its chain carries Notes as origin client. Options:
  (a) as built, (origin client, folder) and (origin client, account): one
  question per folder, whatever runs there. (b) Add `via`, the identity of
  the mod whose request waits: (origin client, via, folder) and (origin
  client, via, account); a new provider asks again. (c) Add the grant kind
  only, `ClaudeCode` or `ClaudeCodeShell` (a shell consent covers a no-shell
  start, not the reverse); a checked and an unchecked provider still share a
  consent. Recommended: (b). The words already name the provider, and it
  also stops a mod in the middle that holds its own grant. It costs one more
  question each time the kernel's profile changes provider. Cost to undo:
  (b) to (a) drops a field of `Consent` and of the `what` text
  (`workspace:<via>:<folder>`), and every consent already given stays valid,
  since it is a subset of (a); (a) to (b) later would void every consent given
  under (a). The source row changes in agentic-harness too. Blocks A3; A2
  starts only in scratch workspaces and needs no answer.
  **Decided (b) by S103's foreman (2026-11-24), for the human to veto**: the
  recommended option, the stronger one, and the one that keeps (a) open at no
  cost (every consent given under (b) stays valid under (a)). 19.7 states it.
- **Q39** The Claude login and loopback under the shell grant (19.3, The
  run; source row: agentic-harness TECHNICAL 4.4, the environment, and 4.2,
  the shell grant). `CLAUDE_CODE_OAUTH_TOKEN` is in `claude`'s environment
  when the participant set it. If a sandboxed `Bash` child inherits it, an
  allowed word (`cat /proc/self/environ`, `printenv`) prints it to the
  consuming mod. `WebFetch` runs in `claude`'s own process, not in the Bash
  sandbox, so the rule "loopback denied" may not hold for it: it may reach
  watane's server on `127.0.0.1`, or a name that resolves to loopback. Neither can be
  shown on paper. Probe **V28**, named for the first A2 slice: (1) does a
  `Bash` child under the sandbox of 19.3 see `CLAUDE_CODE_OAUTH_TOKEN`, and
  can it read `/proc/<claude pid>/environ`; (2) does `WebFetch` reach
  `http://127.0.0.1:<port>/`, `http://[::1]:<port>/` and a name that
  resolves to `127.0.0.1`. Options: (a) as written, plus a deny of `Read`
  on `//proc/**` in `--settings=`, if V28 (1) shows only `/proc` reads. (b) Never
  pass `CLAUDE_CODE_OAUTH_TOKEN` to a start under the shell grant; `claude`
  then uses its own stored login, which the protected reads of 19.3 deny to
  its tools. (c) Drop `WebFetch` from the shell grant (or deny it on
  `localhost`, `127.0.0.1` and `[::1]` in `--settings=`) until V28 (2) shows that
  it honours the sandbox's network rules. Recommended: (b) and (c) until V28
  is recorded, then reopen. Cost to undo: (b) is one line of the
  environment rule; (c) is one row of the grants table. Blocks the shell
  grant in A2; the `ClaudeCode` grant does not wait.
- **Q40** Privacy of scratch and run folders between sessions (19.3,
  Folders; source row: agentic-harness TECHNICAL 4.6, "scratch folders per
  session", and the run folder of the same section). Every session's scratch workspace is
  under `<home>/scratch/` and every run folder under `<home>/runs/`. A deny
  rule cannot carve out the session's own folder, because in Claude Code a deny wins over
  an allow. So a `Glob` with a `path`, or a `ls` under the shell grant, can
  list and read another client's scratch work. The "allow outside the
  workspaces" refusal of 19.3 (Protected reads) covers only asks: not a
  read that `claude` makes without asking, nor the sandbox. Options: (a) a
  per-session parent outside the shared tree (for example
  `<home>/scratch/<random>/<random>/`, with the parent made 0711 so it cannot be
  listed). That hides the names, not the files. (b) Deny the whole of
  `scratch/` and `runs/` and run each session with its scratch folder
  outside the watane home (an OS temporary folder, created 0700 for that
  session); the run folder too. (c) Accept it, and add it to Not protected
  (spec §6). Recommended: (b), which keeps a deny rule simple. It is decided
  with the first A2 slice's scratch design (19.3, Folders). Cost to undo:
  the paths of 3.1 and 19.3, and the open-time cleanup of 3.1 step 3. Blocks
  A2's scratch slice.
- **Q41** Symlinks and the protected paths (19.3, Protected reads; source
  row: agentic-harness TECHNICAL 4.3). A cloned repository, or `ln -s` under
  the shell grant, can plant `F/k -> ~/.ssh/id_rsa` inside a workspace;
  `claude`'s `Read` of `F/k` is inside the workspace and asks nothing.
  Whether `claude` matches deny rules against the resolved target, or the
  path as given, is not probed; nor is the case where the home is reached
  through a symlink (macOS `/tmp`, `/var`). Options: (a) probe V29 in the
  first A2 slice and write each deny rule in both spellings if `claude`
  matches the path as given; (b) accept it under Not protected (spec §6).
  Recommended: (a). Cost to undo: deny lines only. Blocks A2.
  S100 (2026-11-22): the third rule of 19.3 (an `allow` to an ask of a file tool) resolves
  symlinks itself, also in a path that does not exist yet under a linked folder and in the
  working folder's own path, and refuses a link that cannot be followed; a path with `..`
  must stay inside both as the kernel reads it (from the link's target) and as Node's
  `path.resolve` reads it (the name before `..` dropped); so an ask for `F/k` is refused
  whatever `claude` matches. What stays open: a `Read` that `claude` makes without asking
  (an `allow` rule such as `Read(**)`) and the sandbox are held only by the deny rules,
  which is probe V29 (the two spellings); and a `Grep` or `Glob` of a folder inside, whose
  tree holds a link out, is allowed: whether `claude`'s search follows links is not
  probed (validator of S100).
  S100-fix1 closes the adversary's lookalike paths: the rule refuses a path with white space at an
  end or not in NFC, a `Glob` or `Grep` pattern with `{`, `}`, `,` or white space, and an
  `Edit`, `Write` or `NotebookEdit` into the run folder (19.3, "Read as `claude` reads it").
  Still open after S100's second adversary pass (for V29 or A3): a `Glob` pattern with `\`, `[`
  or `]` (`\.\./x`, `[/]etc/*`) passes the rule; whether Claude Code's glob engine reads such a
  pattern as leaving its base is not probed (refusing those characters is the fail-closed
  choice). On a case-insensitive file system, the run-folder write check compares bytes, so a
  workspace that holds the run folder could reach it by another case (picked folders refuse the
  watane home and its ancestors, so not reachable before A3). A picked folder whose canonical
  name is NFD (macOS) refuses every read in it (fail-closed; a usability gap for A3). Brace
  globs such as `**/*.{ts,tsx}` cannot be allowed.
- **Q42** Login files the closed deny list misses (19.3, Protected reads;
  HOST-F-74). Under the shell grant, a sandboxed runner word can read
  `~/.npmrc`, `~/.cargo/credentials.toml`, `~/.config/gcloud/**`, shell
  history and browser profiles: a deny cannot carve the workspaces out of
  `~`, because a deny wins over an allow (as Q40). Options: (a) deny the
  whole home and keep workspaces and scratch outside it (refuses picked
  folders under `~`, which most are); (b) keep the closed list and add the
  rest to Not protected for the shell grant, with HOST-F-74 narrowed to the
  listed files; (c) a longer closed list. Recommended: (b) with (c), since
  the shell grant's words already say "Not checked by watane". Cost to undo:
  the list and one FUNCTIONAL line. Blocks the shell grant in A2.
- **Q43** `say` content shape and normalisation (19.3, the `/` rule;
  source row: agentic-harness TECHNICAL 4.5). `say` takes a string, so "the
  first text block" is the whole string; if the host ever passes content
  blocks, a leading empty block before `/skill` escapes the rule. Whether
  `claude` applies NFKC before its `/` check is not probed. Options: (a)
  the rule applies to the first non-empty text block after the trim of
  19.3, and probe V30 checks NFKC lookalikes; (b) keep `say` a plain
  string forever. Recommended: (a). Blocks A2.

Decided by S95's user-sim step (a foreman writing A1's briefs from these
documents alone found each unwritten; each is inside the host and cheap to
undo, and none changes a WIT signature):

- **Q44** (decided) Who may act on a stream, and the sides that lack an
  export (19.2, Consumer of a mod stream, Acting side, Producer signals;
  19.6). A mod stream's consumer comes only from a call link, a top-level
  `Host::call` or the mod's own page, as agentic-harness TECHNICAL 3 says;
  an event delivery, a collect answer or an item call cannot make the emitter,
  collector or producer a consumer (`not-yours`), so a subscriber cannot fill
  the emitter's 32 streams. A consumer mod must export `stream-sink`
  (`open` and `hand-off` fail `not-yours`); a producer without
  `stream-producer` gets no signals; a mod without `timer-sink` gets no
  firing and its due row is deleted. `close` and `cancel`, which return
  nothing, ignore a wrong side or an unknown id. `ready` follows a `full`
  push only, and after a budget's `full` waits for that budget's room too.
  Cost to undo: a row each.
- **Q45** (decided) How the A1 tests reach a host stream source, a consumer
  that never takes, a clock and the writer's numbers: the feature
  `test-streams` (19.9), built like `test-net`, and the fixtures `latch` and
  `relay` of 13.1 (a `trap-once` mode needs a memory that survives a
  rolled-back call, which only a nested call's committed write gives, Q3).
  Deliveries and timer firings run on host workers started by `Host::open`
  (19.2, Driver). Cost to undo: test code only.
- **Q46** (decided by the foreman in S96-fix1, for the product owner to
  confirm) Which grant a mod stream depends on (19.2, Ends: "lost grant").
  The consumer's `Call` grant of the link through which `open` ran: when it is
  revoked, withdrawn or unbound, or its provider changes, the stream ends
  `gone` for both sides. A top-level `Host::call` of a mod has no link, so
  its streams depend on none; a page or a host stream depends on none until
  A2. Cost to undo: a field of the stream.

Raised by S96's adversary (fifth pass; open, for the product owner; neither
blocks A1):

- **Q47** (open) `remove` waits for the removed mod's call lock with no
  deadline while it holds the removal lock that every install takes (4.4), so
  a mod kept busy by its page, an agent or its timers delays the removal and
  every install on the host, each wait bounded by the call deadline but the
  lock not fair. Options: (a) `remove` waits with the call deadline and fails
  `busy`; (b) a removal takes priority over new callers of the mod; (c) keep.
  Recommended: (a). Cost to undo: one wait.
- **Q48** (open) A crash, or a failed redb write, between the catalog save of
  `remove` and its timer rewrite leaves rows that carry the removed identity
  (its own timers and other mods' timers with it as origin client); nothing
  sweeps them at start, and a later install with the same identity inherits
  them. Options: (a) at `Host::open`, rewrite or delete every row whose owner
  or origin client is not installed; (b) keep the install generation in the
  row (it lives in memory today, so it needs a persisted install id);
  (c) keep. Recommended: (a). Cost to undo: one pass at start.


Raised by S99's adversary (open; none blocks S99, which runs only in scratch
workspaces; each blocks A3's picked folders):

- **Q49** (decided (a) by S103's foreman, 19.3 Protected paths) The protected paths of 19.3 are relative rules, so Claude
  Code anchors them at the working folder only: an `--add-dir` workspace gets
  none, and `.git/**`, `.claude/**`, `.agents/**`, `.codex/**`, `.husky/**`
  match only at the top (`sub/.git/hooks/x` can be written). Harmless in a
  scratch workspace (`--restricted` loads nothing from it). Options: (a) one
  rule set per workspace root, as absolute `//<root>/...` rules, and `**/`
  forms for the folder patterns; (b) keep, and add nested folders to Not
  protected. Recommended: (a). Cost to undo: deny lines only.
- **Q50** (decided (a) by S103's foreman, 19.3 Folders) `session {resume | fork}` accepts any uuid; privacy rests on
  Claude Code keeping transcripts per working folder. Two origin clients that
  hold the same picked folder could resume each other's sessions. Options:
  (a) the host keeps the session ids each origin client started (a table like
  `scratch`) and refuses others `bad-launch {field: "session"}`; (b) accept it
  with the folder consent's words. Recommended: (a). Cost to undo: one table
  and one check.
- **Q51** (open) The protected reads name each root by its canonical spelling
  only; if `HOME` or the watane home is reached through a symlink, the
  spelled path is not denied. Safe only if Claude Code matches the resolved
  path (its rule matcher has an `asSpelledOnly` option, which suggests it
  matches both). Folded into probe V29 (Q41). Cost to undo: deny lines only.
## 19. The agent layer (step 11)

W38 (spec §7): a layer of mods runs on the host. The kernel `mods:agent` and
the two providers `mods:claude-code` live in the repository agentic-harness,
not here. The host gives them the interfaces below; pkg 5.1 gives the two
grants, app 19 the accounts and consents, dev 18 the fakes. Rules are ported
from agentic-harness `design/TECHNICAL.md` and cited "(agentic-harness
TECHNICAL n)". The human's answers there are cited `(N<n>, the human,
2026-11-20)`. The host is built in stages A1 to A5 (15). Three rules were
not decided in S95; the human answered them on 2026-11-21 (section 18, Q34 to
Q36) and 19.1 and 19.3 state the rules. S95's adversary step adds Q38 to Q43
(A2 and A3), still open. Text that depends on one says so. Q37 names where the request rule of 19.6 parts from the
source's, because the host commits each invoke on its own (Q3).

**Terms.** *Producer* and *consumer* of a stream (19.2). A *host stream* has
the host as producer (`claude-code.start`, `https-stream.start`,
`agent-tools.call`); a *mod stream* is opened by a mod with `streams.open`.
The *origin client* is the mod whose authority a chain carries: for a page
call, the mod of the page; for an agent call, the mod that owns the handle
(19.5); for an item call, a job call or a timer call, the origin client of the
call that opened the stream, job or timer; for an event delivery (9.2), and
for a collect answer (9.4) and every call nested in it, the subscriber
itself, never the emitter or the collector: the participant, not the
emitter, chooses the subscribers (F-RUN-6), so authority does not travel
along a subscription (S61). The host carries it with the chain, by the
mod's identity (pkg 4.1), not its local name. A *request* is a function with
an outside effect (19.6).

**The origin client must still be there.** At apply (19.6), and at each
firing of a timer, the host checks that the origin client is installed and
enabled with the identity it carried and the install generation it had (19.6,
At apply); a timer row stores the identity only (7), and its firing checks that. Else a request ends its stream `gone`,
and a timer fires with its own mod as origin client. The host checks when a
call starts, after the mod's call lock is taken, not before the wait: a call of
the host's own (an `ended` or `ready`, an item call, a firing) whose origin
client is gone runs with the called mod's own identity as origin client. An
identity alone does
not tell two installs apart (a local mod's is `local/<slug>`), so `remove`
rewrites the timers that carry the removed mod to their own mod (4.4), and a
later install with the same identity finds no timer that carries it. Each
request records the origin client of the call that made it, at that call:
inside a collect (9.4) that is the subscriber, and the collector's own later
requests keep the collector's origin client. Disabling or removing a
mod ends every stream and closes every handle whose origin client it is
(`gone`), whoever the producer and consumer are (S62).

### 19.1 Interfaces

All are `watane:host/<name>@0.1.0` in `wit/host.wit`, added to the package of
6.4 (the package version stays `0.1.0`, as for `manage`).

```wit
interface streams {                                  // no grant
  type stream-id = string;                           // 128 random bits as 32 hex chars
  record item { seq: u64, body: list<u8> }
  enum sink-outcome { taken, defer }
  record version-need { found: string, need: string }
  /// How a stream ended. A producer's `close` may pass only `closed`, `replaced`, `lagging`
  /// (the kernel's full owner stream, agentic-harness TECHNICAL 9.4) or `failed`; any other
  /// case ends the stream `failed("<case> is the host's outcome")`.
  variant outcome {
    closed, cancelled, lagging, replaced, consumer-failed, idle-timeout,
    gone,                       // disable, removal or update of either side, lost grant or consent, closed page connection, app quit
    mismatch(string),           // claude disagreed with the launch: the field (19.3)
    no-binary, version(version-need),
    consent-refused(string),    // what (19.7)
    no-app, no-account,         // 19.4
    failed(string),
  }
  enum stream-error { not-yours, unknown-stream, full, too-large }
  open: func() -> result<stream-id, stream-error>;
  push: func(s: stream-id, item: list<u8>) -> result<_, stream-error>;
  close: func(s: stream-id, outcome: outcome);
  cancel: func(s: stream-id);
  hand-off: func(s: stream-id) -> result<_, stream-error>;
  /// Consumer only: lets a deferred stream go on (19.2).
  resume: func(s: stream-id) -> result<_, stream-error>;
}

interface claude-types {                             // types only, no grant
  record bad-launch { field: string, reason: string }
  variant decision { allow(option<string>), deny(string), none }
  /// `none` only for a check (19.3). `allow(some(json))`: a changed input.
  variant start-error {
    bad-launch(bad-launch), destination, forbidden-header(string), too-large, full,
    not-owner, not-direct,   // the providers' (agentic-harness TECHNICAL 2.2, 10)
  }
  variant session-error {
    not-yours, unknown-stream, slash-command, unknown-ask, not-allowed-here,
    not-owner, not-direct,
  }
}
interface claude-code {                              // grant: ClaudeCode
  use claude-types.{start-error};  use streams.{stream-id};
  start: func(launch: string) -> result<stream-id, start-error>;
}
interface claude-code-shell {                        // grant: ClaudeCodeShell
  use claude-types.{start-error};  use streams.{stream-id};
  start: func(launch: string) -> result<stream-id, start-error>;
}
interface claude-session {                           // no grant; the caller is the stream's consumer
  use claude-types.{decision, session-error};  use streams.{stream-id};
  say: func(s: stream-id, content: string) -> result<_, session-error>;
  answer: func(s: stream-id, ask: string, d: decision) -> result<_, session-error>;
  interrupt: func(s: stream-id) -> result<_, session-error>;
  compact: func(s: stream-id) -> result<_, session-error>;
}
interface https-stream {                             // no grant; reaches only the hosts of the mod's Http grants
  use claude-types.{start-error};  use streams.{stream-id};
  start: func(request: string) -> result<stream-id, start-error>;
}
interface agent-tools {                              // no grant; a toolset needs consent (19.7)
  use streams.{stream-id};
  record tool { name: string, description: string, input-schema: string, parallel: bool, idempotent: bool }
  variant tools-error { not-granted, unknown-handle, unknown-tool, consent-pending(string), consent-refused(string) }
  open: func(toolsets: list<string>) -> result<string, tools-error>;
  describe: func(handle: string) -> result<list<tool>, tools-error>;
  call: func(handle: string, key: string, tool: string, args: string) -> result<stream-id, tools-error>;
  close: func(handle: string);
}
interface timers {                                   // no grant
  enum timer-error { too-many, bad-id, bad-delay }
  after: func(id: string, delay-ms: u64) -> result<_, timer-error>;
  cancel: func(id: string);
}

// host-only exports: a mod exports them to receive; no page, tool or mod can call them
interface stream-sink {
  use streams.{item, sink-outcome, outcome};
  items: func(s: string, items: list<item>) -> sink-outcome;
  ended: func(s: string, outcome: outcome);
}
interface stream-producer {
  use streams.{outcome};
  ready: func(s: string);
  ended: func(s: string, outcome: outcome);
}
interface timer-sink { fired: func(id: string); }

// the existing interface caller (6.4) gains:
//   identity: func() -> string;
```

`caller.identity()` is a pure read, no grant. It returns the identity of the
caller of the current call, compared exactly by mods:

| Caller | Value |
|---|---|
| a mod | its identity (pkg 4.1): `<publisher key>/<slug>`, or `local/<slug>` for a local mod |
| a page | `page` |
| an agent | `agent:<handle-name>` (19.5) |
| the host (an item call of a host stream, a fired timer) | `host` |

`who()` is unchanged for mods, pages and agents and returns `host` for the
host. A rename of a local name changes `who()` and never `identity()`. `Caller`
(2.3) gets no variant: the host's own calls use a crate-private caller.

**`host-info`** (Q34, answered by the human, 2026-11-21; built in stage A2,
S99). Restart detection is not a function of `caller` (nothing is added to it);
it is a new interface, no grant:

```wit
interface host-info { generation: func() -> u64; }
```

`generation()` is a pure read, no grant, not a request. The host raises it by
one at each `Host::open` and keeps it durably in host state (the `host` table
of 7, key `generation`; the first open of a home gives 1), so it never repeats
for one home, also after a crash. It is the same for every call, of every mod,
until the next open, so a mod that stored the number can tell a restart. The harness fake (2.6) returns 1. `host-info` is in `wit/host.wit` since stage A2 (S99).

Linking (rows of 6.4):

| Import | Linked to |
|---|---|
| `streams`, `claude-session`, `agent-tools`, `timers` | the functions of 19.2, 19.3, 19.5, 19.6 at every call; no grant to check |
| `host-info` (stage A2) | `generation()` of this section at every call; no grant to check |
| `claude-types` | types only |
| `claude-code`, `claude-code-shell` | each function first checks `Grant::ClaudeCode` / `Grant::ClaudeCodeShell` granted in the current catalog, else the call traps `not_granted` ("<local> has no grant for watane:host/claude-code"): the stub of W9 |
| `https-stream` | 19.4; the hosts are those of the mod's granted `Http` grants (6.8) |
| exports `stream-sink`, `stream-producer`, `timer-sink` | allowed export names (5.3); never a call: `no_function` on `/call` and in tools (6.6 step 2); only the host invokes them, as top-level calls with caller `host` (or the producer, 19.2) |

A grant revoked or a mod disabled stops the next host import of a running chain
(Q24) and ends the streams that depended on it (19.2, Ends).

**Harness** (2.6). The new imports are fakes: `streams.open` returns `s1`,
`s2`, ... in call order (one count for the host), every other function returns ok, `identity` returns
`local/<slug>` of the caller or `page`, and `host-info.generation` returns 1. Each request (19.6) of a succeeded
call is recorded in `Outcome.requests` as `RequestRecord { function, args }`
in call order; a failed call records none. A `Fakes` entry under
`watane:host/<interface>` overrides a result.

### 19.2 Streams (H1)

A stream carries numbered items from a producer to one consumer, in order.
(agentic-harness TECHNICAL 3.)

| Rule | Value |
|---|---|
| Consumer of a host stream | the mod whose request opened it |
| Consumer of a mod stream | the caller of the call in which `open` ran, only when that call came through a call link of the caller (a nested call, 6.6), was a top-level `Host::call` with `Caller::Mod`, or came from the mod's own page: a mod, or a page connection when the caller is `page` and the call carried a connection id (8.2). Any other caller (an agent, the host, a page call without a live connection) and a call that came by no call link (an event delivery, 9.2, or a collect answer, 9.4, where the caller is the emitter or collector; an item call, whose caller is the producer): `not-yours` (agentic-harness TECHNICAL 3: "only during a call through a call link from a mod or its own page"). A mod consumer must export `stream-sink`; else `open` fails `not-yours` |
| Acting side | `push` and `close` act for the mod whose instance calls them (the producer). `cancel`, `resume` and `hand-off` act for the consumer: a mod consumer when its own instance calls them; a page consumer when its mod's instance calls them in a chain whose caller is `page` and whose page connection (8.2) is that connection. `close` and `cancel` return nothing: a wrong side, an unknown or an ended id does nothing, as `timers.cancel` with an unknown id (19.6) |
| Page consumer | one `/events` connection, named by the connection id that `watane.js` sends with `/call` (8.2, 8.4, 8.5) |
| `push`, `close` | producer only; `cancel` consumer only; `hand-off` the consumer of a host stream (a mod stream cannot be handed off: `not-yours`), once, to the caller of the current call, which must be a mod (`mod:<local>`) that exports `stream-sink`: a caller `host`, `page` or `agent:<handle-name>`, or a mod without `stream-sink`, is `not-yours`, and the stream stays with its consumer; items queued and not yet taken go with it, numbering goes on |
| Wrong caller, unknown or ended id | `not-yours`, `unknown-stream` |
| Id | 128 random bits, never reused |
| Sequence | each item numbered from 1 per stream |
| Batch | every waiting item, up to 256 items or 1 MiB but always at least one item, one `stream-sink.items` call |
| Delivery | one call to a consumer mod at a time, in FIFO order of its queued batches; each `items` or `ended` call is a top-level call with fresh fuel and deadline (as 9.2 step 5) |
| Driver | delivery is asynchronous: `Host::call` returns after step 6 of 6.6 without waiting for any `items`, `ready` or `ended` call. The host runs deliveries and timer firings (19.6) on its own workers, started by `Host::open` in every mode (no `serve` needed) and holding a `Weak` of the host like the epoch ticker (6.3): dropping the last `Arc<Host>` stops them after the call in progress, so a test can open `state/state.redb` after `drop(host)` |
| Producer signals | a producer mod that does not export `stream-producer` gets no `ready` and no `ended`; its pushes still fail `full` |
| Failed batch | delivered once more with the same numbers; a second failure ends the stream `consumer-failed` |
| Deferred batch | an `items` call that returns `defer` is rolled back like a failed call (its pushes, writes and requests are dropped) but is not a failure: it does not count towards `consumer-failed`. The host keeps that batch and every later item, delivers nothing more on that stream, and a host stream stops reading its source, until the consumer calls `resume`. A stream that ends from the producer side while its consumer holds a deferred batch (the producer's `close` or `cancel`, or a host source that ends: `closed`, `cancelled`, `idle-timeout`, `failed`, `mismatch`, and so on) delivers the held batch and every item still queued, in order, after `resume`, and only then `ended(outcome)`; `ended` never overtakes a held item, and the items count against the budgets until delivered (agentic-harness S32, R7; stage A2 builds it, today only a producer's `close` delivers what is queued before `ended`). The other ends drop the held items and call `ended` at once, as built: `gone` (a side disabled, removed or updated, a lost grant or consent, a closed page connection, app quit), `lagging` of a page connection past its 16 MiB (Byte budgets), the consumer's own `cancel`, and `consumer-failed`; this keeps the consent rule (HOST-F-68): no item reaches a consumer after its consent is withdrawn. Backstop: a consumer that still has not resumed 5 minutes after the deferral gets its held items dropped and the stream ends `consumer-failed`, also when a producer end is pending |
| Resume | consumer only; the host then delivers the kept batch again, with the same numbers. `resume` of a stream that is not deferred does nothing. It takes effect with its chain (Commit); one made in the call that returns `defer` is dropped with it |
| Commit | `push`, `close`, `cancel`, `hand-off` and `resume` follow the chain rule of requests (19.6): they take effect after the top-level invoke of the chain succeeds and its writes commit, in call order, `close` and `cancel` after the pushes before them; a call that is rolled back, deferred or failed, and every call nested in it, drops them. The checks of the call (`not-yours`, `unknown-stream`, `full`, `too-large`) count what every chain in flight has queued, not only the current chain: a call checks and queues its effect under one lock of the host, and a chain releases its reservation when it applies or rolls back. So the in-flight limit, the budgets, the 32 open streams, the 256 live timers (their cancels count only in the chain that made them) and the single hand-off hold across chains running at once. `streams.open` is a request (19.6); a stream opened in a chain can be pushed to and closed in the same chain. A host stream that `claude-code.start` or `claude-code-shell.start` opened in a chain is visible to that chain's later stream and session calls as an `Open` is: its consumer, the starting mod, can `say`, `interrupt`, `compact`, `cancel` and `hand-off` it there, and a `cancel` of the chain starts no process |
| Item call's caller | `host` for host streams, `mod:<producer>` for mod streams |
| Hops | an item call that pushes on passes its hop count; at `limits.hops` (8) the item is dropped and logged |
| In flight | items queued and not yet taken, 256 items or 4 MiB per stream; `push` past it fails `full`; the producer's `ready` runs once when it drops below half after a `push` to it failed `full` (never without such a push); host streams stop reading their source; the host's `ask-resolved` items bypass the item and byte limit; their bytes are reserved in the budgets when the ask or check opens, 19.3 step 5 |
| Page queue | lossless, one per connection, apart from the page bus of 9.3. The host takes a batch for a page when its SSE write completes |
| Ends | close, cancel, `lagging`, `replaced`, disable, removal or update of either side, lost grant or consent (a mod stream depends on the consumer's `Call` grant of the link through which `open` ran, the link and its provider too: revoked, withdrawn (`manage.withdraw`) or unbound, it ends `gone` for both sides at once, after the catalog write (Q24, Q46), or at apply for an `open` of a chain still in flight (19.6, At apply); a side disabled and enabled again while that `open` was in flight keeps the stream, which did not exist at the disable; none for a page or a host stream until A2), closed page connection, `consumer-failed`, `idle-timeout`, app quit; each side gets `ended(outcome)` (`stream-sink.ended`, `stream-producer.ended`; a page gets the message of 8.4) |
| Forwarding | a mod that forwards defers when a `push` to its own downstream stream fails `full`, and resumes in its `stream-producer.ready` for that stream. When that downstream stream ends with any outcome (closed, `replaced`, `lagging`, page gone), `stream-producer.ended` runs and the mod resumes every deferred upstream of it |
| Lag clock | a stream's clock starts when a `push` to it fails `full` and stops and resets when the host calls `ready` for it or when a producer's push to it commits (19.3 step 5; an `ask-resolved` close item is no producer push and changes nothing); after 10 s on one run of the clock the stream ends `lagging` and both sides get `ended`. So a consumer that stops taking items, or takes a few and stops (a page that stays connected but stops reading), cannot hold its producer, nor the producer's deferred upstream, for longer |
| Deferral | a stream whose consumer has deferred it has no lag clock (a running one stops when the consumer defers; a new one starts only at a failed push after `resume`; its producer's pushes fail `full` because the consumer chose to wait). Backstop: a stream deferred for 5 minutes without `resume` ends `consumer-failed`. Neither needs a timer of the mod |
| Restart | streams are memory only; old ids are `unknown-stream` |
| Byte budgets | queued items (in flight and kept by a deferral), and the close reserve and kept input of each open ask or check (19.3 step 5), count against the host's budget of 512 MiB in all, 128 MiB per origin client (the consumer's 32 streams × 4 MiB), and 16 MiB per page connection. A `push` that would pass the origin client's or the host's budget fails `full`, as the in-flight limit does; its `ready` then waits until the stream is below half and the budget that refused it has room for the refused push again (Q44). A page connection past its 16 MiB ends; its streams end `lagging` |
| Limits | 32 open streams per consumer (`streams.open` and a host start past it fail `full`); 4 MiB per item (`too-large`); 4 `claude` processes per origin client, 8 in all (a start past it ends `failed("too many claude processes")`) |

A deferral is how a consumer says "not now" without losing order: the
consumer's call returns `defer`, the host holds the batch, the consumer calls
`resume` later. The page of a lagging stream reattaches from its last index
and recovers by replay (a rule of the kernel, agentic-harness TECHNICAL 9.4).

### 19.3 Claude Code (H2)

`claude-code.start` and `claude-code-shell.start` run the program `claude`
under the rules below. They are requests: they return a stream id at once; the
outcomes (consent refused, no binary, version too old, a mismatch) arrive as
`ended` (19.6). (agentic-harness TECHNICAL 4.)

**The launch document.** JSON with `"version": 1`, parsed into a Rust type
that refuses unknown fields. A value outside its grammar is `bad-launch
{field, reason}` at the call. Each value goes out as one `--flag=value`
argument.

| Field | Grammar or rule | Goes to `claude` as |
|---|---|---|
| `workspace` | a workspace id the origin client holds (19.7), or `scratch` for a new scratch workspace (Folders) | cwd |
| `extra-workspaces` | workspace ids, the same rule; `scratch` is refused here | `--add-dir=` each |
| `model` | `[A-Za-z0-9.\-\[\]]{1,64}` | `--model=` |
| `fallback-models` | the same, at most 4 | `--fallback-model=` |
| `effort` | `low`, `medium`, `high`, `xhigh`, `max` | `--effort=` |
| `system-prompt` | `{replace \| append: text}`, at most 64 KiB | `--system-prompt=` / `--append-system-prompt=` |
| `tools` | names from the grant's allowlist (below); an absent `tools` and `[]` both mean no built-in tools | `--tools=` |
| `allow`, `deny` | `Name` or `Name(arg)`, `arg` without parentheses, no control character anywhere; Name from the allowlist or `mcp__watane__<tool>`; no comma, space or leading `-` outside the parentheses; an allow `Bash(…)` follows the grammar below | `--allowedTools=`, `--disallowedTools=` each |
| `mode` | `manual`, `accept-edits`, `plan`, `dont-ask` | `--permission-mode=` with Claude's names: `default`, `acceptEdits`, `plan`, `dontAsk`; `system/init` has `permissionMode` in the same names (an absent `mode` reads `default`) |
| `ask` | bool | `--permission-prompts=host --permission-prompt-tool=stdio`, or `--permission-prompts=none` |
| `pre-tool-check` | `{timeout-s: 5..120}` | the host's PreToolUse callback in `initialize` |
| `agent-handle` | 32 lowercase hex characters, a handle the origin client holds (19.5): else `bad-launch {field: "agent-handle"}` at the call (a handle opened in the same chain is not held yet) | `--mcp-config=/dev/fd/3` |
| `subagents` | each exactly `{name [a-z0-9-]{1,64}, description, prompt, tools ⊆ tools, model}`; any other key is refused by name; 64 KiB in all (N9, the human, 2026-11-20: subagents stay `--agents=` flags) | `--agents=` |
| `subagent-depth` | 0..3, default 1; 0 also removes `Task` | env `CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH` |
| `skills` | `{name [a-z0-9-]{1,64}, files}`; paths under the skill folder, no `..`, 1 MiB in all; a `SKILL.md` that starts with `---` has strict frontmatter, below | the run folder's plugin, `--plugin-dir=` |
| `sandbox` | the typed record of the grants below; required with the shell grant | the host's `--settings=` |
| `json-schema` | at most 64 KiB, valid JSON Schema | `--json-schema=` |
| `max-turns` | 1..1000 | `--max-turns=` |
| `max-budget-usd` | positive | `--max-budget-usd=` |
| `session` | `{new: uuid}`, `{resume: uuid, at?: uuid, drops-turn?: uuid}`, `{fork: uuid, at?: uuid}` | `--session-id=`, `--resume=`, `--fork-session`, `--resume-session-at=`, `--resume-drops-turn=` |
| `persist` | bool | `--no-session-persistence` when false |
| `partial` | bool | `--include-partial-messages` |
| `profile` | optional, `[A-Za-z0-9._-]{1,64}`: the name of the kernel's profile that built the launch (agentic-harness TECHNICAL 9.2) | nothing: only the words of the workspace consent (19.7, `ConsentRequest.profile`); it grants and checks nothing |

The allow grammar for `Bash` (deny rules are not held to it). An allow
`Bash(<arg>)` is refused unless all of these hold:

- Bare `Bash`, with no parentheses, is refused: it allows any command; so is
  an empty `<arg>` (`Bash()`, `Bash(:*)`).
- `<arg>` has no non-ASCII character and none of `` ` ``, `$`, `{`, `}`, `;`,
  `|`, `&`, `<`, `>`, `'`, `"`, `\`: the shell would read more than a word.
- A colon appears only as one trailing `:*`.
- The first word (split on whitespace, after that `:*` is removed) is no path
  (no `/`), has no `~`, and is made only of `A-Za-z0-9._+-`.
- The first word, lowercased, is not an interpreter or runner: no name
  starting `python`, `pypy`, `node`, `perl`, `ruby`, `php` or `lua`
  (versioned names such as `python3.12`, `nodejs`, `perl5.36.0`, `luajit`),
  and none of these, because each runs, wraps or schedules another command:
  `.`, `ash`, `at`, `awk`, `bash`, `batch`, `builtin`, `bun`, `bunx`,
  `busybox`, `byobu`, `capsh`, `chroot`, `chrt`, `command`, `coproc`,
  `corepack`, `crontab`, `csh`, `dash`, `deno`, `doas`, `dtach`, `elixir`,
  `enable`, `env`, `eval`, `exec`, `expect`, `fakeroot`, `fc`, `fish`,
  `flock`, `gawk`, `gdb`, `ionice`, `irb`, `julia`, `ksh`, `lldb`, `ltrace`,
  `lua`, `mawk`, `mksh`, `nawk`, `nice`, `node`, `nohup`, `npx`, `nsenter`,
  `numactl`, `osascript`, `parallel`, `perl`, `php`, `pipx`, `pkexec`,
  `pnpx`, `prlimit`, `pwsh`, `r`, `rbash`, `rscript`, `ruby`, `run0`,
  `runuser`, `screen`, `script`, `sem`, `setpriv`, `setsid`, `sg`, `sh`,
  `source`, `ssh`, `stdbuf`, `strace`, `su`, `sudo`, `systemd-run`,
  `taskset`, `tclsh`, `tcsh`, `time`, `timeout`, `tmux`, `toybox`, `trap`,
  `ts-node`, `tsx`, `unshare`, `uvx`, `valgrind`, `watch`, `xargs`,
  `yarnpkg`, `zellij`, `zsh`. This list is the rule; the kernel's own copy
  must equal it. No list of this kind is complete, so a word not on it is
  not thereby safe: the shell grant's sandbox is the limit (section 14, Not
  protected). `find` is allowed only with no `*` and no `-exec`, `-execdir`,
  `-ok` or `-okdir`.
- A package runner (table) needs a second word from its row and no option
  word (starting `-`) after it, and the rule has no `*` or `:*` anywhere
  (fail-closed: an option or a forwarder can name a program to run).

| Runner | Second words allowed |
|---|---|
| `npm` | `install`, `i`, `ci`, `test`, `ls`, `list`, `audit`, `outdated` |
| `pnpm` | `install`, `add`, `test`, `list`, `ls`, `audit`, `outdated` |
| `yarn` | `install`, `add`, `test`, `list`, `audit`, `outdated` (not `ls`: yarn runs a script named `ls`) |
| `uv` | `pip`, `sync`, `lock`, `add`, `remove`, `tree`, `venv` |
| `devbox` | `install`, `add` |

The `skills` frontmatter rule, for each file of a skill named `SKILL.md` in
any case and any folder. Frontmatter starts with `---` and a newline at byte 0
(leading white space or a byte order mark before `---` is refused). It ends
at a line `---`. Its keys are `name` and `description`, each once; an indented
line continues the key above it and may not contain `:`; any other line is
refused. Every character is printable ASCII (space to `~`). Each refusal
names its reason.

**Grants and tools** (N1, the human, 2026-11-20: two grants):

| Grant | Built-in tools | Required |
|---|---|---|
| `Grant::ClaudeCode` (`claude-code`) | `Read`, `Write`, `Edit`, `Glob`, `Grep`, `NotebookEdit`, `Skill`, `Task`, `TodoWrite` | a workspace |
| `Grant::ClaudeCodeShell` (`claude-code-shell`) | the above and `Bash`, `WebFetch`, `WebSearch` | a workspace; `sandbox.enabled: true`; writes only in the workspaces; loopback denied; none of `excludedCommands`, `allowUnsandboxedCommands`, `allowUnixSockets`, `allowLocalBinding`; a network allowlist of plain domain names |

**Protected paths.** The host's `--settings=` denies `Edit`, `Write` and
`NotebookEdit` on Claude Code's own protected list and on `.git/**`,
`.claude/**`, `.agents/**`, `.codex/**`, `**/.aws/**`, `**/CLAUDE.md`,
`**/AGENTS.md`, `.mcp.json`, `.envrc`, `**/.github/workflows/**`, `build.rs`,
`package.json`, `Makefile` and `.husky/**`. No launch can remove them.
Each rule is written once per workspace root (the working folder and each
`--add-dir`) as an absolute `//<root>/...` rule after canonicalization, and the
folder patterns also in their nested form (`//<root>/**/.git/**`,
`//<root>/**/.claude/**`, `//<root>/**/.agents/**`, `//<root>/**/.codex/**`,
`//<root>/**/.husky/**`), so a workspace added with `--add-dir` and a nested
`sub/.git` are covered (section 18, Q49, option (a), decided by S103's foreman).

**Protected reads** (S63; open: Q42 login files outside the list; Q41: the third rule resolves symlinks). A file in the participant's home folder is as
readable to `claude` as to the participant, and the mod that consumes the
stream reads whatever the agent prints; so the workspace refusals below do
not keep a credential from a mod on their own. Three rules do:

- The host's `--settings=` also denies `Read`, `Glob`, `Grep`, `Edit`,
  `Write` and `NotebookEdit` (so, under the shell grant, the sandbox's reads
  and writes too) on: the watane home's `app/`, `dist/`, `log/`, `net/`,
  `state/`, `installed.json` and `toolsets.json`; `~/.claude/**`,
  `~/.claude.json`, `~/.codex/**`, `~/.ssh/**`, `~/.aws/**`, `~/.gnupg/**`,
  `~/.netrc`, `~/.git-credentials`, `~/.config/gh/**`, `~/.docker/**`,
  `~/.kube/**` and `~/Library/Keychains/**`, each as an absolute `//` rule
  after canonicalization. No launch can remove them.
- An `allow` rule on `Read`, `Glob`, `Grep`, `Edit`, `Write` or
  `NotebookEdit` (deny rules only narrow) is refused `bad-launch {field: "allow"}`
  unless it has an argument and the argument is a relative pattern (not empty and not exactly
  `*`: Claude Code reads `Read()` and `Read(*)` as the bare `Read`): it does
  not start with `/`, `~` or `//`, and has no `..` segment. A bare name
  (`Read`, `Write`) is refused: it matches every path, so `claude` would read
  or write outside the workspaces without an ask, and the third rule would
  never see it; `Read(**)` is the way to allow the whole workspace.
- An `answer` `allow` to an `ask` whose tool is one of those six and whose
  path input (`file_path`, `path`, `notebook_path`; absent means the
  working folder) is not inside one of the launch's workspaces or its run
  folder, or whose pattern input (`Glob`'s `pattern`, `Grep`'s `glob`)
  starts with `/` or `~` or has a `..` segment, after the
  host resolves it as `claude` would (relative to the workspace, symlinks
  resolved; only a component that does not exist counts as a new name, and
  any other error while resolving, such as a file used as a folder, a loop or
  no permission, refuses it: review r2 of S100, F2.1), is refused `not-allowed-here`; nothing is written to stdin, and
  the ask is denied at its `expires-at`.
  Read as `claude` reads it, fail-closed (S100's adversary; foreman's
  decision): a path input with leading or trailing white space (what
  JavaScript's `trim` removes) or that is not in Unicode NFC is refused, so
  the host never resolves another name than `claude` does; a `Glob` `pattern`
  or `Grep` `glob` holding `{`, `}`, `,` or white space is refused (Claude
  Code splits a `glob` into several and expands braces). The run folder counts
  for `Read`, `Glob` and `Grep` only: an `allow` of `Edit`, `Write` or
  `NotebookEdit` there is refused, so a consumer cannot plant a skill file
  past the `skills` frontmatter rule. The path is checked at the call; a link
  swapped before `claude` opens the file is section 14, Not protected (the
  deny rules of `--settings=` stay the hard limit).

**The run.**

- Fixed arguments: `-p --input-format=stream-json --output-format=stream-json
  --verbose --restricted --strict-mcp-config --system-prompt-snapshot=off` and
  the host's `--settings=`. Never `--safe-mode`, `--bare`, `--plugin-url`,
  `--worktree`, `--bg`, `--cloud`, `--remote-control`, `--chrome`, `--ide`,
  `--file`, `--betas`, `auto` or a bypass mode (N2, the human, 2026-11-20:
  `auto` stays out).
- Binary: the path from `AccountSource::claude_binary` (19.4), the one the
  participant picked, at or above the minimum version: the first that answers
  `initialize` with `hook_on_failure_supported` and has the protected-path
  symlink fixes (2.1.295 or later). None: the stream ends `no-binary`; too old:
  `version {found, need}`; no app: `no-app`.
- Process: its own process group; the pid is recorded in `<home>/runs/pids` with the boot id and its start time (3.1 step 3); the record is removed when the run ends.
  The consent of 19.7 is held first: the host starts the process only then.
- Environment: only `HOME`, `USER`, `LOGNAME`, `LANG`, `LC_*`, `TMPDIR`,
  `SHELL`, a fixed `PATH` (`/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin`), `CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH`, and
  `CLAUDE_CODE_OAUTH_TOKEN` when the participant set it. The host never reads
  that token. Whether a sandboxed `Bash` child can read it, and
  whether `WebFetch` reaches loopback, is section 18, Q39 (probe V28).
- MCP: with an `agent-handle`, `{"mcpServers":{"watane":{"type":"http","url":"http://localhost:<port>/mcp/agent","headers":{"Authorization":"Bearer <token>"}}}}`
  written to a pipe that the child inherits as fd 3 (19.5 for the token); the run adds
  `--mcp-config=/dev/fd/3`. The crate forbids `unsafe`, so the pipe comes in as the child's stderr and
  `/bin/sh -c 'exec "$0" "$@" 3<&2 2>/dev/null' <claude> <arguments>` moves it to fd 3 and
  execs `claude` (the same pid, so the process group, the record in `runs/pids` and the
  kills are those of `claude`; the program path is `$0`, never part of the script). The run
  waits for the handle's consents first, and puts `consent-pending` on the stream once per
  consent that waits; a refused one ends the stream `consent-refused(what)`. A host that
  serves no port ends it `failed`. No handle: no pipe, stderr is `/dev/null`, `claude` is
  started directly. The token is in the pipe and in no argument, item or file.

**The control protocol.**

1. The host writes `initialize`. With `pre-tool-check` it holds
   `hooks.PreToolUse = [{matcher: "*", hookCallbackIds: [<id>], timeout:
   <timeout-s>, onFailure: "block"}]`.
2. With a callback, the answer must hold `hook_on_failure_supported: true` and
   not `hooks_applied: false`; otherwise the host kills the group and ends the
   stream `mismatch(hooks)`. Once this step has passed, the host pushes the item
   `{"type":"initialized","workspace":"<the launch's workspace id>"}` on the
   stream (Folders). The process starts only after the
   workspace consent is held, so `initialized` never comes before it; the
   checked provider relies on that. Before the answer only control lines may come, and none
   is pushed: any other line kills the group and ends the stream `mismatch "init"`, so
   `initialized` is the first item.
3. The `system/init` line must match the launch in tools, MCP servers, agents,
   permission mode (`manual` reads `default`) and plugins (exactly
   `watane-run` and Claude Code's `cc-plugin-*`). Skills are not compared. A
   difference: kill, `mismatch {field}`, the field named as `system/init` names
   it: `tools`, `mcp_servers`, `agents`, `permissionMode`, `plugins`. The lists hold
   names, as strings or as objects with a `name` (Claude Code's `{"name","path"}`
   plugins and `{"name","status"}` servers). Only an entry that the launch did not
   ask for is a difference (a tool not in the launch's `tools`, an MCP server other than the
   host's `watane` of an `agent-handle`, an agent neither in `subagents` nor one of Claude Code's own, another plugin);
   a missing list holds nothing, a missing or other `permissionMode` differs. Every `system/init` line is checked,
   and then passed on as an ordinary line after `initialized`. The first line after the answer
   to `initialize` that is not a control line (`control_request`, `control_response`) must be a
   valid `system/init` (UTF-8, one JSON object, no duplicate key); any other line there, wrapped
   or not, kills the group and ends the stream `mismatch "init"`.
   Claude Code's own agents are the built-in `agentType`s of 2.1.330: `general-purpose`,
   `statusline-setup`, `Explore`, `Plan`, `claude-code-guide`, `claude`, `fork`, `worker`,
   `comment-thread-analyst` (`src/agent/claude/init.rs`). Fail-closed: a user's or a
   project's agent, or a built-in one that a later version adds, is `mismatch(agents)`
   until it is named here (validator of S99).
4. Then each stdout line becomes an item:

| Line | Item |
|---|---|
| `control_request` `can_use_tool` | `{"type":"ask","id":<host id>,"tool_use_id":<the request's>,"tool":…,"input":…,"expires-at":<ms>}` |
| `control_request` `hook_callback` (PreToolUse) | `{"type":"check","id":<host id>,"tool_use_id":<the request's>,"tool":…,"input":…,"expires-at":<ms>}` |
|  | (review r1 of S100) An `ask` or a `check` of `Read`, `Glob`, `Grep`, `Edit`, `Write` or `NotebookEdit` also carries `"path":"<absolute path>"` after `input`: its path input (absent: the working folder) resolved as the third rule of Protected reads resolves it (symlinks resolved), taken from the key that tool reads (`Read`, `Edit`, `Write`: `file_path`; `Glob`, `Grep`: `path`; `NotebookEdit`: `notebook_path`); an input that also holds another of those keys gives no `path` (S100's third adversary pass); an absent key means the working folder only for `Glob` and `Grep`, and gives no `path` for the other four (S100's fourth pass: `claude` refuses those inputs, so a workspace-wide scope must not be offered), so the kernel can scope a session allow before it answers (agentic-harness TECHNICAL 9.6 and 15; the H2 wire form, agreed by the deputy on PR #15); a path the host cannot resolve, or that the rule refuses, gives no `path` key. `input` stays as `claude` sent it |
|  | (S100's foreman) The host id is a decimal counter from `"1"` per stream, one count for asks and checks. An ask's `tool` and `input` are the request's `tool_name` and `input`; a check's are its `input.tool_name` and `input.tool_input`; `tool_use_id` is the request's `tool_use_id` for both. A `tool` that is not a string or an `input` that is not an object is answered deny at once and pushes nothing, as a missing `tool_use_id` is |
| a line whose top-level `type` is a watane item type that can share the stream: `ask`, `check`, `ask-resolved`, `initialized`, `wrapped`, `consent-pending`, and the checked provider's `verdict` and `checking` | never passed through (Q36, option 1): the line goes on the stream as `{"type":"wrapped","line":"<the line as a JSON string>"}`, so that no `claude` line can pass as a host item |
| a line that is not UTF-8, is not one JSON object, or has a duplicate top-level key | `{"type":"wrapped","line":"<the line as a JSON string>"}`, the bytes decoded with U+FFFD for the invalid ones |
| `control_cancel_request` | never pushed. When its `request_id` is that of an open ask or check, the host closes it with `ask-resolved {by: cancelled}`; any other cancel does nothing (S100's foreman) |
| any other line | the line unchanged |

   `expires-at` is on the host's clock: for an ask, the push time plus 10
   minutes; for a check, the push time plus the hook's `timeout-s`. A
   `control_request` whose `tool_use_id` is missing, not a string, or empty is
   not passed as an ask or check: the host answers it deny at once and pushes
   nothing for it. The host's `id` of an `ask` or `check` is unique on its
   stream and never reused. The host builds each `ask` and `check` item from
   its own parsed value and serializes it itself; it never copies bytes of a
   `claude` line into an item, so no `claude` line can pass as a host item. An
   ask or check closes with one item, built by the host as well:
   `{"type":"ask-resolved","id":"<the id of the ask or check>","by":<by>}` (the key is `id`, as agentic-harness TECHNICAL 4.5 and 9.4 read it; review r1 of S100), with
   `by` one of: `answered` (a `claude-session.answer` was written, and the item
   also carries `"outcome":"allow"|"deny"|"none"`, the answer the host sent to
   `claude`; `none` only for a check, Q35); `expired` (an ask that the host
   denied at its `expires-at`, step 5; or a check that reached its
   `expires-at`: the host closes it, and writes nothing to stdin for it);
   `cancelled` (`claude` sent a `control_cancel_request` for it, or the
   consumer's `interrupt` ended its turn, step 5). Each ask or
   check gets exactly one `ask-resolved`, and a later answer to it is
   `unknown-ask` (Q36).
5. Writes to stdin are only these; after `initialize`, none is written until the answer to it has passed step 2 (not until `system/init`), then they go in queue order, and none if step 2 fails:

| Operation | Written | Rule |
|---|---|---|
| `say` | a `user` message with the content | refused `slash-command` when the first text block starts with `/` after white space; white space here is every character that JavaScript's `trim` removes (so also U+FEFF) and every character of the Unicode categories Z, Cc and Cf (so also U+200B), not Rust's `char::is_whitespace` alone (S64) |
| `compact` | a `user` message `/compact` | – |
| `answer` to an ask | `control_response` `{behavior: allow, updatedInput}` or `{behavior: deny, message}` | `none` refused `not-allowed-here`; `allow(none)` sends the original input; a changed input only for `AskUserQuestion` and `ExitPlanMode`, else refused `not-allowed-here` |
| `answer` to a check | `{}` for `none`; `{hookSpecificOutput: {hookEventName: PreToolUse, permissionDecision: deny, permissionDecisionReason}}` for `deny` | `allow` refused `not-allowed-here` |
| `interrupt` | `control_request` `interrupt` | first, a deny (`message` `interrupted`) for each open ask of the stream, nothing for an open check; each open ask and check then closes with `ask-resolved {by: cancelled}`, in the order they were pushed, before the stream's next item (S100's foreman) |

   An ask the host has not had answered at its `expires-at` is denied (written
   as the deny above) and closes with `ask-resolved {by: expired}`. A check
   that is not answered at its `expires-at` is closed with `ask-resolved
   {by: expired}` and nothing is written to stdin for it. A second
   answer, or an answer to an ask or check that is closed, is `unknown-ask`.
   No ask or check is opened before the `system/init` line has passed step 3
   (review r1 of S100): a `can_use_tool` or `hook_callback` that comes before it is
   answered deny at once and pushes nothing. Before the answer to `initialize` has
   passed step 2 nothing is written to stdin, so the deny for a request that comes
   then is queued and written first, before any other line, once step 2 passes; if
   step 2 fails, the stream ends and nothing is written (review r2 of S100, F2.8; the
   deputy's ruling). An ask or check whose `expires-at` has
   come is closed at that instant, whether or not the host's sweep has run: an
   `answer` at or after it is `unknown-ask` at the call, and one whose apply comes at
   or after it writes nothing (the expiry's deny, for an ask, is written once). A
   `control_cancel_request` or an `interrupt` that acts at or after an ask's or
   check's `expires-at` finds it expired first: the ask gets its single expiry deny
   and `ask-resolved {by: expired}`, never the interrupt's deny or `cancelled`
   (review r2, F2.2). An
   `ask-resolved {by: answered}` is pushed only after its line was written to
   `claude`'s stdin; a write that fails (stdin closed) pushes no `answered`, and
   the stream ends as the process does. The end of a run is bounded (review r2,
   F2.3 and F2.4): once `claude`'s stdout reaches its end, the host waits at most
   2 s for the process to exit and then kills its group; it then closes the
   writer's queue and waits at most 2 s more for the stdin writer. The host's write
   end of `claude`'s stdin is non-blocking: a write that finds the pipe full waits for
   room in short slices and gives up at that second bound, so a stray process that
   keeps the pipe open cannot hold the writer. The bound is checked before every
   write call, whatever the last one returned (a partial write, `EINTR`, the next
   queued line), so no write starts after it (review r3, F3.1). If the host cannot
   make its write end non-blocking, the start ends `failed("stdin")` before any line
   is written (fail closed). The `ask-resolved` items that close asks and checks are
   host items, few and small (one per ask or check), and wait on neither the stream
   nor a budget: they go on the stream past its in-flight limit, in order, and the
   bytes of each (128, the most one can take) are reserved in the origin and host
   budgets when its ask or check opens, which is refused `full` when item and reserve
   do not fit; the close takes the reserve, and the stream's end frees what is left.
   So the end of a run never waits on a consumer that stopped reading or on a budget
   (review r3, F3.2). The budget check, the enqueue and the reserve of an ask or
   check are one step under one lock: a stream that ends meanwhile takes nothing
   from the budgets. While an ask or check is open, the input the host keeps of it
   counts against its origin client's and the host's byte budgets, beside its close
   reserve, and is freed when it closes or the stream ends; at most 64 asks and
   checks are open on one stream at a time, and a `can_use_tool` or `hook_callback`
   past that is answered deny at once and pushes nothing (fail closed; S100's sixth
   adversary pass). One whose item, close reserve and kept input do not fit those
   budgets is not denied: its open fails `full` and the run stops reading `claude`
   until it fits, as every host push waits (19.2, In flight); the lag clock bounds
   that wait, so a stream whose budget stays held ends `lagging` (19.2, Lag clock).
   A refusal and its wake come from the one admission check (review r6, F6.1 and
   F6.2; review r7, F7.1: the scan calls the same function as a retry with the
   same arguments, the stream's own pending items and bytes included, so the two
   cannot disagree; the deputy's ruling): whether a waiting producer is woken is decided by
   that check, run under the same lock, on the current pending bytes of every chain
   in flight, against both the origin's and the host's budget, so a woken producer's
   retry is admitted. The scan's admission answer equals a retry's for every state; the
   19.2 In flight rule (`ready` once the stream is below half) then gates the wake. A stream's refusal and lag state change only when a push to it
   commits (its refusal then clears and its lag clock stops); a tentative push in a
   chain that rolls back leaves them exactly as they were. A host close item
   (`ask-resolved`) is no producer push: it neither clears a refusal nor stops
   the lag clock. When the check cannot run at once because the chains gate or a
   chain's log is held, the wake is owed, not dropped: the hub's own worker runs
   the check as soon as it can take the gate (holding no other lock first), so a
   drained stream's producer is woken within that wait, never only at the
   holder chain's end (review r6, validator). A close and a chain's end
   (commit or rollback) run that check for the waiting producers (review r5 and r6). The input an open ask or check keeps is held as the bytes of its JSON
   text, not as a parsed value, and counted by that length, so the count is what
   the host holds; it is parsed again when needed (S100-CAP). When `claude` exits,
   the waiter kills what is left of its group while the exited process is not yet
   reaped (its pid still held), clears the recorded group, and only then reaps it;
   every other kill of the group (`stop()`, the end of the run) goes through that
   record under one lock, so no kill follows the reap and a pid reused since is
   never killed (S100-CAP). The same holds for the version probe (`claude
   --version`): its group is killed, if at all, before its child is reaped, never
   after (S100-CAP, item 5).
   Every line the writer reported written
   has its `ask-resolved {by: answered}` pushed before `ended`; a write that did not
   complete (failed, or given up) pushes none, and none is pushed after the stream
   ended. No thread of the run outlives the bounds, but for the one that reaps
   `claude` if the process is still not reaped 2 s after its group got SIGKILL (a
   process stuck in the kernel).
   The checks of an `answer` are made at the call against the asks open then
   (`unknown-ask` for an id that is not open on `s`); one that closes between the
   call and its apply (expired, cancelled, a second answer applied first) writes
   nothing and pushes nothing more. An ask's `permission_suggestions` and every
   other field of `claude`'s request but `tool_name`, `input` and `tool_use_id`
   stay out of the item; an `allow` never carries `updatedPermissions`.
   The items for a close are those of step 4. Nothing else is ever written: no
   `apply_flag_settings`, `mcp_set_servers`, `set_permission_mode`,
   `updatedPermissions` or second `initialize`.
6. A turn is over when `claude` reports itself idle, not at the first
   `result`.
   That line is `{"type":"system","subtype":"session_state_changed","state":"idle"}`;
   the host passes it unchanged.

**Wire shapes** (Claude Code's stream-json, as S99 builds them; dev's
`fake-claude` checks the same). `initialize`:
`{"type":"control_request","request_id":"<id>","request":{"subtype":"initialize"}}`
(with `hooks` when the launch has `pre-tool-check`); its answer
`{"type":"control_response","response":{"subtype":"success","request_id":"<id>","response":{…}}}`.
`say`: `{"type":"user","message":{"role":"user","content":"<text>"}}`;
`compact`: the same with the text `/compact`; `interrupt`:
`{"type":"control_request","request_id":"<id>","request":{"subtype":"interrupt"}}`;
the host's answer to a control request of `claude`:
`{"type":"control_response","response":{"subtype":"success","request_id":"<its id>","response":<body>}}`,
with the body `{"behavior":"deny","message":"…"}` for a deny.

**Argument order and details as S99 built them.** `-p ...` (the fixed arguments),
`--settings=<json>`, then `--add-dir=` per extra workspace, then one flag per launch
value in the order of the table: `--model=`, `--fallback-model=`, `--effort=`,
`--system-prompt=` or `--append-system-prompt=`, `--tools=`, `--allowedTools=`,
`--disallowedTools=`, `--permission-mode=`, `--permission-prompts=none`, `--agents=`
(one JSON object keyed by name), `--json-schema=`, `--max-turns=`, `--max-budget-usd=`,
the session flags, `--no-session-persistence`, `--include-partial-messages`, and last
`--plugin-dir=<run folder>/watane-run`. `--tools=` always goes out, empty when the launch
names no tool, because `claude` has every built-in tool without it; at `subagent-depth`
0 `Task` is dropped from it. The program is started with `process_group(0)`; the workspace
forbids `unsafe`, so a group is killed with `rustix::process::kill_process_group` (SIGKILL), only for a pgid above 1 (review r3, F3.6). A line over 4 MiB
ends the stream `failed`. `claude-code-shell.start` requires a `sandbox` object and
takes its content unchecked until Q39 and Q42 are decided.

**The session functions.** `claude-session.say`, `answer`, `interrupt` and
`compact` act on host stream `s` of a `claude-code` start; the caller must be
the stream's consumer, else `not-yours` (`unknown-stream` for an unknown or
ended id). Each is a request (19.6): applied after the call chain commits, so
a call that is rolled back, deferred or failed, writes nothing to stdin. The
checks of the table above are made at the call.

**Folders.**

- Run folder `<home>/runs/<random>/`: the plugin folder `watane-run`. Deleted
  when the stream ends; all of `runs/` is deleted at open, after the recorded
  process groups are killed (3.1 step 3).
- Scratch workspace `<home>/scratch/<random>/`, named by the host, owned by
  the origin client; a mod sees its id. It asks no consent: no participant
  folder is in it. Decided by S99's foreman (agentic-harness TECHNICAL 4.6
  left it open; no WIT change, so cheap to undo):
  - **Made by the launch.** `workspace: "scratch"` makes a new one when the
    start applies (19.6), before the process starts: 128 random bits as 32 hex
    chars, the folder created mode 0700. Its id is `scratch:<32 hex>`.
  - **Seen in `initialized`.** Every `claude-code` stream's `initialized` item
    names the launch's workspace: `{"type":"initialized","workspace":"<id>"}`,
    so the mod learns a new scratch id there.
  - **Held across a resume.** A later launch names it as `workspace:
    "scratch:<32 hex>"` (or in `extra-workspaces`). The owner is kept in host
    state (the `scratch` table of 7: the id to the origin client's identity),
    so it survives a restart. An id that is unknown, malformed or owned by
    another identity is `bad-launch {field, reason}` at the call; at apply the
    host checks again, and a folder or row gone since ends the stream `gone`.
  - **Lifetime.** A scratch workspace outlives its stream and its process. The
    `remove` of its owner (4.4) deletes its rows and its folders; nothing else
    does. A start checks that its stream is not stopped after the version probe and before
    each step that makes a folder, a row or a process. It checks once more after it made a
    new scratch folder and its row; if stopped, it deletes that one folder and that one row
    (never the owner's other rows) and the stream ends `gone`. A preview's home goes with
    the preview.
  - **Picked folders** come with the workspace consent (stage A3, S103). A
    `workspace` or `extra-workspaces` value that is not a scratch value is an
    absolute path. At the call the host resolves it (canonical, symlinks resolved,
    it must exist and be a folder) and checks it against the refusals of "Picked
    workspace" below and against Unicode NFC (a canonical name not in NFC is
    refused, fail-closed: S100's adversary); any failure is `bad-launch {field,
    reason}`. The workspace id is that canonical path, and `initialized` names it.
    At apply (19.6), after the version probe and before the run folder, the
    process or any scratch row, the host checks each picked folder of the launch
    against the origin client's consents `Workspace {via, folder}` (`via` the
    identity of the starting mod), resolving it again; a folder whose canonical
    path changed since the call ends the stream `gone`. For each one not held it
    pushes `{"type":"consent-pending","what":"workspace:<via>:<folder>"}` once
    and asks `Confirm::consent` (19.7), one folder at a time in launch order;
    `Granted` is stored and the next is checked; `Refused`, a dropped answer or
    600 s ends the stream `consent-refused("workspace:<via>:<folder>")` and no
    process starts. After the last answer the host resolves each folder once more
    (an answer may take minutes); a changed one ends the stream `gone`, and the
    consent just stored stays. So `consent-pending` items, when any, come before
    `initialized`, which stays the first item of a run.
  - **Sessions are the origin client's** (section 18, Q50, option (a), decided by
    S103's foreman). The host keeps the session ids each origin client's runs
    reported (the `sessions` table of 7: the `session_id` of each `system/init`
    line, to the origin client's identity, written when step 3 of the control
    protocol passes). `session {resume | fork}` with an id that the table does not
    hold for the origin client is `bad-launch {field: "session", reason}` at the
    call and ends the stream `gone` at apply; `session {new: uuid}` with an id
    the table holds for another identity is refused the same way. `remove` of an
    identity deletes its rows.
  - Whether one session can read another's scratch or run folder is section
    18, Q40, still open. Mods see only ids, so moving the folders (Q40 b)
    changes no interface.
- Picked workspace: canonical, symlinks resolved. Refused: `/`, the home
  folder, the watane home and its ancestors, `~/.claude`, `~/.codex`, `~/.ssh`,
  `~/.aws`, `~/.gnupg`, `~/.config/gh`, `~/.docker`, `~/.kube`,
  `~/Library/Keychains`, the run and scratch trees, and any folder inside one
  of these (the watane home, the credential folders just named, `runs/`,
  `scratch/`); each credential folder is compared in both spellings, as named
  under `$HOME` and canonical (a `~/.ssh` that is a link to `~/dotfiles/ssh`
  refuses both: S103's adversary); compared without case on a
  case-insensitive file system.

### 19.4 HTTPS streams and accounts (H3)

`https-stream.start(request)`; a request (19.6). The request is JSON: `url`,
`method`, `headers`, `body`, `account?`, `idle-s?` (default 90, at most 300).
(agentic-harness TECHNICAL 5.)

- **Items.** First `{status, headers}`; then one item per server-sent event,
  or per 64 KiB chunk of a body that is not SSE.
- **Refused at the call, before a byte leaves** (the `start-error`): a URL that
  is not `https`, or whose host is not exactly one of the mod's granted `Http`
  hosts (6.8): `destination`; the headers `Authorization`, `Cookie`, `Host`,
  `Proxy-*`, `OpenAI-Organization`, `OpenAI-Project`, `ChatGPT-Account-Id`,
  their names compared without case (`host`, `AUTHORIZATION`, `proxy-authorization`),
  and any name that is not an HTTP token (a leading `:` pseudo-header, white
  space, a control character): `forbidden-header` (S64); a body over 1 MiB: `too-large`; with `account:
  "openai"`, anything but `POST https://api.openai.com/v1/responses` or `GET
  https://api.openai.com/v1/models`: `destination`. At apply the host checks
  again that the mod is enabled and the hosts still granted, else the stream
  ends `gone`.
- No redirects. No retries: a mod decides retries.
- **Idle.** No bytes for `idle-s` ends the stream `idle-timeout`.
- **Account `openai`.** The app runs Sign in with ChatGPT and keeps the tokens
  (app 19.1). The host asks `AccountSource::openai_token` for a fresh access
  token per request and adds `Authorization: Bearer <token>`. Without an app:
  the stream ends `no-app`; without tokens: `no-account`. The consent of
  19.7 is held first.
- **Account `judge`** (N10, the human, 2026-11-20). The host keeps the judge's
  key as an account `judge`, as it keeps the OpenAI tokens. The participant
  enters the key and the judge's origin (`https://host[:port]`) in the app, on
  the manager page of that checked provider (app 19.4).
  - Identity: the provider is named by its `caller.identity()`
    (`<publisher>/<slug>`), compared exactly. The unchecked provider, another
    publisher's mod or a preview gets `no-account`.
  - Value: `account` is exactly `openai` or `judge`; any other value is
    refused `destination`.
  - Binding: the key is looked up by the caller's identity (`AccountSource::judge`),
    so a caller with no key of its own gets `no-account`. A request with
    `account: "judge"` is refused before a byte leaves (`destination`) unless
    the caller is the provider the key was entered for, the method is `POST`,
    and the URL's origin is exactly the account's origin and is in the
    provider's `[http]`. The origin is compared after one parse, used for both
    the check and the connection: scheme `https`, no userinfo, host
    lower-cased without a trailing dot, the port with 443 as default; the same
    parse decides `[http]`. The host adds `Authorization: Bearer <key>`; no
    redirects. Without an entered key: `no-account`.
  - When: `AccountSource::judge` is an in-process read, so the binding is
    checked at the call (`destination` is the `start-error`); with no key the
    call returns the stream, which ends `no-account`. At apply the host reads
    the account again and checks the binding again; a key that was removed
    or now names another origin ends the stream `gone`, before a byte leaves.
  - The parse: `judge_origin(text)` (below) parses an entered origin, and the
    host applies the same rule to the stored origin at each check. It accepts
    `https://host[:port]` and a lone trailing `/`; it refuses any other
    path, a query, a fragment, userinfo and any scheme but `https`
    (`bad_request`). Its text has the host lower-cased without a trailing dot
    and the port only when it is not 443. The app calls it for what it shows
    and stores (app 19.4); the host's check at each request decides where a
    key goes.
  - Lifetime: removing the key (the app calls
    `Host::account_changed(&Account::Judge { provider })`) ends the
    provider's live judge streams, outcome `gone`. Uninstalling the provider
    deletes its file, with the same call; a later mod with the same identity
    starts with no key. Signing out of ChatGPT (`Account::OpenAi`) ends every
    live stream with `account: "openai"` the same way.
  - Scope: the account is bound to the judge's origin (N10), not to one path.
  - Entering the key is the participant's consent for that provider: no
    per-origin-client consent for this account (19.7).

The host reaches tokens, keys and the `claude` path only through this
in-process trait, which the app implements and hands over through
`Confirm::accounts` (12.1), on the `Confirm` it gives to `Host::open`
(`Mode::Home { app }`); `Host::open` takes nothing new. No mod sees a
credential.

```rust
pub trait AccountSource: Send + Sync + 'static {
    /// A fresh access token for the account `openai`, refreshed when needed (app 19.1).
    fn openai_token(&self) -> Pin<Box<dyn Future<Output = Result<String, AccountError>> + Send>>;
    /// The key and origin entered for the provider with this identity (`<publisher>/<slug>`).
    fn judge(&self, provider: &str) -> Result<JudgeAccount, AccountError>;
    /// The path of the `claude` program the participant picked (app 19.3).
    fn claude_binary(&self) -> Result<PathBuf, AccountError>;
}
pub struct JudgeAccount { pub key: String, pub origin: String }   // Debug prints no key
/// NoAccount: no sign-in, no key, no program chosen. Unavailable: the entry is there but cannot
/// be used now (a token refresh failed on the network); never read as "not signed in".
pub enum AccountError { NoAccount, Unavailable(String) }          // Debug, Clone, PartialEq
/// An account whose entry changed (app 19.1 sign-out, 19.4 key removed or replaced, uninstall).
pub enum Account { OpenAi, Judge { provider: String } }           // Debug, Clone, PartialEq, Eq; provider: its identity
/// The judge-origin parse (above): the origin text, e.g. "https://judge.example" or "https://j.example:8443".
pub fn judge_origin(text: &str) -> Result<String, HostError>;
// Confirm (12.1) gains, default `None` (a home with no app, a preview, or an app with no accounts):
//   fn accounts(&self) -> Option<Arc<dyn AccountSource>> { None }
// Host gains:
//   pub fn account_changed(&self, account: &Account);   // ends the live streams that used it, outcome `gone`
```

Without an app (`accounts()` is `None`) each use fails `no-app`; an entry that
is absent is `AccountError::NoAccount`, which the host turns into `no-account`;
`Unavailable(m)` ends the stream `failed("account unavailable: <m>")`, so a mod
may retry. `claude_binary` failing `NoAccount` ends a start `no-binary`.
A preview has no accounts (19.7); its `claude` program is 19.9.

### 19.5 Agent tools (H4)

(agentic-harness TECHNICAL 6.) `agent-tools.open` and `call` are requests
(19.6); `describe` is a read and `close` acts at once (it only takes away).
The toolsets are those of 10.1; `open` with no toolset, or one that
`toolsets.json` does not hold, fails `not-granted` at the call.

- **Handle.** 128 random bits (32 hex chars), owned by (origin client, opening
  mod); it lives until `close`, the end of either mod, or a withdrawn consent
  (19.7). `open(toolsets)` returns the handle at once; each toolset new for the
  origin client waits for its consent (19.7); while one waits, `describe` and
  `call` fail `consent-pending(what)`, and after a refusal `consent-refused(what)`
  and the handle is closed. `describe` returns the tools of the toolsets as 10.1
  resolves them, with their annotations (below). `call` and `describe` on an
  unknown or foreign handle fail `unknown-handle`; a tool not in the handle's
  toolsets `unknown-tool`. A handle whose `open` has not been applied yet (the
  same chain) is `unknown-handle`.
  The first `describe` or `call` that finds a refusal returns `consent-refused(what)` and
  closes the handle; the next is `unknown-handle`. A handle is the origin client's: a
  `describe`, `call`, `close` or launch of another origin client, or by another mod than the
  opener (`describe`, `call`, `close`), finds `unknown-handle`. When each toolset's consent is
  held at `open` the handle is ready as soon as the call returns. The questions of the toolsets
  that wait go one after the other, on a thread of the host.
- **Token.** One MCP token per handle, 32 bytes, memory only (10.2), made when the handle
  opens, and written to `claude`'s fd 3 (19.3) only once its consents are held. It names the handle's toolsets together and is
  presented on `/mcp/agent` (10.3). A tool's mod sees the caller
  `agent:<handle-name>`: `h` and 30 lowercase hex chars (a `Handle`, 2.3), a
  second random value, never the handle itself.
- **Job.** `call(handle, key, tool, args)` returns a stream at once (its consumer is the
  opening mod, its origin client the handle's); the tool runs later as its own top-level
  call with its own budget and the caller of the previous bullet; its result is the stream's
  one item `{"ok": <json>} | {"err": {"code", "message"}}` (`args` that are not JSON: the
  `err` item with code `bad_request`, no call), and then the stream ends `closed`. `close`,
  a withdrawn consent, and the end of either mod end the job's stream `gone`; the
  running call is not interrupted, its result goes nowhere.
- **Limits** (decided by S103's foreman; host-internal, cheap to undo). At most 32 live
  handles per (origin client, opening mod); an `open` past it fails `not-granted` (no new WIT
  case). At most one consent question waits per (origin client, toolset): a later `open` that
  needs the same toolset waits on that answer instead of asking again, so a looping mod
  can neither spawn questions nor waiting threads without bound.
- **Key.** A live job with the same (handle, key) returns the same stream, also a job that
  the same chain, or another chain in flight, queued. The key is free when the result is
  there. Keys are memory only.
- **Annotations `@parallel` and `@idempotent`** come from the tool mod: a
  doc-comment line of the WIT function (`@parallel`, `@idempotent`); no
  `mod.toml` table (pkg question 21); default false each. They decide order and
  retries (agentic-harness TECHNICAL 9.5, 9.3), never permissions: a tool
  without them is as callable as one with them. `describe` carries them
  (`tool.parallel`, `tool.idempotent`); MCP `tools/list` (10.3) is unchanged,
  since `claude` orders its own calls.
- **Manager functions** are refused in a handle's toolsets by default
  (F-AGENT-4): a handle leaves out the functions of a mod that holds
  `Manage` (a manager, 6.10) unless they are in the functions stored with
  the toolset consent (19.7, `Consent::Toolset.functions`).
- A handle's tools are the union of its toolsets, read from the current
  catalog at each call (10.1), and only those whose names are in the
  functions stored with the origin client's consent for that toolset: the
  participant consented to the functions the words named, not to a name.
  A function the toolset gains later (an entry `<local>:<interface>` after an
  update adds a function, a `set_toolset` that adds an entry) is left out of `describe`, is `unknown-tool` at `call` and
  is not listed on `/mcp/agent`, until a later `open` of that toolset asks
  the consent again with the whole current list; `Granted` then replaces the
  stored list (S65). A name alone does not show that its mod changed: a mod
  that gains `Manage` keeps its function names, and a different mod installed
  under a removed mod's local name has the same names. So `remove` and a
  `grant` of `Manage` drop that mod's names from every stored list (4.4), and
  its functions are asked again like gained ones.
  A question that waits across such a change does not undo it (S103's adversary):
  the host notes a drop counter when it builds the question's list, and at
  `Granted` it leaves out of the stored list, under the catalog lock that writes it, the
  names of each mod whose names a `remove`, a `grant` of `Manage` or an update that grants
  `Manage` dropped since (4.4). The handle is ready with the names the participant saw; the names left out
  are asked again by a later `open`, like gained ones. A toolset left with no function
  stores an empty list and asks nothing.

### 19.6 Requests and timers (H5, H7)

(agentic-harness TECHNICAL 7.)

**Requests.** Each of these has an outside effect: `claude-code.start`,
`claude-code-shell.start`, `https-stream.start`, `agent-tools.open`,
`agent-tools.call`, `streams.open`, `timers.after`, `timers.cancel`, and
`claude-session.say`, `answer`, `interrupt`, `compact`. Each returns at once;
its checks that need nothing outside are made at the call; its outcomes
(consent refused, no binary, version too old, a refused value) arrive as the
stream's first item or as `ended`. `streams.resume`, `push`, `close`, `cancel`
and `hand-off` are not requests, but they follow the same chain rule below
(19.2, Commit).

- **When applied: with the whole chain.** A request (and a stream effect of
  19.2) goes to the outbox of its invoke. When the invoke succeeds, the outbox
  passes to the invoke that called it, not to the chain: a nested call's
  requests ride with its caller's. When an invoke fails or is rolled back
  (a trap, a limit, an `items` call that returns `defer`), it drops its own
  requests and every request its nested calls passed up to it. Only the
  requests that reach the top-level invoke, and only when that invoke
  succeeds, join the chain effects. The host applies them after the top-level
  invoke has returned and every write of the chain has committed, in order
  with the manage effects, before the events are delivered (6.6 step 6). So a
  request applies only when no call between it and the top of its chain was
  rolled back: a rolled-back call, deferred or failed, and every call nested in
  it, sends nothing to `claude`, opens no stream and leaves no timer, and its
  `timers.after` and `timers.cancel` are made again on redelivery. This rule
  differs from that of writes (Q3: a succeeded nested call keeps its writes
  when its caller fails), of events (9.1) and of manage effects (6.10), which
  a nested call keeps: section 18, Q37.
- **Durable first.** The host applies a request only after the commit of its
  chain is durable, whatever the durability of other calls (N4, the human,
  2026-11-20: every commit keeps the host's full durability, one fsync per
  write; a per-batch option is not built). redb's default commit (7) is
  that. V8 (15, A1) measures the cost and records it; whether to change this
  is the human's then.
- **At apply** the host checks again that the mod is enabled and holds the
  grant or consent the request needs (the rule of 6.10), and that the origin
  client is installed and enabled with the identity and the install
  generation that the request recorded at its call (19, Terms); a request that fails this ends its
  stream `gone`. Every check that a catalog change can falsify is made again
  where the effect lands, under the catalog lock that the change takes: an
  `Open` and a `HandOff` record the install generation of the mods they name,
  and of the origin client, at their call: a number in the running catalog
  only (not in `installed.json`; a restart ends every stream anyway), given
  to every installed local name when the catalog loads and replaced, by a
  number never used before, at every install, update and removal. A
  recorded generation that is missing fails the check.
  At apply a side updated, removed or reinstalled since (another
  generation), or not enabled, or a consumer that lost the `Call` grant, link
  or provider of `via` (Q46), ends the stream `gone` at once, both sides told;
  a `HandOff` whose target fails this ends its stream `gone`. The other
  effects of 19.2 Commit need no catalog check: `push` and `close` carry the
  producer, `cancel` and `resume` the consumer that the call acted for, and
  the hub acts only if it still holds that stream with that producer or
  consumer (a hand-off in between, even one that gives the stream back, makes
  a stale `cancel` or `resume` do nothing: the stream counts its hand-offs and
  the effect records the count at its call), and every change
  that falsifies their call (disable, update, removal, a lost grant, an
  `Open` or `HandOff` that failed above) ends the stream in the hub first, so
  they find nothing.
- **A call commits whole.** The host-state writes of one invoke commit
  together or not at all (7); a deferred or failed call leaves no write
  behind. So a failed `append` in a mod leaves no chunk written.

**Timers.** Per mod: ids of at most 128 bytes (else `bad-id`), at most 256
live (`too-many`), delays up to 30 days (else `bad-delay`), kept in host
state (7: `timers/<mod>/<id>` → due time and the origin client's identity,
never its local name, so a later mod given the same local name inherits no
timer's authority; and `remove` rewrites each timer that carries the removed
mod to the timer's own mod, so a later mod with the same identity, such as a
local mod with the same slug, inherits none either; 19, Terms; 4.4). `after(id,
delay-ms)` with a live id replaces it; `cancel(id)` removes it (an unknown id
is ignored). When `remove` finds a timer whose owner is not installed, so that its identity is not known, it sets that timer's origin client to the empty text, which no mod has (the timer then fires with its own mod as origin client). The checks (`bad-id`, `bad-delay`, `too-many` counted with the
timers that every chain in flight has queued, 19.2 Commit) are made at the call. The write follows the
chain rule above: the `after` and `cancel` that reach the top-level invoke are
written to the `timers` table in the write transaction of that invoke's
commit (7), together with its state writes; a rolled-back call's timers are
never written. A due timer fires as a
top-level call of the mod's `timer-sink.fired(id)` with caller `host`, fresh
budget, and the stored origin client; one due during downtime fires soon after
the host starts (within the first second). A firing reads its row again
after the mod's call lock is taken: a firing for an id cancelled or replaced
meanwhile (other due time) is ignored, and the origin client comes from that
row. Firings run on one worker per mod with due timers, in due order, and a
firing in progress is never armed again (`enable` reloads the rows; one firing
per row due), and a row that a reload skipped for that reason is armed when the
firing ends. A firing that finds its mod disabled or updated while it waited
keeps its row, and `enable`, an update or the next load arms it; it is no
failure. The origin client of the rows a chain writes is checked again at the
depth-0 commit, by identity and install generation, inside the write
transaction of the rows (after `begin_write`, reading the catalog for a
moment; the catalog lock is not held across the commit or its fsync).
`remove` changes the catalog before it opens the write transaction of its
rewrite, and redb runs one write transaction at a time, so either the commit
runs first and `remove`'s rewrite rewrites its row, or it runs after the
catalog change and sees the origin gone, and writes the timer's own mod. The
`timers` table keeps the identity only. A firing keeps its row when its own
mod is disabled, removed or updated; a nested call of the firing that fails
`disabled` or `no-mod` for another mod is a failure like any other (the row
is deleted). A mod in the `error` state gets no firing; its timers wait, and
the reload after every catalog commit arms them when the error
clears (a removal clears no other mod's error). Timers survive a restart; a kill after the
commit of `after` and before the host arms it loses nothing: the timer is
armed from state at start, and its firing happens once. A mod that is
disabled gets no firing; its timers wait. A mod that does not export
`timer-sink` gets no firing: at its due time the row is deleted and the host
logs `watane: timer <id> of <local> dropped: no timer-sink`. Removal of the mod deletes its
timers (the no-`timer-sink` delete checks, in its own write transaction,
that the install it read is still current and still lacks the export; an
unchanged reinstall of a mod in the error state loads its component like an
update). Streams and jobs are memory only (19.2): after a restart their
requests are gone, and the mod that wants them again starts them from a
fired timer or a call.

### 19.7 Consents

(agentic-harness TECHNICAL 8.) A consent is the participant's answer in the
app, kept by the host with the grants.

| Consent | Asked when | Kept for |
|---|---|---|
| Workspace | a launch names a folder new for the origin client and the mod whose request waits | (origin client, via, folder) |
| Account | an `https-stream` with `account: "openai"` new for the origin client and the mod whose request waits (`judge` asks none: entering its key is the consent) | (origin client, via, account) |
| Toolset | `agent-tools.open` with a toolset new for the origin client, or whose current functions are not all in the stored list (19.5) | (origin client, toolset), with the functions named |

- **Storage.** In `installed.json` on the record of the origin client (3.3,
  `consents`), shown and withdrawn like grants (4.4). A workspace and an account
  consent name `via`, the identity (pkg 4.1) of the mod whose request waits (the
  provider), so a consent given through one provider is not reused by another, nor
  by a mod in the middle that holds its own grant (section 18, Q38, option (b),
  decided by S103's foreman). A toolset consent names no `via`: its handle is the
  origin client's own (19.5). The words, the consent view and the choice belong to
  the app (app 19.2).
- **Asking.** When a request needs a consent not held, the host calls
  `Confirm::consent` (12.1), the one method for the three consents, when it
  applies the request. `Granted` is stored and the request goes on; `Refused`,
  or no answer in 600 s, ends the request `consent-refused(what)`. The host
  drops the future when the request's stream ends or its handle closes first
  (a withdrawal, a cancel, the mod's end); the app reads a dropped future as
  a withdrawn question. A preview and a home with no app refuse at once.
  `ConsentRequest` carries what the app's words need (agentic-harness
  TECHNICAL 8; app 19.2): `origin` is {origin client}, `via` is {provider}
  (the mod whose request waits), the consent's `folder` is {folder},
  `profile` is the launch's `profile` (19.3) or none, and `functions` are
  {functions}.
- **Origin client.** As in the terms above: carried by the host with the
  chain, never read from a request field.
- **While a consent waits,** a request's requester gets the item
  `{"type":"consent-pending","what":"<what>"}` once on the request's stream
  (`agent-tools` as 19.5 says); `what` is `workspace:<via>:<folder>`,
  `account:<via>:openai` or `toolset:<name>`, the same in `consent-refused(what)`
  (`<via>` the identity, which holds no `:`; the folder is the rest of the text).
- **Withdrawal** (`Host::withdraw_consent`, errors: `no_mod` for an unknown mod, `bad_request`
  for a consent the mod does not hold, matched by the `what` key) ends the dependent streams (outcome `gone`) and closes the
  dependent handles (19.5), at once, also inside a running chain (Q24).
- **Previews** refuse both grants and both accounts, and dev's fakes stand in
  (W5; agentic-harness FUNCTIONAL AGT-F-31): a preview never runs the
  participant's `claude`. It links `claude-code` and `claude-code-shell` as it
  grants any declared grant, but a start there runs only the program given by
  `Host::set_fake_claude` (19.9; dev's `fake-claude`), else ends `no-binary`;
  every `account` use ends `no-account`; every consent is refused (Asking), so
  a preview's `fake-claude` runs only in a scratch workspace (19.3).

```rust
// Debug, Clone, PartialEq, Eq, Serialize, Deserialize; JSON {"kind":"workspace","via":"…","folder":"…"} etc.
// Display and parse(&str) -> Result<Consent, HostError> use the `what` text: "workspace:<via>:<folder>",
// "account:<via>:openai", "toolset:<name>" (the key by which the app lists and withdraws a consent;
// `functions` is not part of the key). Toolset.functions: the `<local>_<interface>_<function>` names
// of the ConsentRequest that was Granted, sorted, less those that `remove` or a `grant` of `Manage`
// dropped since (4.4); a handle reaches no other (19.5).
pub enum Consent { Workspace { via: String, folder: String }, Account { via: String, account: String }, Toolset { toolset: String, functions: Vec<String> } }
pub struct ConsentRequest {            // Debug, Clone, PartialEq
    pub origin: LocalName,             // the origin client: {origin client}
    pub via: LocalName,                // the mod whose request waits (a provider, the kernel): {provider}
    pub consent: Consent,              // Workspace: {folder}, canonical
    pub profile: Option<String>,       // Workspace only: the launch's `profile` (19.3): {profile}
    pub functions: Vec<String>,        // Toolset only: the `<local>_<interface>_<function>` names it opens: {functions}
}
pub enum ConsentAnswer { Granted, Refused }
// Confirm (12.1) gains, default `Refused`:
//   fn consent(&self, request: ConsentRequest) -> Pin<Box<dyn Future<Output = ConsentAnswer> + Send>> { .. }
// Host gains:
//   pub fn withdraw_consent(&self, local: &LocalName, consent: &Consent) -> Result<(), HostError>;
//   (matches the stored consent by its `what` key, never by `==`: `Consent::parse("toolset:t")`
//   has no functions, and it withdraws the stored toolset consent `t` whatever its functions)
// InstalledMod gains: #[serde(default)] pub consents: Vec<Consent>;
```

### 19.8 Errors

The codes the host returns in this layer (agentic-harness TECHNICAL 12).
Every error carries a message that names the cause.

| Code | Where | Meaning |
|---|---|---|
| `not-yours`, `unknown-stream`, `full`, `too-large` | `streams`, starts | 19.2 |
| `consumer-failed`, `idle-timeout`, `replaced`, `lagging`, `gone` | stream outcomes | 19.2, 19.4 |
| `bad-launch {field, reason}` | `claude-code`, `claude-code-shell` | a value outside its grammar (19.3) |
| `no-binary`, `version {found, need}`, `mismatch {field}` | stream outcome | 19.3 |
| `slash-command`, `unknown-ask`, `not-allowed-here` | `claude-session` | 19.3 |
| `consent-refused {what}`, item `consent-pending {what}` | any request | 19.7 |
| `destination`, `forbidden-header`, `too-large`, `no-app`, `no-account` | `https-stream` | 19.4 (`no-app`, `no-account` as stream outcomes) |
| `not-granted`, `unknown-handle`, `unknown-tool` | `agent-tools` | 19.5 |
| `too-many`, `bad-id`, `bad-delay` | `timers` | 19.6 |
| `not-owner`, `not-direct` | the providers' answers, carried by `start-error` and `session-error` of `claude-types` | the host never returns them itself; `not-owner`: the caller is not the kernel; `not-direct`: the stream is not handed to the caller (agentic-harness TECHNICAL 2.2, 10) |

`bad-delay` is the host's own: agentic-harness TECHNICAL 12 names no code for
a delay past 30 days (7 sets the limit), so the host refuses it at the call
rather than clamp it.

### 19.9 Fakes and test hooks for tests and previews

dev's fakes (dev 18; dev 17, question 16) stand in for `claude`, OpenAI and a
judge. Two settings of the host let them in; neither exists in a participant
home's production path. A third, `test-streams`, serves only host's own A1
tests (last item).

- **The preview's `claude`.** `Host::set_fake_claude(program)` names the
  program that a preview runs for every `claude-code` and
  `claude-code-shell` start, under all the rules of 19.3 (arguments, stdin,
  `initialize`, `init`). Outside a preview it fails `bad_request` and
  changes nothing. With none set, a preview's start ends `no-binary`. The
  program `watane` takes it as `--fake-claude <path>` after `--preview`
  (12.2); `watane-dev serve` passes the `fake-claude` it linked into
  `<P>/claude/` (dev 18.4). Tests in a participant home give the path through
  their fake `AccountSource::claude_binary` instead.
- **Test routes** (feature `test-net`, off by default; host 1). With the
  feature, `Host::set_test_net(TestNet)` makes `https-stream` connect a
  request whose host equals a route's name to the route's local address,
  with TLS for that name, and trust `root_pem` besides the webpki roots.
  Every check of 19.4 runs on the URL as the mod wrote it, unchanged: the
  `Http` grants, the `openai` endpoints, the judge binding and its parse.
  `ScriptedResponses` gives the addresses (`responses_addr`, `judge_addr`)
  and the root (`ca_pem`). Another crate that tests with it enables the
  feature in its `[dev-dependencies]` only, and host's own A4 tests run with
  it (A4's brief names how); the release builds of `watane` and the app
  never hold it (checked in A4: `cargo tree -e features -i watane-host
  --workspace --edges normal` names no `test-net`; `-p watane-host` alone
  does not resolve the features that `app` or `dev` turn on).

```rust
impl Host {
    /// A preview only (else `bad_request`): the program run in place of `claude` (dev's `fake-claude`).
    pub fn set_fake_claude(&self, program: &Path) -> Result<(), HostError>;
    #[cfg(feature = "test-net")]
    pub fn set_test_net(&self, net: TestNet);
}
#[cfg(feature = "test-net")]
/// Debug, Clone. `routes`: ("api.openai.com", 127.0.0.1:port) and the judge host; `root_pem`: one PEM certificate.
pub struct TestNet { pub routes: Vec<(String, SocketAddr)>, pub root_pem: String }
```

- **Test streams** (feature `test-streams`, off by default; host 1; stage
  A1). The A1 tests (13.2) need a producer of host streams before H2 to H4
  exist, a consumer that takes nothing, a clock they control, and the
  writer's numbers for the budgets. The feature gives them; no mod, page or
  tool reaches it, and the release builds never hold it (checked as for
  `test-net`: `cargo tree -e features -i watane-host --workspace --edges normal` names no
  `test-streams`). Host's own tests enable it, and `test-net` from A4, by a
  dev-dependency of the crate on itself:
  `watane-host = { path = ".", features = ["test-streams"] }`.
  - `Host::test_stream(consumer)` opens a host stream as a host start does
    (19.2: the consumer's limit of 32, `full`; origin client the consumer
    mod), applied at once (no chain). `TestConsumer::Mod(local)` makes the
    mod its consumer (it must export `stream-sink`, else `not-yours`);
    `TestConsumer::Held { origin }` makes a host-side consumer that takes no
    batch until `TestStream::take` (a consumer that "never takes and does not
    defer"); each held stream is a consumer of its own (no limit of 32), and
    its queued bytes count against the origin client named by `origin` (an
    identity text, 19, Terms) and the host, so a test can fill an origin
    client's 128 MiB and the host's 512 MiB without a mod per origin.
  - `TestStream::push` queues one item under every rule of 19.2 (in flight,
    byte budgets, `too-large`); `paused()` is true while the host has stopped
    reading this source (in flight past its limit, or deferred), the "stops
    reading its source" of 19.2; `close(outcome)` closes it as a producer;
    `ended()` is the outcome the producer side got, once it ended.
  - `TestStream::deliveries()` lists every `items` call made for the stream,
    in order: the numbers of its batch and how it came back (`Taken`,
    `Deferred`, `Failed`). It is the oracle of "counted deliveries".
  - `Host::set_test_clock(TestClock)` replaces the clock of the lag clock and
    the deferral backstop of 19.2 (not the call deadline of 6.3, not the
    timers of 19.6, which tests wait for in real time); `TestClock::advance`
    moves it, and the host checks the stream clocks at each advance.
  - `Host::test_stats()` reads the counters of the state writer (7) since
    open: commits, the wall time spent in them (redb commit, fsync included), and
    the part of both that the host's own writes made (every commit of `state.rs` that is not a
    mod's transaction: timer writes, the scratch rows added, deleted or removed, the generation, a removed mod's state;
    the rest are the mods' transactions),
    and the state bytes read and written by calls (keys and values through
    `state`, host tables excluded). The load test of A1 takes the writer
    busy share and the state bytes written per item from two readings.

```rust
#[cfg(feature = "test-streams")]
impl Host {
    pub fn test_stream(&self, consumer: TestConsumer) -> Result<TestStream, StreamError>;
    pub fn set_test_clock(&self, clock: TestClock);
    pub fn test_stats(&self) -> TestStats;
}
#[cfg(feature = "test-streams")]
pub enum TestConsumer { Mod(LocalName), Held { origin: String } }
#[cfg(feature = "test-streams")]
/// Debug, Clone, PartialEq: the WIT `stream-error` (19.1).
pub enum StreamError { NotYours, UnknownStream, Full, TooLarge }
#[cfg(feature = "test-streams")]
/// Debug, Clone (shares the clock); starts at the real now.
pub struct TestClock { /* private */ }
#[cfg(feature = "test-streams")]
impl TestClock { pub fn new() -> TestClock; pub fn advance(&self, by: Duration); }
#[cfg(feature = "test-streams")]
pub struct TestStream { /* private: the stream id and a Weak of the host */ }
#[cfg(feature = "test-streams")]
impl TestStream {
    pub fn id(&self) -> &str;                               // 32 hex chars
    pub fn push(&self, body: &[u8]) -> Result<(), StreamError>;
    pub fn paused(&self) -> bool;
    pub fn close(&self, outcome: &str);                     // the outcome in the JSON of 6.9, e.g. "closed"
    pub fn ended(&self) -> Option<serde_json::Value>;       // the outcome in the JSON of 6.9
    pub fn take(&self) -> Vec<(u64, Vec<u8>)>;              // `Held` only: takes every queued item
    pub fn deliveries(&self) -> Vec<Delivery>;
}
#[cfg(feature = "test-streams")]
#[derive(Debug, Clone, PartialEq)]
pub struct Delivery { pub seqs: Vec<u64>, pub answer: DeliveryAnswer }
#[cfg(feature = "test-streams")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeliveryAnswer { Taken, Deferred, Failed }
#[cfg(feature = "test-streams")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TestStats { pub commits: u64, pub commit_time: Duration, pub host_commits: u64, pub host_commit_time: Duration, pub state_bytes_read: u64, pub state_bytes_written: u64 }
```
