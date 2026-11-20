# pkg: technical specification

How `pkg` does what `FUNCTIONAL.md` says. Ids `TC-`, `W` and section numbers
"spec §n" refer to `spec/TECHNICAL.md`. Sections 1 to 8 are needed for
steps 1 to 3 and are complete. Items marked "(step 4)" or "(step 5)" are
fixed only as far as the formats of steps 1 to 3 need them.

## 1. Crate and layout

One crate at the root of this folder: package `watane-pkg`, library
`watane_pkg`, edition 2024, stable Rust, no binary, no `build.rs`,
`unsafe_code = "forbid"`. It depends on no watane subsystem.

```text
pkg/
  Cargo.toml
  src/lib.rs        re-exports only; no logic
  src/names.rs      Slug, Version, compatible, InterfaceName, HttpHost,
                    PublisherKey, PackageId, ReleaseRef, NameError   (section 3)
  src/grant.rs      Grant, Provider, Requirement                     (section 5)
  src/meta.rs       mod.toml                                         (section 4.2)
  src/deps.rs       Deps, deps.json                                  (section 4.3)
  src/add.rs        add_provider                                     (section 4.8)
  src/world.rs      WIT reading, World, classification               (sections 4.4, 4.5)
  src/folder.rs     Mod, Mod::read, Violation, Rule, ModError        (sections 4.6, 4.7)
  src/component.rs  Mod::check_component                             (section 4.9)
  src/package.rs    pack, Package, PackageError                      (section 6)
  src/release.rs    Release, RELEASE_KIND, ReleaseError              (section 7)
  src/open.rs       open below a root without following child links  (section 6.4)
  src/refusal.rs    causes and next steps for a link or special file
  tests/            integration tests; tests/fixtures/<case>/ folders
```

All I/O is blocking filesystem I/O (`std::fs` and Unix `rustix`). No async,
no threads, no global state.

## 2. Dependencies

| Crate | Version | Features | Use |
|---|---|---|---|
| `wit-parser` | `0.254` | default (`decoding`, `serde`) | parse `wit/`, decode `logic.wasm` |
| `blake3` | `1.8` | default | package id |
| `data-encoding` | `2.11` | default | z-base-32, hex |
| `minicbor` | `2.3` | `default-features = false, features = ["alloc"]` | release body |
| `serde` | `1` | `derive` | JSON forms |
| `serde_json` | `1` | default | `deps.json`, JSON forms |
| `toml` | `1.2` | default | `mod.toml` |
| `thiserror` | `2` | default | error types |
| Unix: `rustix` | `1` | `fs` | safe descriptor-relative no-follow reads (6.4) |
| dev: `tempfile` | `3.27` | default | test folders |
| dev: `wit-component` | `0.254` | `dummy-module` | make test components (`dummy_module`, `ComponentEncoder`) |

Wasmtime 48.0.8 (`host`) requires `wit-parser` and `wasmparser` `0.254.1`,
which resolve to 0.254.2; `wit-parser`, `wasmparser` and `wit-component`
are 0.254.2, one copy each. The calls of 4.4, 4.9 and section 11 compile
against 0.254.2. No public item exposes a `wit-parser` type; when `host`
moves Wasmtime, raise these crates to what `cargo tree -i wit-parser` shows.

## 3. Text forms

`pkg` owns these forms. `host`, `dev`, `log` and `dist` use them as they are.
Each type has `FromStr` (strict), `Display` (the same text), `Serialize`
and `Deserialize` as that text, `Clone`, `Debug`, `Eq`, `Ord`, `Hash`.

| Type | Grammar | Limits | Example |
|---|---|---|---|
| `Slug` | `word *("-" word)`, `word = [a-z] *[a-z0-9]`; not a WIT keyword (3.1) | 1..=40 bytes | `reading-list` |
| `Version` | `num "." num "." num`, `num = "0" / [1-9] *[0-9]`; no pre-release, no build part | each part fits `u64` | `0.1.0` |
| `InterfaceName` | `ns ":" pkg "/" iface "@" Version`; `ns`, `pkg`, `iface` each `word *("-" word)` | ≤ 128 bytes | `mods:reading-list/books@0.1.0` |
| `HttpHost` | `label *("." label) [":" port]`; `label = [a-z0-9] [*61[a-z0-9-] [a-z0-9]]`; `port` 1..=65535, no leading zero | name ≤ 253 bytes | `api.openlibrary.org`, `127.0.0.1:8080` |
| `PublisherKey` | 32 bytes as unpadded z-base-32, alphabet `ybndrfg8ejkmcpqxot1uwisza345h769`, most significant bit first, the 4 unused bits zero | exactly 52 chars | `yryonyebyryonyebyryonyebyryonyebyryonyebyryonyebyryo` (32 × `0x01`) |
| `PackageId` | 32 bytes as lowercase hex | exactly 64 chars | `0866e9df…f62ff87b` |
| `ReleaseRef` | `PublisherKey "/" Slug "@" Version` | | `yryo…yryo/reading-list@0.1.0` |
| `Provider` (display only) | `PublisherKey "/" Slug`, or `local/` Slug | | `local/reading-list` |

Rules:
- Uppercase is refused, never folded.
- The `PublisherKey` text is the text of `log`'s `PublicKey` (log §4.1):
  same alphabet, bit order and length, and the same refusals in the same
  order (length, upper case, alphabet, unused bits). Only `log` checks that
  the bytes are a valid Ed25519 point. `PublisherKey::from_bytes([u8; 32])`
  and `as_bytes()` convert to and from `log`'s key.
- `Ord` of `Slug`, `InterfaceName` and `HttpHost` is byte order of the
  text. `Ord` of `Version` is numeric (major, minor, patch). `Ord` of
  `PublisherKey` and `PackageId` is byte order of the 32 bytes.
- A slug is also a DNS label and a WIT identifier, so `host` can build a
  local name and an origin from it.

### 3.1 WIT keywords refused as a slug

`use type func u8 u16 u32 u64 s8 s16 s32 s64 f32 f64 char resource own
borrow record flags variant enum bool string option result future stream
error-context list map as from static interface tuple world import export
package constructor include with async` (the keyword list of the
`wit-parser 0.254.2` lexer).

## 4. The mod folder

### 4.1 Layout

```text
mod.toml                     required (4.2)
deps.json                    optional; absent means no providers (4.3)
wit/world.wit                required; the package line and the one world (4.4)
wit/*.wit                    optional; more files of the same package
wit/deps/<slug>/*.wit        the WIT package of each mod provider, as a copy
wit/deps/watane-host/*.wit   the WIT of watane:host (the text comes from host)
wit/deps/<name>/*.wit        WASI packages, when the world uses wasi:http
logic/Cargo.toml             the logic crate (source; built by dev)
logic.wasm                   the built logic part, at the folder root
ui/index.html                the page; ui/ holds its other files
tests/                       contract tests (source; their form is dev's)
```

Every file read below the mod folder uses the private shared opener of 6.4.
Size and regular-file checks and the bounded read use the same opened handle;
there is no path metadata check followed by a second open for content.

The slug and the version are the package line of `wit/world.wit`:
`package mods:<slug>@<version>;`. The namespace is always `mods`.

### 4.2 `mod.toml`

UTF-8 TOML, at most 16 KiB. Unknown keys and tables are refused.

```toml
name = "Reading list"                  # required; 1..=64 chars
summary = "Keep a list of books."      # required; 1..=280 chars
repository = "https://github.com/alice/reading-list"   # optional (step 6)
main = true                            # optional (W36); a boolean; only with ui/

[http]                                 # optional
hosts = ["api.openlibrary.org"]        # 1..=16 HttpHost, no duplicates

[origin]                               # optional (step 5); written by dev
release = "<ReleaseRef>"               # the release this mod was made from
```

`name` and `summary`: no character of Unicode category Cc (this includes
newlines), no leading or trailing white space, length counted in `char`.
`repository` (step 6, W29, S40): the git repository with the source of
this mod. It is the publisher's claim: `host` and `dist` never fetch it;
dev clones it only to compare it with a verified package (dev 10.4.2).
A TOML string, checked by this grammar (no URL crate; the release body
does not change):

```text
repository = "https://" host [":" port] ["/" path]   ; 1..=256 bytes in all
host       = label *("." label)
label      = 1*63(ALPHA / DIGIT / "-")
port       = 1*5DIGIT
path       = *(ALPHA / DIGIT / "-" / "." / "_" / "~" / "/" / "%")
```

So no user name, password, query, fragment, white space or control
character. A refusal is one violation, rule `mod-toml`, file `mod.toml`;
the first failing check in this order gives the message exactly:

| Check | Message |
|---|---|
| over 256 bytes | `` `repository` is longer than 256 bytes `` |
| does not start with `https://` | `` `repository` must start with https:// `` |
| an `@` before the first `/` after `https://` | `` `repository` must not hold a user name or password `` |
| an empty host | `` `repository` has no host `` |
| any other break of the grammar | `` `repository` may hold only letters, digits and - . _ ~ / % after the host `` |

A value that is not a TOML string fails the second check (S41).

`Mod::repository()` returns the text as written.

`main` (W36, pkg stage 10): the mod proposes itself as the main mod of
the participant (the view the app opens at start). It is only a proposal:
the app decides. A TOML boolean; any other value is refused with
`` `main` must be true or false ``. A mod with `main = true` and no
`ui/index.html` is refused with `` `main` needs a page (ui/index.html) ``.
Both are rule `mod-toml`, file `mod.toml`. `Mod::main()` gives the value,
`false` when the key is absent. The release body does not carry it: the
app reads it from the installed mod.
The message for an unknown key `slug` or `version` is: "`<key>` is not a
mod.toml key: the slug and the version come from the package line of
wit/world.wit".

### 4.3 `deps.json`

A JSON object, at most 64 KiB and 32 members. Each key is
`mods:<slug>` of one provider; duplicate keys are refused. Each value is
the publisher key text, or `null` for a local provider.

```json
{
  "mods:reading-list": null,
  "mods:tags": "yryonyebyryonyebyryonyebyryonyebyryonyebyryonyebyryo"
}
```

`Deps::write` gives the canonical text: `serde_json::to_string_pretty` of
the map in key order, then `"\n"`. No members gives `{}\n`. It writes
`deps.json.tmp` in the folder and renames it over `deps.json`.

The version of a provider is not in `deps.json`: it is the version of the
package `mods:<slug>` in `wit/deps/<slug>/`. The release that a mod depends
on is `<publisher>/<slug>@<that version>` (a `ReleaseRef`).

### 4.4 Reading the WIT

1. Walk `wit/` (4.7 rule `path`), and check the sizes and the nesting
   (rule `limit`) before any parse.
2. Group the checked bytes by WIT package directory in `SourceMap`s and
   resolve with `Resolve::push_groups` inside `std::panic::catch_unwind`.
   Never pass filesystem paths to a parser that reopens them. Component
   import checking uses the imports from this same checked resolution.
   An error or a panic is rule `wit`, with the parser message rendered by wit-parser (its `--> <path>:<line>:<column>` location and snippet), cut to 2000 bytes at a UTF-8 boundary with that line kept. Before the cut the message escapes every control character except the `\n` between lines and every bidi format character (`\u{1b}`, `\r`, `\u{202e}`), and every invisible or line-separator character (U+2028, U+2029, U+200B-U+200D, U+2060, U+FEFF, the tag characters U+E0000-U+E007F) of the untrusted source.
3. The main package must be `mods:<slug>@<version>` with a valid `Slug` and
   `Version`, and hold exactly one world.
4. Classify each item of that world (4.5). `wit-parser` has elaborated the
   world, so interfaces pulled in by `use` are imports too.

Nesting scan: for each `.wit` file, skip `//` comments, `/* */` comments
and the token `->`; count `<`, `(`, `{` up and `>`, `)`, `}` down (never
below 0). A depth above 32 is rule `limit`.

### 4.5 Classification and grants

Every interface name comes from `Resolve::id_of`. A package without a
version is rule `wit`. A world item that is a function, or an interface
with a plain name (`import x: interface {…}` or `import x: ns:p/i`), is
rule `wit`. Type items are allowed and ignored.

| World item | Interface | Meaning | Grant |
|---|---|---|---|
| import | `watane:host/state@0.1.0` | private storage | `Storage` |
| import | `watane:host/caller@0.1.0` | the caller | none |
| import | `watane:host/manage@0.1.0` (step 10) | the management of mods (W35; host 6.10); `World::manage` | `Manage` |
| import | other `watane:*` | — | rule `import` |
| import | `wasi:{cli,clocks,filesystem,io,random,sockets}/*@0.2.x` | base WASI, linked to nothing | none |
| import | `wasi:http/outgoing-handler@0.2.x` | network | one `Http { host }` for each host of `[http] hosts` |
| import | `wasi:http/types@0.2.x` | types of the above | none |
| import | other `wasi:*` | — | rule `import` |
| import | `mods:<own slug>/*` | an outlet: its events and collects (`World::events`; below) | none |
| import | `mods:<other slug>/*` | a call (`World::calls`) | `Call { interface, provider }` |
| import | any other namespace | — | rule `import` |
| export | `mods:<own slug>/*` | an offered interface (`World::exports`) | none |
| export | `mods:<other slug>/*` | a subscription (`World::subscriptions`) | `Subscribe { interface, provider }` |
| export | any other | — | rule `export` |

**Outlet functions** (step 9, W34). Each function of an outlet has no
result (an event) or a result `list<T>` for any `T` (a collect). Any other
result is rule `import`, with the message `<interface>.<function>: a
function of an own interface that the world imports returns nothing (an
event) or a list (a collect)`. `World::collects` lists the outlets that
hold at least one collect; it is a subset of `World::events`. A
subscription to an outlet takes its collects with its events: the
subscriber exports the whole interface, so it answers both.

`provider` = `Provider { slug: <other slug>, publisher: deps.json value }`.
`Mod::grants()` returns the grants sorted by `Ord` (5.1), without
duplicates. An imported interface with only types still gives a `Call`
grant: the table of spec §3 is applied as written.

### 4.6 Parts

- The mod has a logic part if and only if `World::exports` or
  `World::subscriptions` is not empty. Then the folder holds `logic.wasm`
  or `logic/Cargo.toml`.
- With no logic part, the world imports nothing, and neither `logic.wasm`
  nor `logic/Cargo.toml` exists.
- The mod has a page if and only if `ui/` exists; then `ui/index.html` is a
  regular file.
- At least one part exists (F-RUN-1). All of these are rule `parts`.

### 4.7 Rules

`read_file` opens once through 6.4, takes file type and size from that
handle and reads it with a cap. A refused link is rule `path`, with
`a symbolic link; replace it with a regular file`; a non-regular file, a
socket included, is `expected a regular file` under the caller's rule. A missing optional
file remains `Ok(None)`. A part (`mod.toml`, `deps.json`, `logic.wasm`, `ui`, `wit`) that is a symbolic link gets exactly one violation, rule `path` with the part's name; no `parts` or other derived violation is added for it.

`Mod::read` collects every violation that it can find and returns them all
in `ModError::Invalid`. If `mod.toml`, `deps.json` or the WIT does not
parse, the cross checks that need it are skipped. Tests check `rule` and
`file`, and the exact provider messages specified below.

| Rule | File | Refused when |
|---|---|---|
| `path` | the path | `mod.toml`, `deps.json`, `logic.wasm`, `ui`, `wit` or anything below `ui/` or `wit/` is a symbolic link; in `wit/`: a directory other than `wit/deps` and `wit/deps/<segment>`, a `.wit` file deeper than `wit/deps/<segment>/`, a name that starts with `.` and is a directory or ends in `.wit`, a `.wasm` or `.wat` file in `wit/deps/`, a name outside the segment grammar (6.2). Other files in `wit/` are ignored. |
| `limit` | the file | a `.wit` file over 1 MiB, all `.wit` files over 4 MiB, nesting over 32; more than 32 exports, events, calls or subscriptions |
| `mod-toml` | `mod.toml` | missing, over 16 KiB, not UTF-8 TOML, an unknown key, a missing key, a bad value |
| `deps-json` | `deps.json` | over 64 KiB, not a JSON object, more than 32 members, a bad key or value |
| `wit` | the `.wit` file, or `wit/world.wit` | `wit/world.wit` missing; a parse or resolve error; a package line other than `mods:<slug>@<version>`; not exactly one world in the main package; a package without a version; one package name at two versions; a package whose namespace is not `mods`, `wasi` or `watane`; a `watane` package other than `watane:host@0.1.0`; a `mods:<slug>` dependency that is not in `wit/deps/<slug>/`; a world function or a plain-named interface |
| `import` | `wit/world.wit` | an import that 4.5 refuses |
| `export` | `wit/world.wit` | an export that 4.5 refuses |
| `provider` | `deps.json` | a call or subscription whose package has no member; a member that no call or subscription uses; a member for the mod's own package; the provider package has not exactly one world; a called interface that the provider's world does not export; a subscribed interface that the provider's world does not import |
| `http` | `mod.toml` | `wasi:http/outgoing-handler` imported with no `[http] hosts`, or hosts listed without that import |
| `parts` | `.` (the mod folder) | a rule of 4.6 |
| `component` | `logic.wasm` | a rule of 4.9 (only in `check_component`) |

For a `deps.json` member that no call or subscription uses, other than the
mod's own package, the `provider` message is exactly
`deps.json lists mods:<slug> but wit/world.wit imports none of its interfaces`.
For a member equal to the mod's own slug, it is exactly
`deps.json lists this mod itself as mods:<slug>`. The own-package case emits
one violation, even when the world exports its own interfaces.
A copy of the mod's own package under `wit/deps/<segment>/` (any version)
with no `deps.json` member for it is one `provider` violation at file
`wit/deps/<segment>`, exactly `wit/deps/<segment> holds this mod's own
package mods:<slug>`; it is reported before WIT resolution, so no
duplicate-package `wit` error hides it.

A symbolic link anywhere below `ui/` or `wit/`, hidden entries included, is rule `path` at the
link's own path. Below a WIT directory this section refuses, the reader still descends without
following links and reports each link instead of the directory rule; with none, the directory
rule applies. A non-hidden entry below `ui/` that is neither a regular file nor a directory is rule
`path`, `not a regular file or a directory (a FIFO, socket or device); replace or remove it`; hidden
special files are ignored, as pack ignores them (S46).

Pack and `Mod::read` word a link and a special file alike, from one set of constants (S57): a link
where a file or directory may stand is `a symbolic link; replace it with a regular file or
directory`; a link where only a file may stand (`mod.toml`, `deps.json`, `logic.wasm`, `read_file`)
is `a symbolic link; replace it with a regular file`. `PackageError::Symlink` displays
`` `<path>`: `` followed by the text for that path. An entry that was listed and is gone or no
longer a directory when the walk opens it, or an entry of it that is gone before its stat, is rule `path` at that entry, `changed while the mod was read; read it
again`. Other I/O errors stay `ModError::Io`.

### 4.8 `add_provider` (step 2)

`add_provider(dir, provider, publisher)` makes `provider` usable by the mod
in `dir`. It does not read the consumer's world, so it works on a mod that
does not yet pass `Mod::read`.

1. For each entry `E` of `<provider>/wit/deps/` (as the walk of 4.7 sees
   it): if `<dir>/wit/deps/E` exists and its files or bytes differ, refuse
   with rule `provider`, file `wit/deps/E`, and write nothing.
2. Replace `<dir>/wit/deps/<provider slug>/` with a copy of the top-level
   `.wit` files of `<provider>/wit/`.
3. Copy each entry of step 1 that does not exist yet.
4. Read `deps.json` (absent means empty), set `mods:<slug>` to `publisher`,
   and write it (4.3).

Preflight reads `deps.json` and checks the resulting member limit before any
write. Copy and comparison select regular `.wit` files; unrelated files
are ignored as in 4.7. Consumer destination ancestors must be directories
without symbolic links. Conflicts use `provider` at `wit/deps/E`; path
violations use `path`. I/O failures may leave completed copies in place.

Every read and write goes through the no-follow handles of 6.4. Preflight opens the consumer's
`wit/deps` once (creating `wit` and `wit/deps` when absent) and `wit/deps/<slug>` when present;
all later writes are relative to those handles, a link inside the old slug folder is unlinked and
never followed, and a link swapped in after the checks is never written through. `deps.json` is
written as in 4.3: the temporary file is created with `create_new` and renamed over `deps.json`,
so a link at either name is replaced, never followed.

A name created, removed or replaced between preflight and write (EEXIST or ENOENT from `mkdirat`,
`openat`, `unlinkat` or a listing) is `ModError::Invalid`, rule `path`, `changed while the mod was
read; read it again` at the path relative to the mod, never `ModError::Io` with raw OS text. A listed `.wit` that is a directory when it is read (or
below a directory that became a file) is rule `path`, `expected a regular file, found a directory`,
never rule `limit`.

### 4.9 `Mod::check_component`

Needed only when the mod has a logic part; otherwise it returns `Ok`.

1. `logic.wasm` exists, is a regular file, and is at most 64 MiB.
2. The bytes start with the WebAssembly component header
   (`00 61 73 6d 0d 00 01 00`), and
   `wit_parser::decoding::decode(bytes)` inside `catch_unwind` gives
   `DecodedWasm::Component(resolve, world)`. Anything else: "logic.wasm is
   not a WebAssembly component".
3. Each import of that world is an interface whose name is an import of
   the mod's world, or an interface of a base WASI package
   (`wasi:{cli,clocks,filesystem,io,random,sockets}` at `0.2.x`, any
   interface). Any other import, a function import or a plain name: "logic.wasm
   imports `<name>`, which wit/world.wit does not declare".
4. The set of export names equals the set of export names of the mod's
   world. Each difference is one violation.

## 5. Grants, providers and the version rule

### 5.1 Types

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Provider { pub slug: Slug, pub publisher: Option<PublisherKey> }   // None = local

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", try_from = "RawGrant")]
pub enum Grant {
    Storage,
    Http { host: HttpHost },
    Call { interface: InterfaceName, provider: Provider },
    Subscribe { interface: InterfaceName, provider: Provider },
    Manage,                                       // step 10 (W35): import watane:host/manage
}
```

- Invariant of `Call` and `Subscribe`: `interface.namespace() == "mods"`
  and `interface.package() == provider.slug`. `Deserialize` checks it
  (through a private `RawGrant`).
- Order: `Storage`, `Http`, `Call`, `Subscribe`, `Manage`; then host or
  interface text; then publisher, with local first. `Manage` is last, so
  every grant list that a release of an earlier step carries keeps its
  order and its bytes.
- JSON form, one per kind:
  `{"kind":"storage"}`,
  `{"kind":"http","host":"api.openlibrary.org"}`,
  `{"kind":"call","interface":"mods:reading-list/books@0.1.0","provider":{"slug":"reading-list","publisher":null}}`,
  `{"kind":"subscribe","interface":"mods:reading-list/events@0.1.0","provider":{"slug":"reading-list","publisher":"yryo…yryo"}}`,
  `{"kind":"manage"}` (step 10).
- Text form (`Display`, for logs and errors, not consent words):
  `storage`, `http <host>`, `call <interface> from <provider>`,
  `subscribe <interface> from <provider>`, `manage` (step 10).
- (step 4) `Grant::same_authority(&self, other) -> bool`: equal after
  removing the `@<version>` of the interface. `host` uses it for F-INST-7.

```rust
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Requirement { pub provider: Provider, pub version: Version }
impl Requirement {
    /// Same slug, same publisher (both local, or equal keys), and compatible(&self.version, version).
    pub fn satisfied_by(&self, provider: &Provider, version: &Version) -> bool;
}
```

`Mod::requires()` gives one `Requirement` for each provider slug in calls
and subscriptions, with the version of `mods:<slug>` in `wit/deps/`, sorted
by provider.

### 5.2 The version rule

`compatible(wanted, offered)`:
- if `wanted.major >= 1`: `offered.major == wanted.major` and `offered >= wanted`;
- if `wanted.major == 0`: `offered.major == 0`, `offered.minor == wanted.minor` and `offered.patch >= wanted.patch`.

`InterfaceName::satisfied_by(&self, offered)`: same namespace, package and
interface, and `compatible(self.version(), offered.version())`.

| wanted | offered | compatible |
|---|---|---|
| 1.2.0 | 1.2.0 | yes |
| 1.2.0 | 1.3.5 | yes |
| 1.2.0 | 1.1.9 | no |
| 1.2.0 | 2.0.0 | no |
| 0.1.0 | 0.1.7 | yes |
| 0.1.3 | 0.1.2 | no |
| 0.1.0 | 0.2.0 | no |
| 0.1.0 | 1.0.0 | no |
| 0.0.1 | 0.0.2 | yes |

## 6. The package

### 6.1 Content

| Path | Class | When |
|---|---|---|
| `mod.toml` | mod | always |
| `deps.json` | mod | when it exists |
| `wit/<s>.wit`, `wit/deps/<s>.wit`, `wit/deps/<s>/<s>.wit` | mod | always |
| `logic.wasm` | mod | when the mod has a logic part |
| `ui/<path>` | mod | when the mod has a page |
| `logic/Cargo.toml`, `logic/Cargo.lock`, `logic/src/<path>` | source | `include_source` and the file exists |
| `tests/<path>` | source | `include_source` |

A decoded package may hold only these paths. `has_source()` is true when
it holds a path of class source. Hidden regular files and directory contents
are omitted; hidden directories are still walked for links at every depth.

### 6.2 Paths

```text
path    = segment *15("/" segment)      ; 1..=255 bytes in all
segment = first *127rest                ; 1..=128 bytes
first   = ALPHA / DIGIT / "_" / "-"
rest    = ALPHA / DIGIT / "_" / "-" / "."
```

Also refused: two paths equal after ASCII lowercase (macOS file systems
ignore case); a path that is also the parent directory of another path.

### 6.3 Byte format

All integers are big-endian.

| Offset | Size | Field |
|---|---|---|
| 0 | 8 | magic `57 41 54 41 4E 45 50 4B` (`WATANEPK`) |
| 8 | 1 | format `01` |
| 9 | 4 | `count` (u32), 1..=4096 |
| 13 | … | `count` records |

Record: `path_len` (u16, 1..=255), `path` (`path_len` bytes, 6.2),
`size` (u32), `data` (`size` bytes). Records are in strictly ascending
byte order of `path`. Nothing follows the last record. The whole package
is at most `MAX_PACKAGE_BYTES` = 67 108 864 bytes (64 MiB). There is no
compression, no time, no owner and no mode, so equal files give equal
bytes, and the unpacked size is never larger than the package.

Count 0 is `PackageError::Format`; count above 4096 is
`PackageError::TooManyFiles` (`too many files; the limit is 4096`), in both
pack and decode. The package id is `blake3(all bytes)`.

### 6.4 Pack, decode and unpack

`pack(dir, include_source)`:
1. Walk the paths of 6.1 with `symlink_metadata`. Skip each entry whose
   name starts with `.` only after checking it and every descendant for
   symbolic links. Hidden regular content is omitted; links under hidden
   directories (including nested hidden directories under `ui/` and `wit/`)
   are `Symlink` with the full relative path, before mod validation.
   Refuse a symbolic link (`PackageError::Symlink`),
   anything that is not a regular file or a directory, and a name outside
   6.2 (`PackageError::Path`). Never walk outside these paths, so
   `logic/target/`, root `.git/` and root `.cargo/` are never read.
2. `Mod::read(dir)?` and `check_component()?`. The path walk comes first
   so selected links and special files are refused as package errors before
   any mod parser or file read can reach them.
3. Sort, check 6.2 and the limits, write the bytes of 6.3. Stop when the
   size passes the limit. The private opener shared with `folder.rs` returns
   an I/O result plus the relative prefix that failed. Open the caller's
   mod root normally (it may itself be a link, e.g. `/tmp` on macOS).
   On Unix, walk every part below it with safe `rustix::fs::openat`,
   `NOFOLLOW | RDONLY | CLOEXEC`, plus `DIRECTORY` for middle parts and
   `NONBLOCK` for the last part. Map `ELOOP` and `EMLINK` to `Symlink`;
   when `DIRECTORY` reports `ENOTDIR`, use no-follow `statat` relative to
   the open parent to distinguish a link. Name the prefix through the link,
   e.g. `wit/deps/x`. Non-directory middle parts and non-regular final
   handles give `Path`; FIFOs never block. Check size/type and read through
   the same final handle. Non-Unix uses a plain open and handle metadata.

4. Return `Package::from_bytes` of the result, so pack only gives what
   decode accepts.

`Package::from_bytes(bytes)`: check the length against the limit first,
then the header, then each record. Compare each declared length with the
bytes that remain before using it. Keep offsets into the input; allocate
nothing in proportion to a declared size. Refusals are `TooLarge`,
`TooManyFiles`, `Format` or `Path`, naming the actual cause.

`Package::unpack(dest)`:
1. The parent of `dest` exists. Create `dest` with `create_dir` (mode
   0755); if it exists, `PackageError::DestExists` and nothing changes.
2. For each file in order: create the missing parent directories with
   `create_dir` (0755), then the file with `create_new(true)` (0644), and
   write it.
3. `Mod::read(dest)?` and `check_component()?`.
4. On any error after step 1, attempt to remove `dest` and return the
   original unpack error even if cleanup also fails. Successful cleanup
   leaves no destination; a cleanup failure can leave partial output.

Every walk of the mod folder (`Mod::read` and pack) opens directories through the shared no-follow
walk and lists and stats entries relative to the open directory handle, never by a path string
below the root (S46). The write helpers of `open.rs` act relative to an open directory: `mkdirat`
(mode 0755), `openat` with `CREAT | EXCL | NOFOLLOW` (mode 0644) and `unlinkat` (with `REMOVEDIR`
for a directory); `add_provider` is the only writer (S57). An entry listed and then gone or
replaced when opened is the `changed while the mod was read` violation of 4.7. In `pack` each
gives a `PackageError::Path` at that path: an entry gone (open, listing or stat) has the reason
`changed while the mod was read; read it again`; a listed file that is a directory when opened,
`expected a regular file, found a directory`; a FIFO, socket or device, the special-file reason.
`open::file` tells these apart (`IsADirectory` against `InvalidInput`); `PackageError::Io` is left
for errors that name no such cause.

### 6.5 Example (golden)

The committed folder `tests/fixtures/hello`, packed without source:
- `mod.toml`: `name = "Hello"\nsummary = "Says hello."\n`
- `ui/index.html`: `<p>hello</p>\n`
- `wit/world.wit`: `package mods:hello@0.1.0;\n\nworld hello {}\n`

Bytes (159), hex (line breaks only for reading; this also holds for 7.3):

```text
574154414e45504b0100000003
00086d6f642e746f6d6c00000027
6e616d65203d202248656c6c6f220a73756d6d617279203d2022536179732068656c6c6f2e220a
000d75692f696e6465782e68746d6c0000000d3c703e68656c6c6f3c2f703e0a
000d7769742f776f726c642e7769740000002a
7061636b616765206d6f64733a68656c6c6f40302e312e303b0a0a776f726c642068656c6c6f207b7d0a
```

Package id: `0866e9df433265eaf0d959c589d82f1c2020d5b07faf7a530b012ec9f62ff87b`.
These values were made by an independent Python script and the `blake3`
crate, not by `pkg`.

## 7. The release body

### 7.1 Type

```rust
pub const RELEASE_KIND: &str = "release";    // the kind of the log entry that carries the body
pub const MAX_RELEASE_BYTES: usize = 32_768;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub slug: Slug,
    pub version: Version,
    pub name: String,
    pub summary: String,
    pub package: PackageId,
    pub size: u64,                       // package bytes
    pub source: bool,                    // package.has_source()
    pub offers: Vec<InterfaceName>,      // World::exports, sorted
    pub events: Vec<InterfaceName>,      // World::events, sorted
    pub grants: Vec<Grant>,              // Mod::grants(), sorted; every provider has a publisher
    pub origin: Option<ReleaseRef>,      // mod.toml [origin] (step 5)
}
```

The publisher is not in the body: it is the channel of the `log` entry.

### 7.2 Encoding

Deterministic CBOR (RFC 8949 §4.2.1: shortest heads, definite lengths),
written with `minicbor::Encoder` on a `Vec<u8>`:

```text
release = [ 1, slug: tstr, version: tstr, name: tstr, summary: tstr,
            package: bstr .size 32, size: uint, source: bool,
            offers: [* tstr], events: [* tstr], grants: [* grant],
            origin: null / [publisher: bstr .size 32, slug: tstr, version: tstr] ]
grant   = [0] / [1, host: tstr]
        / [2, interface: tstr, publisher: bstr .size 32 / null]
        / [3, interface: tstr, publisher: bstr .size 32 / null]
        / [4]                                    ; step 10: Manage
```

`Release::decode(bytes)` refuses: more than `MAX_RELEASE_BYTES`; anything
but an array of 12 with first item `1`; a value that breaks its type in
section 3 or 4.2; `offers`, `events` or `grants` not strictly ascending; an
offer or event whose package is not `mods:<slug>@<version>` of the
release; a grant whose interface is not `mods:` of another slug; a `null`
publisher in a grant (`ReleaseError::LocalProvider`); more than 32 offers,
32 events or 82 grants (1 storage, 16 hosts, 32 calls, 32
subscriptions and, from step 10, 1 manage); bytes that differ from `encode` of the result
(`ReleaseError::NotCanonical`, which also covers trailing bytes,
indefinite lengths and long heads).

`Release::new(m, package)`: build the fields from `m` and `package`; refuse
a local provider (`LocalProvider { slug }`). The largest body that `Mod`
allows is below 26 KiB, so `new` never exceeds the limit.

`Release::check(&self, m, package)`: `Release::new(m, package)?` must equal
`self`; else `Mismatch { field }` with the first differing field in the
order of 7.1.

A malformed release body is reported as `<field>: expected <what>, found <what was there>`; the
top-level array has no field prefix, since the error already reads `bad release body:` (S46). When the bytes end inside a value whose type was already read, `found` says so (`a text string cut short by the end of input`); when nothing was there it stays `end of input`.

### 7.3 Example (golden)

The release of the committed `tests/fixtures/hello` (6.5):
`[1, "hello", "0.1.0", "Hello", "Says hello.", h'0866e9df…f62ff87b', 159, false, [], [], [], null]`,
73 bytes:

```text
8c016568656c6c6f65302e312e306548656c6c6f6b536179732068656c6c6f2e5820
0866e9df433265eaf0d959c589d82f1c2020d5b07faf7a530b012ec9f62ff87b
189ff4808080f6
```

Grants `[storage, http api.openlibrary.org, call mods:reading-list/books@0.1.0
from K/reading-list, subscribe mods:reading-list/events@0.1.0 from
K/reading-list]` with K = 32 × `0x01`:

```text
8481008201736170692e6f70656e6c6962726172792e6f7267
8302781d6d6f64733a72656164696e672d6c6973742f626f6f6b7340302e312e30
58200101010101010101010101010101010101010101010101010101010101010101
8303781e6d6f64733a72656164696e672d6c6973742f6576656e747340302e312e30
58200101010101010101010101010101010101010101010101010101010101010101
```

Step 10: the grants `[storage, manage]` encode as `82 8100 8104`
(`828100 8104` without spaces: an array of 2, `[0]`, `[4]`).

Origin `K/reading-list@0.1.0`:
`835820` + 32 × `01` + `6c72656164696e672d6c69737465302e312e30`.
These were made by a hand-written CBOR encoder in Python.

## 8. Public API

All items are re-exported at the crate root.

```rust
// names.rs: section 3
pub struct Slug(String);            pub fn as_str(&self) -> &str;
pub struct Version { pub major: u64, pub minor: u64, pub patch: u64 }
pub fn compatible(wanted: &Version, offered: &Version) -> bool;
pub struct InterfaceName(String);
impl InterfaceName {
    pub fn namespace(&self) -> &str;  pub fn package(&self) -> &str;
    pub fn interface(&self) -> &str;  pub fn version(&self) -> Version;
    pub fn satisfied_by(&self, offered: &InterfaceName) -> bool;
}
pub struct HttpHost(String);        pub fn as_str(&self) -> &str;
pub struct PublisherKey([u8; 32]);  pub fn from_bytes(b: [u8; 32]) -> Self;  pub fn as_bytes(&self) -> &[u8; 32];
pub struct PackageId([u8; 32]);     pub fn of(bytes: &[u8]) -> Self;  pub fn from_bytes(b: [u8; 32]) -> Self;  pub fn as_bytes(&self) -> &[u8; 32];
pub struct ReleaseRef { pub publisher: PublisherKey, pub slug: Slug, pub version: Version }
#[derive(Debug, thiserror::Error)] #[error("invalid {kind} `{text}`: {reason}")]
pub struct NameError { pub kind: &'static str, pub text: String, pub reason: &'static str }  // text cut to 80 chars

// grant.rs: section 5
pub struct Provider { .. }  pub enum Grant { .. }  pub struct Requirement { .. }

// folder.rs, world.rs, component.rs, deps.rs: section 4
#[derive(Clone, Debug)]
pub struct Mod { /* private */ }
impl Mod {
    pub fn read(dir: &Path) -> Result<Mod, ModError>;
    pub fn check_component(&self) -> Result<(), ModError>;
    pub fn dir(&self) -> &Path;
    pub fn slug(&self) -> &Slug;
    pub fn version(&self) -> &Version;
    pub fn name(&self) -> &str;
    pub fn summary(&self) -> &str;
    pub fn http_hosts(&self) -> &[HttpHost];       // sorted
    pub fn origin(&self) -> Option<&ReleaseRef>;
    pub fn repository(&self) -> Option<&str>;        // step 6, 4.2
    pub fn main(&self) -> bool;                      // W36, 4.2
    pub fn deps(&self) -> &Deps;
    pub fn world(&self) -> &World;
    pub fn page(&self) -> Option<PathBuf>;        // <dir>/ui
    pub fn logic(&self) -> Option<PathBuf>;       // <dir>/logic.wasm (may be unbuilt)
    pub fn grants(&self) -> Vec<Grant>;
    pub fn requires(&self) -> Vec<Requirement>;
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct World {                                // each list sorted, no duplicates
    pub exports: Vec<InterfaceName>,
    pub events: Vec<InterfaceName>,
    pub collects: Vec<InterfaceName>,             // step 9: the outlets of `events` with a collect (4.5)
    pub calls: Vec<InterfaceName>,
    pub subscriptions: Vec<InterfaceName>,
    pub storage: bool,
    pub http: bool,
    pub manage: bool,                             // step 10: imports watane:host/manage (4.5)
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Deps(/* BTreeMap<Slug, Option<PublisherKey>> */);
impl Deps {
    pub fn read(dir: &Path) -> Result<Deps, ModError>;
    pub fn write(&self, dir: &Path) -> Result<(), ModError>;
    pub fn get(&self, slug: &Slug) -> Option<Option<&PublisherKey>>;
    pub fn set(&mut self, slug: Slug, publisher: Option<PublisherKey>);
    pub fn remove(&mut self, slug: &Slug) -> bool;
    pub fn iter(&self) -> impl Iterator<Item = (&Slug, Option<&PublisherKey>)>;
}
pub fn add_provider(dir: &Path, provider: &Mod, publisher: Option<PublisherKey>) -> Result<(), ModError>;

#[derive(Debug, thiserror::Error)]
pub enum ModError {
    #[error("{}: {source}", path.display())]
    Io { path: PathBuf, #[source] source: std::io::Error },
    #[error("the mod breaks {} rule(s): {}", .0.len(), list(.0))]   // "[rule] file: message; …"
    Invalid(Vec<Violation>),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Violation { pub rule: Rule, pub file: String, pub message: String }   // file relative to the mod folder
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)] #[serde(rename_all = "kebab-case")]
pub enum Rule { Path, Limit, ModToml, DepsJson, Wit, Import, Export, Provider, Http, Parts, Component }

// package.rs: section 6
pub const MAX_PACKAGE_BYTES: u64 = 67_108_864;
pub fn pack(dir: &Path, include_source: bool) -> Result<Package, PackageError>;
#[derive(Clone, Debug)]
pub struct Package { /* bytes, offsets, cached id */ }
impl Package {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Package, PackageError>;
    pub fn id(&self) -> PackageId;
    pub fn as_bytes(&self) -> &[u8];
    pub fn into_bytes(self) -> Vec<u8>;
    pub fn len(&self) -> u64;
    pub fn paths(&self) -> impl Iterator<Item = &str>;
    pub fn file(&self, path: &str) -> Option<&[u8]>;
    pub fn has_source(&self) -> bool;
    pub fn unpack(&self, dest: &Path) -> Result<Mod, PackageError>;
}
#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error("the package is {size} bytes; the limit is 67108864")] TooLarge { size: u64 },
    #[error("too many files; the limit is 4096")] TooManyFiles,
    #[error("bad package format: {0}")] Format(String),
    #[error("path `{}` is not allowed: {reason}", path.escape_debug())] Path { path: String, reason: &'static str },
    #[error("`{}`: {}", path.escape_debug(), refusal::link(path) /* 4.7 */)] Symlink { path: String },
    #[error("{} exists already", path.display())] DestExists { path: PathBuf },
    #[error("{}: {source}", path.display())] Io { path: PathBuf, #[source] source: std::io::Error },
    #[error(transparent)] Mod(#[from] ModError),
}

// release.rs: section 7
impl Release {
    pub fn new(m: &Mod, package: &Package) -> Result<Release, ReleaseError>;
    pub fn encode(&self) -> Vec<u8>;
    pub fn decode(bytes: &[u8]) -> Result<Release, ReleaseError>;
    pub fn check(&self, m: &Mod, package: &Package) -> Result<(), ReleaseError>;
}
#[derive(Debug, thiserror::Error)]
pub enum ReleaseError {
    #[error("the mod depends on the local provider {slug}; publish it first")] LocalProvider { slug: Slug },
    #[error("bad release body: {0}")] Malformed(String),
    #[error("the release body is not in canonical form")] NotCanonical,
    #[error("the release body is {size} bytes; the limit is 32768")] TooLarge { size: usize },
    #[error("the release and the package differ in `{field}`")] Mismatch { field: &'static str },
}
```

`Malformed` adds the context `bad release body:` once. Its cause does
not repeat that context: empty input displays `bad release body: end of
input bytes`; truncation keeps its field and end-of-input cause. Name
errors similarly avoid `slug: invalid slug` and `version: invalid version`,
while preserving any distinct enclosing field context.

## 9. Example folders (fixtures for J2)

`reading-list` (`deps.json` absent):

```toml
name = "Reading list"
summary = "Keep a list of books to read."
```

```wit
package mods:reading-list@0.1.0;

interface books {
  add: func(title: string) -> result<_, string>;
  all: func() -> list<string>;
}

interface events {
  added: func(title: string);
}

world reading-list {
  import watane:host/state@0.1.0;
  import watane:host/caller@0.1.0;
  import events;
  export books;
}
```

Plus `wit/deps/watane-host/host.wit` (the WIT of spec §4), `ui/index.html`,
and `logic.wasm` (for tests: made with `wit-component` from this world).

`recommendations`: `name = "Recommendations"`, `summary = "Suggest the next
book."`, `deps.json` = `{\n  "mods:reading-list": null\n}\n`, the copies
`wit/deps/reading-list/world.wit` and `wit/deps/watane-host/host.wit`,
`logic.wasm` (made the same way), and:

```wit
package mods:recommendations@0.1.0;

interface suggest {
  next: func() -> option<string>;
}

world recommendations {
  import watane:host/state@0.1.0;
  import mods:reading-list/books@0.1.0;
  export mods:reading-list/events@0.1.0;
  export suggest;
}
```

## 10. Security properties

| Property | Enforced by |
|---|---|
| Grants come only from the world (W2) | `Mod::grants` reads only the world, `deps.json` and `[http] hosts`; no other input exists. |
| A logic part cannot use an undeclared import | `check_component` (4.9) runs in `pack` and in `unpack`, so every installed and every published logic part passed it. |
| A provider is pinned by publisher (W14) | `Call` and `Subscribe` carry the publisher from `deps.json`; `Requirement::satisfied_by` compares it; `Release` refuses a local provider. |
| No path traversal | path grammar (6.2) in pack and decode, before any write; `unpack` writes only below a directory it has just created, with `create_new`. |
| No overwrite through case or file/directory clashes | the lowercase and parent checks of 6.2. |
| No size bomb | no compression; the length limit is checked first; each declared size is compared with the remaining input; `.wit`, `mod.toml`, `deps.json` and release limits are checked before parsing; WIT nesting ≤ 32. |
| A malformed input cannot stop the host | the WIT parser and the component decoder run inside `catch_unwind`; every refusal is a value, never a panic. The `wit` message escapes control, bidi, invisible and line-separator characters of the source (4: U+2028, U+2029, U+200B-U+200D, U+2060, U+FEFF, U+E0000-U+E007F). Malformed release errors name context and cause once. `Path` and `Symlink` display paths with `str::escape_debug`, preserving raw path fields without emitting control characters; an unpack error survives cleanup failure. |
| No secret leaves through a package | only the paths of 6.1; hidden directory descendants checked for links before omission; symbolic links refused; every mod and pack content read and every `add_provider` read and write uses the shared all-components no-follow walk and same-handle checks of 6.4 on Unix; WIT resolves checked bytes without reopening; private state lives in the home, never in a mod folder. |
| One meaning, one byte string | canonical package bytes (6.3) and canonical release CBOR (7.2) with decode-then-re-encode; so one id and one signature per meaning. |
| A package matches its release | `Release::check` re-derives the body from the unpacked mod and compares every field. |
| `pkg` runs nothing | it only parses; it starts no process and loads no Wasm into a runtime. |

Residual: a fault in `wit-parser` or `wasmparser` (stack overflow is not
caught by `catch_unwind`; the nesting limit reduces it).

## 11. Test plan

Each test computes its expectation from this file, from hand-written
bytes, or from another tool. None calls the function under test to get its
expected value.

| Test | Checks | Oracle |
|---|---|---|
| `names` | each grammar of section 3: accepted and refused examples; `Display` gives the input back | hand-written tables from section 3 |
| `zbase32` | key text of 32 × `0x00` is 52 × `y`; of 32 × `0x01` is A25; the RFC 8032 key of log §4.1 | Python of this spec; log §4.1 |
| `compatible` | the table of 5.2 | the table |
| `mod_toml`, `deps_json` | schema, limits, unknown keys (A8), canonical write | literal texts |
| `grant_manage` (step 10) | A30: the world of A30 gives `World::manage` true and grants `[storage, manage]`; `{"kind":"manage"}` and the text `manage` both ways; `Manage` sorts after a `Subscribe`; `same_authority(Manage, Manage)`; A31: the release of a mod with `[storage, manage]` encodes its grants as `828100 8104` and decodes back; a release whose grants hold `[4]` before a `[3, ...]` is refused as not ascending | the table of 4.5; literal JSON, text and hex written by hand |
| `mod_toml_main` (W36) | A28, A29: `main = true` with a page gives `Mod::main()` true; absent gives false; `main = "yes"` and `main = true` without `ui/index.html` give the exact messages of 4.2 | literal texts |
| `mod_toml_repository` (step 6) | accepted: `https://github.com/alice/reading-list`, `https://git.example.org:8443/a/b-c.d_e~f%20`, `https://h`, a 256-byte URL; refused, with the exact message of 4.2: `http://github.com/a/b`, `https://user@github.com/a/b`, `https://user:pw@h/x`, `https://`, `https:///a`, a 257-byte URL, `https://h/a b`, `https://h/a?b=c`, `https://h/#x`, a non-string value; `Mod::repository()` gives the text back; a package of a mod with `repository` unpacks and reads it back | literal texts |
| `grants` | A1, A2, A3; ordering of 5.1; JSON forms of 5.1 | the table of 4.5 and the literals of 5.1 |
| `rules` | one fixture for each line of 4.7; `rule` and `file` as expected (A4–A7) | the table of 4.7 |
| `component` | A9, A10, a missing export, an extra export, a core module, `wasi:http/types` without the world import | components made by `wit-component` (`dummy_module`, `embed_component_metadata`, `ComponentEncoder`) from a world written in the test, not from `pkg` |
| `pack_golden` | A11 | hex and id of 6.5 |
| `pack_determinism` | A12 | the other copy |
| `pack_content` | A13, A14, a name with a space, a FIFO | literal path lists |
| `decode_refusals` | A15, A16, bad magic, format `02`, count 0, unsorted, duplicate, trailing byte, short record | bytes built in the test by a raw writer that neither sorts nor checks |
| `unpack` | the files on disk equal the package map; modes 0644 and 0755; A17; A18 | a directory walk compared with a literal map |
| `round_trip` | 200 file sets from a seeded generator: `from_bytes(pack(dir))` gives back each file | the generated map |
| `add_provider` | A20; a conflicting `wit/deps` entry writes nothing | literal files |
| `release_golden` | A21; `decode` of the hex of 7.3 equals the literal struct; the grant and origin hex of 7.3 | hex of 7.3 |
| `release_refusals` | A22, A23, a `null` publisher, unsorted offers, 13 items, format `2` | hand-edited hex |
| `release_check` | A24 and one case for each field | literal structs |

S22 tests: `ac1_s22_hidden_links` and `ac1_s22_hidden_regular_files_omitted`
check hidden descendant links and byte/id equality; private opener unit test
`ac2_s22_every_part_no_follow` checks linked middle parts, FIFOs and linked
roots; `read_file` unit tests `ac3_s22_checked_handle` and
`ac3_s22_missing_optional_file` preserve diagnostics and optional absence.
`ac4_s22_committed_fixtures_pack` packs both on-disk logic fixtures and
checks the local-provider refusal. Their committed `logic.wasm` files are
produced independently by `dummy_module` → metadata → `ComponentEncoder`;
regenerate with `cargo test -p watane-pkg --test it support::regenerate_fixtures -- --ignored`
(the test is `#[ignore = "regenerates fixtures"]`). Ordinary release tests
copy these committed files without rebuilding. `ac5_s22_disk_hello_golden`
checks the disk fixture against both 6.5's id and 7.3's 73 literal bytes.
`ac6_s22_release_cause_once` and `ac6_s22_other_repeated_contexts` check
non-repeating contexts and the retained cause. All existing golden bytes
and ids remain fixed.

`cargo test` runs all of them; no network, no Wasm runtime, no toolchain
for `wasm32-wasip2`.

## 12. Build plan

`pkg` needs no sibling at any stage, so nothing is faked. Each stage ends
with `cargo test`, `cargo clippy --all-targets -- -D warnings` and `cargo
fmt --check` green.

| Stage | Build | Done when | Needed by |
|---|---|---|---|
| 1 | `names.rs` (section 3, 5.2) | `names`, `zbase32`, `compatible` pass | all |
| 2 | `meta.rs`, `deps.rs` (read, write), `Violation`, `Rule`, `ModError` | `mod_toml`, `deps_json` pass | dev `check` |
| 3 | `world.rs`, `folder.rs`, `grant.rs` (4.4–4.7, 5.1) | `grants`, `rules` pass | host plan, dev `check` (J1) |
| 4 | `component.rs` (4.9) | `component` passes | dev `test`, host install (J1) |
| 5 | `package.rs` (section 6) | `pack_*`, `decode_refusals`, `unpack`, `round_trip` pass | host preview and install (J1, J2) |
| 6 | `add_provider` (4.8) | `add_provider` passes | dev `add` (J2) |
| 7 | `release.rs` (section 7) | `release_*` pass | dist publish and fetch, host verify (J3) |
| 8 (step 6) | `repository` in `meta.rs` (4.2), `Mod::repository` | `mod_toml_repository` passes | dev `publish --push`, `new --from` (step 6) |
| 9 (step 9) | the outlet-function rule and `World::collects` in `world.rs` (4.5) | `outlet_functions` passes (A26, A27) | host collect and cycle check, dev contract fakes (step 9) |
| 10 (W36) | `main` in `meta.rs` (4.2), `Mod::main` | `mod_toml_main` passes (A28, A29) | app stage I |
| 11 (step 10) | `Grant::Manage` (5.1), `World::manage` and the 4.5 row in `world.rs`, the grant `[4]` and the limit 82 in `release.rs` (7.2) | `grant_manage` passes (A30, A31); every earlier golden byte unchanged | host stage 11, dev stage 13, app stage J |

`Grant::same_authority` (step 4) is one function; build it with stage 3 or
when step 4 starts.

## 13. Provides to other subsystems

| Item | Users |
|---|---|
| `Slug`, `Version`, `InterfaceName`, `HttpHost`, `PublisherKey`, `PackageId`, `ReleaseRef`, `NameError`, `compatible` | all |
| `Mod` (read, check_component, accessors), `World`, `Violation`, `Rule`, `ModError` | host, dev |
| `Grant`, `Provider`, `Requirement` (one shape and one JSON form everywhere) | host, app (through host), dev |
| `Deps`, `add_provider` | dev |
| `pack`, `Package`, `PackageError`, `MAX_PACKAGE_BYTES` | host, dev, dist |
| `Release`, `RELEASE_KIND`, `MAX_RELEASE_BYTES`, `ReleaseError` | dist, host |

Public types are `Send + Sync`, errors also `'static`. Callers use section
8 as it is (D-A3); no request of host §17, dist §10 or dev §14 adds an item:
- `Error` and its `provider()`, `field()`: `ModError`, `PackageError`,
  `ReleaseError`; match `LocalProvider { slug }`, `Mismatch { field }`.
- Text fields and accessors, `Hash`, `is_slug`: the typed items of section
  3 (`PackageId` for `Hash`); `Display`, `FromStr` and serde give the text.
- `has_logic`, `has_page`, `World::package`, `Provider { version, release
  }`: `logic().is_some()`, `page().is_some()`, `slug()` and `version()`,
  `requires()`. `Grant` is that of 5.1.
- `Release::of(&Package)`, `check(&Package)`: `package.unpack(tmp)?`, then
  `Release::new(&m, &package)` or `release.check(&m, &package)`.

## 14. Needs from other subsystems

Nothing at build time. These are the facts that the formats above rely on:

| From | Need |
|---|---|
| `log` | `PublicKey` is 32 bytes (Ed25519), with `as_bytes(&self) -> &[u8; 32]` and a fallible constructor from `&[u8; 32]`; its text form is the `PublisherKey` text of section 3. An entry `kind` is a string, and a body of `MAX_RELEASE_BYTES` (32 KiB) fits. |
| `host` | The canonical text of `watane:host@0.1.0` (spec §4) as a `pub const`, which `dev` copies to `wit/deps/watane-host/host.wit`. To install a package of another participant: `Entry` kind = `RELEASE_KIND`, `Release::decode(body)`, `release.package == package.id()`, `unpack`, `release.check(&m, &package)`. |
| `dev` | The builder writes the component to `<mod>/logic.wasm`. The template follows 4.1–4.2 (no `slug` or `version` in `mod.toml`; package line `mods:<slug>@0.1.0`). `add <mod folder>` calls `add_provider(dir, &provider, None)`. |
| `dist` | The body of a `release` entry is `Release::encode`. Publish refuses a second release of one slug and version in the own channel; when reading, the first release of a slug and version in a channel is the one that a `ReleaseRef` names. Fetch refuses more than `release.size` bytes, then compares `PackageId::of`. |

## 15. Questions for spec/

Each answer below is the one that this specification uses.

1. **Slug and version in `mod.toml`**: decided (D-A2), sections 4.1–4.2.
2. **`deps.json` fields**: decided (D-A2), section 4.3.
3. **WIT namespace**: decided (D-A2), always `mods`.
4. **A mod with only a page**: does it need a world? Answer: yes, an empty
   world, so every mod has one contract file and one place for its identity.
5. **Where the built logic part lives**: spec §3 says `logic/` "builds to
   logic.wasm". Answer: `<mod>/logic.wasm`, at the folder root.
6. **"Base WASI interfaces"** (spec §4) are not listed. Answer:
   `wasi:{cli,clocks,filesystem,io,random,sockets}` at `0.2.x`; plus
   `wasi:http/types` beside `wasi:http/outgoing-handler`.
7. **World functions, plain-named interfaces, unversioned packages** are not
   covered by the table of spec §3. Answer: refused.
8. **How a release is named** across channels: decided (D-A1), `ReleaseRef`;
   a `log` entry id names an entry only inside its channel.
9. **Release body fields**: the sketches list slug, version, package,
   summary, interfaces, origin. Answer: also `name`, `size` (fetch limit),
   `source` (F-PUB-4, F-BUILD-1), `events` and `grants` (F-FIND-3, F-INST-7).
10. **What "the source" is** (spec §3, F-PUB-4). Answer: `logic/Cargo.toml`,
    `logic/Cargo.lock`, `logic/src/`, `tests/`.
11. **Who owns the WIT text of `watane:host`**. Answer: `host`; `pkg` knows
    only the interface names `state` and `caller`.
12. **Version rule details**: section 5.2; no pre-release versions.
13. **An imported interface with only types** is a call grant by spec §3.
    Answer: kept; `host` links it with no functions.
14. **Folder-level violation paths** (pkg, host, dev): `parts` uses `file = "."`, the relative path of the mod folder, so diagnostics need no absolute path or empty-string convention.
15. **Duplicate provider keys** (pkg, host, dev): `deps.json` refuses duplicate object keys instead of silently choosing a publisher; section 4.3 makes this explicit.
16. **Component versus core module** (pkg, host, dev): `wit-parser` 0.254.2 decodes an empty core module as `DecodedWasm::Component`; section 4.9 checks the component header explicitly before decoding.
17. **Package preflight** (pkg, host, dev, dist): section 6.4 walks selected paths before `Mod::read`, preserving `PackageError::Symlink` and refusing special files before opening them; the byte format and mod checks are unchanged.

18. **Provider copy preflight** (pkg, dev): section 4.8 compares selected `.wit` files, checks dependency metadata before writes, and refuses linked destination ancestors; I/O failures do not promise rollback.

19. **Folder reads and fixture provenance** (pkg; host, dev, dist see no API change):
    6.4's private safe `rustix` walk checks each child component, permits a
    linked caller root, and shares checked handles with folder reads. WIT
    parses the bounded bytes instead of reopening paths. Hidden descendant
    links are refused before omission. J2 logic fixtures commit independently
    generated components; the on-disk hello is exactly 6.5 and 7.3. Malformed
    release contexts appear once. Public signatures and byte formats stay fixed.

20. **The grant `Manage` in a release body** (S73, step 10). W35 makes
    `Manage` a grant kind, and a release body carries its grants (7.1), so
    it is encoded `[4]`. A home built before step 10 refuses such a
    release as a bad release body, and names it: it cannot install that
    mod, which it could not run anyway. Every release without `Manage`
    keeps its bytes, and the body format stays `1`. The other way, a body
    that leaves `Manage` out, would make F-INST-7's "no new grant" check
    blind to it. Decided; the cost to undo is a new body format.
