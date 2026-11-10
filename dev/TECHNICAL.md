# dev: technical specification

How `watane-dev` does what `FUNCTIONAL.md` says. Sections 13 and 14 are
the contract with the other subsystems, section 15 the test plan, section
16 the build plan, section 17 the questions for `spec/`. The sibling names
used here follow the draft `TECHNICAL.md` files of `pkg`, `host` and `app`
of 2026-11-10; section 14 lists every item.

## 1. Crate and layout

One crate at the root of this folder: package `watane-dev`, lib
`watane_dev`, binary `watane-dev`. Rust edition 2024, Rust 1.95 or newer
(the `rust-version` of `wasmtime-wasi` 48).

```text
dev/
  Cargo.toml
  src/main.rs           parse the CLI, call cli::run, print, exit
  src/lib.rs            the modules below, all `pub`
  src/cli.rs            clap types; run(Cli) -> Outcome; one fn for each command
  src/outcome.rs        Outcome, Problem, Code, Class; JSON and text output
  src/env.rs            the cache folder and the limits
  src/template.rs       the embedded template; `new`
  src/check.rs          pkg rules plus the builder constraints
  src/allowlist.rs      the embedded allowlist; find and verify the vendor folder
  src/toolchain.rs      find cargo, rustc and the sysroot; check the target
  src/builder.rs        copy, lock, cargo build, dep-info check, logic.wasm
  src/unit.rs           build the unit tests; run them in Wasmtime
  src/contract.rs       the contract test file; run it on the host Harness
  src/json_eq.rs        equality of two JSON values for tests
  src/deps.rs           `add <mod folder>`; .watane-dev.json; local providers in order
  src/api.rs            the local API client; find a running host
  src/preview.rs        `serve`; find the program `watane`
  src/bin/fake-watane.rs  a fake host for tests (feature `fake-host`)
template/               the files of a new mod (section 4)
allowlist/Cargo.toml    the allowlist: direct crates, exact versions
allowlist/Cargo.lock    the allowlist: the full closure, with checksums
allowlist/lib.rs        empty; makes `allowlist/` a crate for `cargo vendor`
allowlist/vendor/       made by scripts/vendor.sh; not in git
scripts/vendor.sh       cargo vendor of the allowlist
tests/                  integration tests (section 15) and tests/fixtures/
```

`.gitignore`: `/target`, `/allowlist/vendor`.

### 1.1 Cargo.toml

```toml
[package]
name = "watane-dev"
version = "0.1.0"
edition = "2024"
rust-version = "1.95"
publish = false

[features]
fake-host = ["dep:tiny_http"]

[dependencies]
watane-pkg = { path = "../pkg" }
watane-host = { path = "../host" }
clap = { version = "4.6", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "1"
semver = "1"
blake3 = "1.8"
ureq = { version = "3", default-features = false, features = ["json"] }
wasmtime = { version = "=48.0.8", default-features = false, features = ["component-model", "cranelift", "runtime", "std"] }
wasmtime-wasi = { version = "=48.0.8", default-features = false, features = ["p2"] }
tiny_http = { version = "0.12", optional = true }

[dev-dependencies]
tempfile = "3.27"
regex = "1"

[[bin]]
name = "fake-watane"
path = "src/bin/fake-watane.rs"
required-features = ["fake-host"]
```

Version notes:
- S2 adds `toml` and `semver` to the S1 manifest. S4-2 adds direct `wasmtime`
  dependencies to run unit tests as Wasm.
- `wasmtime` and `wasmtime-wasi` are the versions of `watane-host` (48,
  the long-term-support line). The APIs used in section 7
  (`component::Linker::define_unknown_imports_as_traps`,
  `wasmtime_wasi::p2::add_to_linker_sync`, `p2::pipe::MemoryOutputPipe`,
  `WasiCtx::builder`, `Store::epoch_deadline_trap`, `StoreLimitsBuilder`)
  exist in 48.0.8.
- `ureq` without default features has no TLS. dev needs none: it speaks
  only to loopback.
- `toml` 1.x has `toml::from_str` and `toml::Table`.

## 2. The command line

```text
watane-dev [--json] [--dir <path>] <command>
  new <slug>                 --dir is the new folder; default ./<slug>
  check
  test
  serve [--stop]
  doctor
  add <mod folder>                                          (step 2)
  install [--home <path>] [--wait <seconds>]                (step 2)
  publish [--no-source] [--home <path>] [--wait <seconds>]  (step 3)
  search <text>                                             (step 4, outline)
  new <slug> --from <link>                                  (step 5, outline)
  add --release <link>                                      (step 5, outline)
```

- `--json` and `--dir` are global (`clap` `global = true`). `--dir` is the
  mod folder, default the current folder. dev canonicalizes it.
- `--wait` defaults to 600 seconds: the time that the participant has to
  answer in the app.
- With `--json`, standard output holds exactly one JSON object and a
  newline. Standard error may hold progress lines, for people only.
- Without `--json`, dev prints text for people (2.3). The text is not a
  contract.

### 2.1 The result envelope

Every command, also a failed one, prints this object:

```json
{
  "schema": 1,
  "command": "test",
  "ok": false,
  "dir": "/abs/path/of/the/mod",
  "result": {},
  "errors": [],
  "warnings": [],
  "next": "watane-dev serve --json"
}
```

| Field | Type | Meaning |
|---|---|---|
| `schema` | integer | 1. Raised on an incompatible change. |
| `command` | string | `new`, `check`, `test`, `serve`, `doctor`, `add`, `install`, `publish`. |
| `ok` | bool | `errors` is empty. |
| `dir` | string or null | The mod folder; null when the command has none (`doctor`). |
| `result` | object or null | 2.4. Present also on a failure, where a part ran. |
| `errors` | array of Problem | Empty when `ok`. |
| `warnings` | array of Problem | Never change `ok` or the exit status. |
| `next` | string or null | The command that a maker usually runs next. |

A Problem:

```json
{"code": "build.compile", "message": "cannot find value `x` in this scope",
 "file": "logic/src/lib.rs", "line": 12, "column": 5, "fix": null}
```

`file` is relative to the mod folder when it is inside it, else absolute.
`file`, `line`, `column` and `fix` are null when unknown.

### 2.2 Exit status and error codes

The exit status is the class of the first error; 0 when there is none.

| Class | Status | Meaning |
|---|---|---|
| Mod | 1 | The mod must change. The agent fixes its files. |
| Usage | 2 | The command was used wrongly. |
| Env | 3 | The device lacks something. A person or a command fixes it. |
| Refused | 4 | The host or the participant said no, or did not answer. |
| Internal | 5 | A fault of dev, or an I/O error. |

| Code | Class | When | Message (template) | Fix |
|---|---|---|---|---|
| `usage.args` | 2 | clap rejects the arguments | the clap text | `watane-dev --help` |
| `usage.not_a_mod` | 2 | the folder has no `mod.toml` | `<dir> is not a mod folder: it has no mod.toml` | `Run the command in a mod folder, or pass --dir <folder>. Create a mod with: watane-dev new <slug>` |
| `usage.slug` | 2 | `watane_pkg::Slug::from_str` refuses it | `"<s>" is not a valid slug: <pkg reason>` | `Use lowercase words of letters and digits joined by "-", each starting with a letter, at most 40 bytes, not a WIT keyword. Example: reading-list` |
| `usage.exists` | 2 | `new` into an existing path | `<path> exists` | `Choose another slug, or pass --dir <new folder>` |
| `usage.provider` | 2 | `add`: the path is not a mod folder | `<path> is not a mod folder` | — |
| `env.toolchain` | 3 | no rustup, no cargo, no rustc | `no Rust toolchain found: <detail>` | `Install rustup with your package manager, then run: rustup default stable` |
| `env.no_wasm_target` | 3 | the sysroot has no `wasm32-wasip2` | `the Rust toolchain at <sysroot> has no wasm32-wasip2 target` | `rustup target add wasm32-wasip2` |
| `env.allowlist` | 3 | the vendor folder is missing or differs from the allowlist | `the crate allowlist at <path> is missing` / `... differs from the allowlist of this watane-dev: <crate> <version>` | `Reinstall watane-dev, or set WATANE_DEV_VENDOR` |
| `env.watane_missing` | 3 | the program `watane` is not found | `the program watane was not found (<where>)` | `Install watane, or set WATANE_BIN to the program` |
| `env.preview_start` | 3 | the preview host printed an error, exited, or gave no ready line in 30 s | `the preview did not start: <host error or last 20 log lines>` | `Read <log path>` |
| `env.host_not_running` | 3 | no `run.json`, no answer, a preview in that home, or a home host with no app | `no watane app runs for the home <home>` (or the specific cause) | `Start the watane app, then run this command again` |
| `mod.invalid` | 1 | each `pkg` `Violation` | `[<rule>] <message>`, with the violation's `file` | — |
| `mod.cargo_key` | 1 | a key of `logic/Cargo.toml` outside 5.2 | ``logic/Cargo.toml: `<key>` is not allowed`` | `Remove it. Allowed: <list>` |
| `mod.dependency` | 1 | a dependency outside the allowlist, or of a refused form | ``dependency `<name>` is not in the allowlist`` / `... must be a version requirement` / `... requires <req>, the allowlist has <version>` | `Use only: <direct crates with versions>` |
| `mod.logic_file` | 1 | a file of `logic/` outside 5.3 | `logic/build.rs is not allowed: the builder runs no build script` (one text for each kind) | `Remove <file>` |
| `mod.symlink` | 1 | a symbolic link in the mod folder | `<file> is a symbolic link` | `Replace it with a file` |
| `mod.no_contract_tests` | 1 | a logic part and no `tests/*.json` | `the mod has a logic part and no contract test` | `Add tests/<name>.json (see AGENTS.md)` |
| `mod.self_dependency` | 1 | `add` of the mod itself | `a mod cannot depend on itself` | — |
| `mod.dependency_cycle` | 1 | the local providers make a cycle | `<a> -> <b> -> <a> is a cycle` | — |
| `mod.provider_missing` | 1 | a local provider has no folder in `.watane-dev.json`, or the folder is gone | `the local provider <slug> has no known folder` | `Run: watane-dev add <folder of <slug>>` |
| `build.resolve` | 1 | cargo cannot resolve with the allowlist | cargo's text | `Use only the crates and features of the allowlist: <list>` |
| `build.compile` | 1 | each rustc error | the rustc message, with its location | — |
| `build.reads_outside` | 1 | the compiler read a file outside the folders of 6.4 step 6 | `the build read <path>, outside the mod` | `Remove the include_bytes!, include_str!, include! or #[path] that names it` |
| `build.imports` | 1 | each `Violation` of `Mod::check_component` | `[component] <message>` | — |
| `test.unit` | 1 | each failed unit test | the panic message | — |
| `test.unit_limit` | 1 | a unit test binary passed the time or memory limit | `unit tests stopped at <test> after <n> s` / `... used more than 512 MiB` | — |
| `test.contract_file` | 1 | a test file does not parse or has a wrong field | serde's text, or the field | `See the contract test form in AGENTS.md` |
| `test.contract` | 1 | each failed contract step | `<file> step <n> (<call>): expected <x>, got <y>` | — |
| `publish.local_dependency` | 1 | a provider in `deps.json` is local (`null`) | `mods:<slug> is a local provider; it has no release` | `Publish <slug> first, then depend on its release` |
| `host.declined` | 4 | the host answers with error code `refused` | the host's message | — |
| `host.refused` | 4 | any other error answer of the host or the app | `[<code>] <message>` | — |
| `host.no_answer` | 4 | `--wait` passed | `no answer from the home host in <n> s` | `Run the command again with a longer --wait` |
| `internal` | 5 | anything else | the error chain | — |

Warning codes: `ui.external` (a file of `ui/` names an `http://` or
`https://` URL in `src`, `href`, `import` or `fetch(`; the content policy of
the host blocks it), `ui.inline_script` (`ui/*.html` holds a `<script>`
without `src`; the content policy blocks it), `test.uncovered` (an
exported function is called by no contract step).

### 2.3 Text output

For each error: `error[<code>]: <message>`, then `  --> <file>:<line>:<column>`
and `  fix: <fix>` when known. The same for warnings with `warning[...]`.
Then one line `<command>: ok` or `<command>: failed`, the main fields of
`result` as `key: value` lines, and `next: <next>`.

### 2.4 The result of each command

**new**
```json
{"slug": "reading-list", "dir": "/abs/reading-list", "files": ["AGENTS.md", "CLAUDE.md", "..."]}
```
`files` are relative paths, sorted. S2 `next`: `Edit wit/world.wit, logic/src/lib.rs and ui/, then run: watane-dev serve --json`.

**check** (also the `check` field of `test`)
```json
{"slug": "reading-list", "version": "0.1.0", "name": "Reading list",
 "page": true, "logic": true,
 "grants": ["storage"],
 "exports": ["mods:reading-list/api@0.1.0"],
 "events": ["mods:reading-list/changes@0.1.0"],
 "calls": [], "subscriptions": [],
 "providers": [{"slug": "a", "publisher": null, "path": "../a"}]}
```
`grants` are the `Display` texts of `watane_pkg::Grant`. The four lists
are the fields of `watane_pkg::World`. `path` is null for a published
provider.

**test**
```json
{"check": {"...": "as above"},
 "build": {"wasm": "logic.wasm", "bytes": 48213, "lock_updated": false, "seconds": 2.4},
 "unit": {"passed": 2, "failed": 0, "ignored": 0, "not_run": 0,
          "failures": [{"test": "tests::x", "message": "...", "file": "logic/src/lib.rs", "line": 40}]},
 "contract": {"files": 1, "passed": 4, "failed": 0,
              "failures": [{"file": "tests/counter.json", "step": 2, "call": "api.add",
                            "message": "...", "expected": 1, "got": 2}]}}
```
S4-2 leaves `contract` null; S4-3 supplies contract execution.
A part that did not run is null. `build`, `unit` and `contract` are null
for a mod with no logic part. `not_run` counts the tests after a test that
aborted the run (7.2). In a failure, `expected` and `got` are JSON values
or null. `next` on success: `watane-dev serve --json`; on failure:
`watane-dev test --json`.

**serve**
```json
{"home": "/abs/cache/previews/reading-list-1a2b3c4d/home",
 "api": "http://localhost:52011/api/", "reused": false,
 "pages": [{"slug": "reading-list", "local": "reading-list",
            "url": "http://reading-list.localhost:52011/"}],
 "mcp": {"url": "http://localhost:52011/mcp/preview", "toolset": "preview", "token": "wt_..."},
 "log": "/abs/cache/previews/reading-list-1a2b3c4d/watane.log"}
```
`pages` lists the mod and its local providers, in install order (providers
first); `url` is null for a mod with no page. The token reaches the tools
as `Authorization: Bearer <token>`. **serve --stop**:
`{"stopped": true, "home": "..."}`; `stopped` is false when none ran, and
that is not an error.

**doctor**
```json
{"toolchain": {"cargo": "/abs", "rustc": "/abs", "sysroot": "/abs", "version": "rustc 1.97.1 ...", "wasm32_wasip2": true},
 "allowlist": {"vendor": "/abs", "direct": ["serde 1.0.229", "serde_json 1.0.154", "wit-bindgen 0.63.0"], "crates": 57},
 "watane": "/abs/watane",
 "home": {"path": "/abs/.watane", "running": false, "app": false}}
```
A missing part is null and adds its error. `doctor` reports every gap, not
only the first. A home host that does not run is no error.

**add** (step 2)
```json
{"slug": "a", "version": "0.1.0", "path": "../a", "wit_dir": "wit/deps/a",
 "imports": ["import mods:a/api@0.1.0;"],
 "subscriptions": ["export mods:a/changes@0.1.0;"]}
```
`next`: `Copy the lines that the mod needs into the world in wit/world.wit. Each line is a grant that the participant approves.`

**install** (step 2)
```json
{"home": "/abs/.watane", "installed": [{"slug": "a", "local": "a", "version": "0.1.0",
  "package": "<64 hex>", "page": "http://a.localhost:47341/"}]}
```
From the host's `InstalledMod` list, in install order.

**publish** (step 3)
```json
{"home": "/abs/.watane",
 "release": "<publisher key>/reading-list@0.1.0",
 "package": "<64 hex>", "source": true, "site": "/abs/site folder", "link": "watane:...#reading-list"}
```
`release`, `site` and `link` come from the app's answer; `link` is null
while no site URL is set. `next`: `Put the site folder on your static web host`.

## 3. Environment

| Variable | Use | Default |
|---|---|---|
| `WATANE_DEV_CACHE` | the cache folder | macOS: `~/Library/Caches/watane-dev`; else `$XDG_CACHE_HOME/watane-dev`, else `~/.cache/watane-dev` |
| `WATANE_DEV_VENDOR` | the vendor folder of the allowlist | 6.2 |
| `WATANE_DEV_TOOLCHAIN` | a toolchain folder with `bin/cargo` and `bin/rustc` | `rustup which` (6.1) |
| `WATANE_DEV_UNIT_SECONDS` | the time limit of one unit test binary | 60 |
| `WATANE_BIN` | the program `watane` | 9.1 |
| `WATANE_HOME` | the home of the participant | read through `watane_host::default_home()` |

The cache folder (dev creates and canonicalizes it):

```text
<cache>/
  cargo-home/            CARGO_HOME of every cargo run
  tmp/                   TMPDIR of every cargo run
  build/<slug>/          the copy of the mod sources for one build
  build/<slug>.lock      a file lock for the duration of a build of that slug
  target/                the shared cargo target folder of all mods
  previews/<slug>-<h8>/  h8: the first 8 hex characters of BLAKE3(canonical mod path)
    home/                the preview home
    ready.json           the ready line of the preview host (mode 0600)
    watane.log           standard error of the preview host
```

`.watane-dev.json` in a mod folder (written by `add`, read by `serve`,
`install` and `publish`) holds the folders of local providers. `pkg`'s
`pack` never includes it.

```json
{
  "local_providers": {
    "reading-list": "../reading-list"
  }
}
```

Keys are slugs; values are paths relative to the mod folder, with `/`.
dev writes it with sorted keys, two-space indent and a final newline.

## 4. The template

`new <slug>` checks the slug with `watane_pkg::Slug::from_str` and writes
these files. The sources are in `template/` and enter the binary with
`include_str!`. dev replaces the placeholders:

| Placeholder | Value |
|---|---|
| `{{slug}}` | the slug, e.g. `reading-list` |
| `{{crate}}` | the slug with `-` replaced by `_` |
| `{{title}}` | the slug with `-` replaced by a space, first letter upper case |
| `{{crates}}` | one line `- <name> <version>` for each direct crate of `allowlist/Cargo.toml` |

After the replacement no `{{` may remain; a unit test checks this.

| File | Source |
|---|---|
| `AGENTS.md` | `template/AGENTS.md` |
| `CLAUDE.md` | one line: `@AGENTS.md` |
| `.gitignore` | `logic.wasm` and `logic/target/` |
| `mod.toml` | `template/mod.toml` |
| `wit/world.wit` | `template/wit/world.wit` |
| `wit/deps/watane-host/host.wit` | `watane_host::HOST_WIT` |
| `logic/Cargo.toml` | `template/logic/Cargo.toml.tmpl` (`.tmpl` keeps cargo away from it) |
| `logic/src/lib.rs` | `template/logic/src/lib.rs` |
| `ui/index.html` | `template/ui/index.html` |
| `ui/app.js` | `template/ui/app.js` |
| `tests/counter.json` (S4) | `template/tests/counter.json` |

`new` writes no `deps.json` (absent means no provider) and no
`logic/Cargo.lock` (the first build writes it, 6.4). If a write fails,
dev removes the new folder.

### 4.1 Contents

`template/mod.toml` (pkg: the slug and the version are the package line
of the world):
```toml
name = "{{title}}"
summary = "A counter. Replace this with one sentence on what the mod does."
```

`template/wit/world.wit`:
```wit
package mods:{{slug}}@0.1.0;

/// The calls of this mod. Its page, agents and other mods call them.
interface api {
  /// Adds one to the count and returns the new count.
  add: func() -> u32;
  /// Returns the count.
  get: func() -> u32;
}

/// The events of this mod. Another mod receives them when it exports
/// this interface and the participant grants it.
interface changes {
  /// The count changed.
  counted: func(count: u32);
}

world {{slug}} {
  import watane:host/state@0.1.0;
  import changes;
  export api;
}
```

`template/logic/Cargo.toml.tmpl`:
```toml
[package]
name = "{{slug}}"
version = "0.1.0"
edition = "2024"
publish = false

[lib]
crate-type = ["cdylib"]

[dependencies]
wit-bindgen = "0.63"
serde_json = "1"

[workspace]
```

`template/logic/src/lib.rs`:
```rust
//! A counter, stored as JSON in the private state of the mod.
wit_bindgen::generate!({ path: "../wit", world: "{{slug}}", generate_all });

use exports::mods::{{crate}}::api::Guest;
use mods::{{crate}}::changes;
use watane::host::state;

struct Mod;

impl Guest for Mod {
    fn add() -> u32 {
        let count = next(decode(state::get("count").as_deref()));
        state::put("count", &encode(count));
        changes::counted(count);
        count
    }

    fn get() -> u32 {
        decode(state::get("count").as_deref())
    }
}

export!(Mod);

/// Reads a stored count. A missing or broken value counts as 0.
fn decode(bytes: Option<&[u8]>) -> u32 {
    bytes.and_then(|b| serde_json::from_slice(b).ok()).unwrap_or(0)
}

fn encode(count: u32) -> Vec<u8> {
    serde_json::to_vec(&count).expect("a u32 always encodes")
}

/// The next count. It stays at the maximum instead of overflowing.
fn next(count: u32) -> u32 {
    count.saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_missing_and_broken_values_as_zero() {
        assert_eq!(decode(None), 0);
        assert_eq!(decode(Some(b"41")), 41);
        assert_eq!(decode(Some(b"bad")), 0);
        assert_eq!(decode(Some(b"-1")), 0);
    }

    #[test]
    fn counts_up_and_stays_at_the_maximum() {
        assert_eq!(next(0), 1);
        assert_eq!(next(u32::MAX), u32::MAX);
        assert_eq!(encode(42), b"42");
    }
}
```

`template/tests/counter.json`:
```json
{
  "steps": [
    {"call": "api.get", "returns": 0},
    {"call": "api.add", "returns": 1,
     "emits": [{"event": "changes.counted", "args": {"count": 1}}]},
    {"call": "api.add", "returns": 2},
    {"call": "api.get", "returns": 2}
  ]
}
```

`template/ui/index.html`: about 15 lines; `<meta charset>`, a viewport
meta, a title `{{title}}`, a count, an "Add" button, an element with
`role="alert"`, then `<script src="/watane.js"></script>` and
`<script src="app.js"></script>`. No inline script: the content policy of
the host refuses it.

`template/ui/app.js`:
```js
const count = document.getElementById('count');
const alert = document.getElementById('error');
const show = (value) => { count.textContent = String(value); alert.textContent = ''; };
const fail = (error) => { alert.textContent = error.message; };
const read = () => watane.call('api.get').then(show, fail);
document.getElementById('add').addEventListener('click', () => watane.call('api.add').then(show, fail));
watane.on((message) => {
  if (message.type === 'event' && message.function === 'counted') show(message.args.count);
  else if (message.type === 'changed') read();
});
```

`counted` events carry the count directly; `changed` triggers a read.
The stream also dispatches `changed` when it opens, so no separate first
read is needed (host 8.4–8.5).

`template/AGENTS.md`: the guide for the coding agent, at most 150 lines,
with these sections in this order (S4-2 includes 1–7, 9 and 10; section 8
joins with its slice):
1. **This folder**: a table of the files above and who edits each.
   `wit/deps/`, `deps.json` and `.watane-dev.json` are written by
   `watane-dev`; do not edit them.
2. **Order of work**: edit `wit/world.wit`, then `logic/src/lib.rs`, then
   `ui/`; run `watane-dev test --json` until `ok` is true, then
   `watane-dev serve --json` and give the page URL to the person.
3. **The world**: keep `package mods:{{slug}}@<version>;` and
   `world {{slug}}`; the version of the package line is the version of
   the mod. Each import is a grant that the participant approves, so import
   only what the mod calls. An import of an interface of the own package
   sends events. WIT names are kebab-case; a keyword as a name takes `%`.
4. **The logic part**: how WIT names map to Rust
   (`exports::mods::{{crate}}::<iface>::Guest`, kebab to snake case, the
   types in the module of their interface). The host interfaces are in
   `wit/deps/watane-host/host.wit`; read its comments. A panic fails the
   call and undoes its writes. There are no files, no environment variables
   and no sockets.
5. **Crates**: only these, with these versions: `{{crates}}`. No build
   script, no procedural macro, no `.cargo/`, no `rust-toolchain` file.
6. **The page**: scripts only from files in `ui/` (no inline script, no
   CDN, no other site); `watane.call("<iface>.<fn>", args)` returns the
   value or throws an error with `code` and `message`;
   `watane.on(handler)` gets `{"type":"changed"}` and
   `{"type":"event", ...}`; the JSON form of WIT values (the table of host
   `TECHNICAL.md` 6.9: records as objects, `option` as null or the value,
   variants as `"case"` or `{"case": payload}`, `result` as `{"ok": v}` or
   `{"err": e}`).
7. **Tests**: unit tests run as WebAssembly with no files, no environment
   and no host imports: test pure functions there. A panic ends the whole
   unit test run, so the tests after it do not run. Contract tests: the
   form of 8.1 in short, with an `emits` example. Contract execution,
   fake and `deliver` examples arrive in S4-3 with links.
8. **Building on another mod**: `watane-dev add <mod folder> --json`, then
   copy the needed lines of `result.imports` or `result.subscriptions`
   into the world.
9. **Results**: the envelope fields, the exit statuses, and the advice
   "read `errors[].fix`; for `build.compile` read `file` and `line`".
10. **Rules**: do not ask questions; finish with a passing
    `watane-dev test --json`, then `watane-dev serve --json`.

## 5. Check

### 5.1 Order

`check` (and the first part of `test`, `serve`, `install`, `publish`) runs
these stages. It collects all errors of a stage and stops after the first
stage with an error.

1. `<dir>/mod.toml` exists, else `usage.not_a_mod`.
2. dev writes `wit/deps/watane-host/host.wit` when its bytes differ from
   `watane_host::HOST_WIT`.
3. **Files**: no symbolic link anywhere in the mod folder (`logic/target/`
   and `.git/` are skipped): `mod.symlink`. The files of `logic/`: 5.3.
4. **pkg**: `watane_pkg::Mod::read(dir)`. Each `Violation` of
   `ModError::Invalid` is one `mod.invalid` with its file; `ModError::Io`
   is `internal`.
5. **Cargo manifest**: 5.2, when `logic/` exists.
6. **Contract tests** (S4-2): when the mod has a logic part, at least one
   regular `tests/*.json` file (not in subfolders), else `mod.no_contract_tests`.
7. **Warnings**: `ui.external`, `ui.inline_script`.

### 5.2 `logic/Cargo.toml`

dev parses it with `toml` into a `toml::Table` and allows only these keys.
Any other key, at any level, is `mod.cargo_key` with its dotted path.
`mod.cargo_key` and `mod.dependency` include the offending key's line,
using the TOML parser's key spans; a missing required key has no line.

| Key | Allowed value |
|---|---|
| `package.name` | any string |
| `package.version` | any string |
| `package.edition` | `"2021"` or `"2024"` |
| `package.publish`, `package.description`, `package.license`, `package.authors` | any |
| `lib.crate-type` | exactly `["cdylib"]` (required) |
| `dependencies.<name>`, `dev-dependencies.<name>` | a version string, or a table with only `version` (required), `features`, `default-features` |
| `workspace` | an empty table (required) |

Thus `build`, `links`, `[build-dependencies]`, `[lib] proc-macro`,
`[patch]`, `[replace]`, `[profile]`, `[features]`, `[target]`,
`cargo-features`, and the dependency keys `path`, `git`, `registry`,
`package` and `optional` are refused.

Each `<name>` must be a crate of the allowlist, and its `version`
requirement must match the allowlisted version (`semver::VersionReq`).
Else `mod.dependency`.

### 5.3 Files of `logic/`

Allowed: `logic/Cargo.toml`, `logic/Cargo.lock`, files under
`logic/src/` that end in `.rs`, and anything under `logic/target/` (never
read). Every other file is `mod.logic_file`, with a reason for these:

| File | Reason in the message |
|---|---|
| `logic/build.rs` | the builder runs no build script |
| `logic/.cargo/*` | the builder uses no cargo configuration of the mod |
| `logic/rust-toolchain`, `logic/rust-toolchain.toml` | the builder chooses the toolchain |
| any other | only Cargo.toml, Cargo.lock and src/**/*.rs are built |

## 6. The builder

### 6.1 Toolchain

1. If `WATANE_DEV_TOOLCHAIN` is set, `cargo` and `rustc` are
   `<it>/bin/cargo` and `<it>/bin/rustc`.
2. Else dev finds `rustup` on `PATH`, then in `$CARGO_HOME/bin`, then in
   `~/.cargo/bin`, and runs `rustup which cargo` and `rustup which rustc`
   with the cache folder as working folder (so no toolchain file of a mod
   applies). This avoids a packaged `rustc` that shadows rustup on `PATH`
   (colos `explore/dev-onboarding`, finding 2).
3. Else `env.toolchain`.
4. `<rustc> --print sysroot` gives the sysroot. If
   `<sysroot>/lib/rustlib/wasm32-wasip2/lib` is not a folder:
   `env.no_wasm_target`, and dev runs no cargo.
5. `<rustc> --version` gives the version for `doctor`.

### 6.2 Allowlist

The allowlist is the closed set of crates that a logic part may use.

- `allowlist/Cargo.toml` names the direct crates with exact versions:
  `wit-bindgen = "=0.63.0"`, `serde = { version = "=1.0.229", features =
  ["derive"] }`, `serde_json = "=1.0.154"`. It has `[workspace]` and
  `[lib] path = "lib.rs"`.
- `allowlist/Cargo.lock` is made once with `cargo generate-lockfile` and
  kept in git. It is the full closure, with checksums.
- Both files enter the binary with `include_str!` (`allowlist::MANIFEST`,
  `allowlist::LOCK`).
- `scripts/vendor.sh` runs
  `cargo vendor --locked --versioned-dirs --manifest-path allowlist/Cargo.toml allowlist/vendor`
  (add `--offline` when the crates are in the local registry cache). The
  packager runs it and installs the folder as
  `<prefix>/share/watane-dev/vendor`.
- dev finds the vendor folder in this order: `WATANE_DEV_VENDOR`;
  `<folder of the executable>/../share/watane-dev/vendor`;
  `<CARGO_MANIFEST_DIR at compile time>/allowlist/vendor`.
- **Verification**, before each build: the vendor folder holds exactly one
  folder `<name>-<version>` for each registry package of the lock, and no
  other folder. The `package` field of its `.cargo-checksum.json` equals
  the `checksum` of the lock. Else `env.allowlist`.
- A change of the allowlist is a change of dev, with a review of each new
  crate, its build script and its procedural macros.

### 6.3 The copy

The build never runs in the mod folder. Under the lock
`<cache>/build/<slug>.lock` (`File::lock`), dev deletes
`<cache>/build/<slug>/` and copies into it only `wit/**`,
`logic/Cargo.toml` and `logic/src/**/*.rs`. The mod's `Cargo.lock` is not
copied: dev makes a new one (6.4). dev removes `<mod>/logic.wasm` before
the build, so a failed build leaves none.

### 6.4 Cargo

All cargo runs use one command builder, `builder::cargo(...)`:

- Program: the `cargo` of 6.1. Working folder: `<cache>/build/<slug>/logic`.
- Environment: `env_clear()`, then only:
  `PATH=<folder of cargo>:/usr/bin:/bin`, `RUSTC=<rustc>`,
  `CARGO_HOME=<cache>/cargo-home`, `TMPDIR=<cache>/tmp`,
  `CARGO_TERM_COLOR=never`, `CARGO_INCREMENTAL=0`, and `SDKROOT` and
  `DEVELOPER_DIR` when the parent has them (macOS needs them to link the
  build scripts of allowlisted crates). Thus `RUSTFLAGS`, `RUSTC_WRAPPER`,
  proxies and every secret of the maker stay out (DEV-F-15). If cargo needs
  `HOME`, set `HOME=<cache>/cargo-home`.
- Arguments common to every run (`<x>` is a canonical absolute path; each
  `--config` value is TOML, quoted with `toml::Value::String(..).to_string()`):

```text
--manifest-path <cache>/build/<slug>/logic/Cargo.toml
--offline
--config build.target-dir="<cache>/target"
--config source.crates-io.replace-with="watane-allowlist"
--config source.watane-allowlist.directory="<vendor>"
--config target.wasm32-wasip2.rustflags=["--remap-path-prefix=<cache>=/watane-dev","--remap-path-prefix=<vendor>=/vendor","--remap-path-prefix=<sysroot>=/toolchain"]
--config profile.release.debug=false
--config profile.release.codegen-units=1
```

`build.target-dir` is a config value shared by both runs;
`generate-lockfile` does not accept a `--target-dir` argument.

The rustflags are under `[target.wasm32-wasip2]`, because target
rustflags elsewhere would hide `build.rustflags` (colos commit 33d5fb6).
rustc applies the last matching remap, so the more specific prefix comes
later. The flags are the same for every mod, so the target folder stays
warm.

The runs, in order:

1. `cargo generate-lockfile <common>`. Failure: `build.resolve`.
2. Every package of the new `Cargo.lock`, except the mod itself, must be
   in `allowlist::LOCK` with the same version and checksum. Else
   `env.allowlist`.
3. If the new lock differs from `<mod>/logic/Cargo.lock`, dev writes it
   there; `lock_updated` is true.
4. Before the reads check (deferred in S2), dev deletes
   `<cache>/target/wasm32-wasip2/release/deps/<crate>-*.d`
   (`<crate>`: the package name with `-` replaced by `_`).
5. `cargo build <common> --locked --release --target wasm32-wasip2 --message-format json`.
   dev reads standard output line by line as JSON:
   - `reason: "compiler-message"` with `message.level` `error`: one
     `build.compile`. `message` is `message.message`; the location is the
     primary span; its `file_name` is mapped from the copy to the mod
     folder.
   - `reason: "compiler-artifact"` whose `target.kind` holds `cdylib`: the
     `.wasm` file in `filenames`.
   - On a failure with no `compiler-message` error: `build.resolve` with
     the last 40 lines of standard error.
6. **Reads** (DEV-S-10; deferred beyond S2): dev reads the new `deps/<crate>-*.d` files (make syntax:
   `target: dep dep ...`, with `\ ` for a space in a path). Every path in
   them must lie under the copy folder, the vendor folder, the sysroot or
   `<cache>/target`. Else one `build.reads_outside` for each path, and dev
   writes no `logic.wasm` (DEV-F-14). rustc lists the files of
   `include_bytes!`, `include_str!`, `include!` and `#[path]` there.
7. dev copies the `.wasm` to `<mod>/logic.wasm`, then runs
   `watane_pkg::Mod::read(dir)?.check_component()`. Each violation is
   `build.imports`, and dev deletes `<mod>/logic.wasm` again.

### 6.5 Reproducible output (DEV-S-12; deferred beyond S2)

The output holds no path of the device: the copy, the target folder and the
cargo home are under `<cache>` (remapped to `/watane-dev`), the crates under
`<vendor>` (`/vendor`), std under `<sysroot>` (`/toolchain`). The copy path
is `/watane-dev/build/<slug>/` on every device. One codegen unit and no
debug info remove the remaining variation. A `~/.cargo/config.toml` of the
maker is still read by cargo, because it is in a parent folder of the
cache; it is the maker's own choice and can change the output.

## 7. Unit tests

### 7.1 Build and run

1. Under the same per-slug build lock, dev prepares a fresh checked cache
   copy and verified lock as in 6.3–6.4, then runs `cargo test <common> --locked --no-run --target wasm32-wasip2 --message-format json`
   (after 6.4 step 1). Compiler errors: as 6.4 step 5. The test binaries
   are the `executable` fields of `compiler-artifact` messages with
   `profile.test` true. The reads check of 6.4 step 6, including deleting and scanning the new
   `debug/deps/<crate>-*.d` files, remains deferred with DEV-S-10.
2. dev runs each binary as a `wasi:cli/command` component in Wasmtime:
   - `Config`: `epoch_interruption(true)`. A thread calls
     `engine.increment_epoch()` every 100 ms; the store deadline is
     `WATANE_DEV_UNIT_SECONDS × 10` ticks, with `epoch_deadline_trap()`.
   - `Store` limiter: `StoreLimitsBuilder::new().memory_size(512 << 20)`
     with `trap_on_grow_failure(true)`; shared Wasm memories are disabled.
   - `Linker`: `wasmtime_wasi::p2::add_to_linker_sync`, then register
     trapping functions for non-WASI imports, so a call to `watane:host`
     or to a provider traps instead of failing to link. In 48.0.8,
     `define_unknown_imports_as_traps(&component)` also stubs resource
     aliases inside already linked WASI interfaces (for example
     monotonic-clock's `pollable`), causing mismatched resource identities.
     Dev therefore leaves typed WASI instances intact; this grants no
     capability. Unsupported WASI imports fail to link.
   - `WasiCtx::builder()`: args `["test", "--test-threads=1", "--nocapture"]`, stdout and
     stderr each a `MemoryOutputPipe::new(1 << 20)`, no environment, no
     preopened folder, no network (the default denies every address),
     empty stdin. TCP, UDP and IP name lookup are explicitly disabled.
   - `p2::bindings::sync::Command::instantiate` then
     `wasi_cli_run().call_run(&mut store)`.

### 7.2 Reading the output

`--nocapture` sends panic diagnostics to the pipe before an abort; default
libtest capture would lose them. The parser follows the current test across
any intervening stdout from that test until its status or an abort.

libtest prints `running <n> tests`, then `test <name> ... ok|FAILED|ignored`.
On `wasm32-wasip2` a panic aborts the whole binary (panic strategy
`abort`): the output then ends after `test <name> ... `, and stderr holds
`thread '<..>' panicked at /watane-dev/build/<slug>/<file>:<line>:<col>:`
and the message. dev reports:
- each `FAILED` line, and the aborted test, as `test.unit`, with the panic
  message and the location mapped to the mod folder;
- `not_run` = n − passed − failed − ignored;
- the epoch trap as `test.unit_limit` naming the last started test; a
  memory failure as `test.unit_limit`. Reports retain the counts and failed
  test even on a limit. Failure classification is private to `UnitFailure`
  and is not serialized.

## 8. Contract tests

### 8.1 The file

Every file `tests/*.json` (not in subfolders), in byte order of the name.
Each is one JSON object. Unknown fields are errors.

```json
{
  "fakes": {
    "mods:reading-list/books": {
      "all": ["Dune", "Emma"],
      "add": {"ok": null}
    }
  },
  "steps": [
    {"call": "suggest.next", "returns": "Dune"},
    {"deliver": "events.added", "args": {"title": "Emma"}},
    {"call": "api.add", "returns": 1,
     "emits": [{"event": "changes.counted", "args": {"count": 1}}]},
    {"call": "api.clear", "fails": "trap"}
  ]
}
```

| Field | Meaning |
|---|---|
| `fakes` | Optional. Key: an interface of another package without version (`mods:<slug>/<iface>`); a key with `@` is `test.contract_file`. Value: function name → the JSON value that the fake returns, always. This is `watane_host::Fakes` as it is. |
| `steps` | Required, at least one. Run in order, on one host, so state carries from step to step. |
| `call` | An export function as `watane.js` names it (`<iface>.<fn>`). Runs as the page. |
| `deliver` | Instead of `call`: a subscription export (`<iface>.<fn>`, the short name of the provider's interface), called as the host calls it for an event. |
| `args` | Optional, default `{}`: an object by parameter name or an array in order. |
| `returns` | The call must succeed with this value. |
| `fails` | `true`: the call must fail. A string: the call must fail, and `"<code>: <message>"` of the error must contain it (`"not_granted"`, `"trap"`, a word of a panic). |
| neither | The call must succeed; any value. |
| `emits` | Optional. The call must send exactly these events, in this order. `event` is `<iface>.<fn>` for an interface of the own package, or `mods:<slug>/<iface>.<fn>`. `args` is an object by parameter name. A failed call sends none. |

Exactly one of `call` and `deliver`; at most one of `returns` and `fails`.
Values use the JSON encoding of host `TECHNICAL.md` 6.9. Two values are
equal when `json_eq` holds: the same type and members, object members in
any order, numbers by value (`1` equals `1.0`).

### 8.2 The run

For each file: `watane_host::Harness::open(dir, fakes)` (a new temporary
home with only this mod, fresh state), then each step through
`Harness::call` or `Harness::deliver`, compared by 8.1. An event matches
an `EventRecord` when the function names are equal, the interface names
are equal without their versions (a short `event` name is prefixed with
`mods:<own slug>/`), and the args are `json_eq`. A host error at `open` is
one `test.contract` for the file. `contract::run_steps` takes the call
function as a closure parameter, so its unit tests use a closure instead
of the host. `test.uncovered` warns for each function of
`Harness::functions()` (as `<short iface>.<name>`) that no step of any
file names.

## 9. Preview and the home host

### 9.1 Finding programs and hosts

- **The program `watane`**: `WATANE_BIN` (if set and missing:
  `env.watane_missing`); else `watane` beside the running executable; else
  `watane` on `PATH`; else `env.watane_missing`.
- **A running host**: `watane_host::RunInfo::read(home)` gives the port;
  the API base is `http://localhost:<port>/api/`. The host runs when
  `GET <base>status` answers 200 within 2 s, with `home` equal to the
  canonical home. dev never connects to a host other than `localhost`.
- **The HTTP client**: one `ureq::Agent` with `proxy(None)` and a global
  timeout (2 s for `status`, `--wait` for install and publish, 60 s in a
  preview). dev sends no `Origin` header. An error answer is
  `{"error": {"code": "...", "message": "..."}}`: code `refused` is
  `host.declined`; any other code is `host.refused`; a timeout is
  `host.no_answer`.

### 9.2 Local providers and order

`deps::local_order(dir)` follows each provider of `Mod::deps()` with no
publisher, through the folder that `.watane-dev.json` names for its slug
(`mod.provider_missing` when there is none or it has no `mod.toml`), depth
first, and returns the folders with providers before their users and `dir`
last. A folder seen twice on one path is `mod.dependency_cycle`. Each
folder runs the `test` pipeline (sections 5 to 8). The first folder that
fails stops the command. Each Problem of a provider gets the prefix
`<provider slug>: ` in its message, and its `file` is absolute.

### 9.3 `serve`

1. S2 runs check (5.1–5.3) and build (6.1–6.4) for the mod.
   Unit and contract tests join in S4; provider traversal joins with `add`.
2. Preview folder `P = <cache>/previews/<slug>-<h8>/`, home `P/home`.
   If the host of `P/home` runs (9.1) and `P/ready.json` exists: reuse it
   (`reused` true). If it runs and `P/ready.json` is missing: stop it
   (9.4 steps 1 to 3), then start it.
3. Start: spawn `<watane> --preview <P/home>` with stdin null, stdout
   piped, stderr appended to `P/watane.log`, and on Unix
   `process_group(0)`, so the preview outlives dev and the agent's shell. A
   thread reads one line of stdout and ends. dev waits up to 30 s:
   - a line that parses as `watane_host::Ready`: write it to
     `P/ready.json` with mode 0600;
   - a line `{"watane":1,"error":...}`, an exit of the child, or 30 s:
     `env.preview_start` with the host error or the last 20 log lines.
   The host writes nothing more to stdout after the ready line, so closing
   the pipe is safe.
4. `POST <api>install` with `{"folders": [<absolute folders of 9.2>]}`.
   The preview grants what each mod declares and asks nothing (W5). A
   folder with the slug of an installed local mod updates it; its state
   stays, because the host keys state by identity (`local/<slug>`).
5. Report `pages` from the `installed` answer (`slug`, `local`, `page`),
   and `api`, `mcp` and `token` from `P/ready.json`.

### 9.4 `serve --stop`

Stopping never runs `check` or reads the mod's manifest or world. Canonicalize
its folder, compute h8 as in section 3, and find `<cache>/previews/*-<h8>`.
Apply the steps below to every matching preview folder `P`, so stopping also
works after broken edits or a slug change. Report `stopped` true if any host
was stopped; `home` is the first matching home in sorted path order, or null
when no preview folder matches.

1. `RunInfo::read(P/home)`; if none, or the host does not answer with
   `home` = `P/home` in `/api/status`, go to step 4 with `stopped` false.
2. Run `kill -TERM <pid>`.
3. Wait up to 10 s, polling every 100 ms, until `kill -0 <pid>` fails or
   `run.json` is gone.
4. Delete `P`.

### 9.5 `add <mod folder>` (step 2)

1. `Mod::read(provider)` (else `usage.provider`). The same canonical folder
   as this mod is `mod.self_dependency`. A provider whose
   `local_order` contains this folder is `mod.dependency_cycle`.
2. `watane_pkg::add_provider(dir, &provider, None)`: it copies the WIT into
   `wit/deps/<slug>/` and sets `"mods:<slug>": null` in `deps.json`. Its
   errors are `mod.invalid`.
3. Set `local_providers.<slug>` in `.watane-dev.json` to the path of the
   provider relative to this folder.
4. Report an `import <iface>;` line for each interface of
   `provider.world().exports` and an `export <iface>;` line for each of
   `provider.world().events` (full names with version).

### 9.6 `install` (step 2)

Home: `--home`, else `watane_host::default_home()`. `GET /api/status` must
say `mode: "home"` and `app: true`; else `env.host_not_running` with the
cause ("the host of <home> is a preview" / "the host of <home> runs
without the app"). Run 9.2, then one `POST /api/install` with
`{"folders": [...]}`. The host packs the folders, plans, and waits for the
participant in the app. dev reports the `installed` answer.

### 9.7 `publish` (step 3)

1. Each member of `Mod::deps()` with no publisher is one
   `publish.local_dependency`, before any request.
2. Test the mod (sections 5 to 8); `watane_pkg::pack(dir, !no_source)`.
3. `POST <api>publish`, `Content-Type: application/octet-stream`, the
   package bytes as the body, on the home host (status as in 9.6). The app
   asks the participant, who sees whether the source is included; `dist`
   signs and writes the site folder. dev reports the answer, plus
   `package` (`Package::id()`) and `source` (`Package::has_source()`).

## 10. Steps 4 and 5 (outline; not designed)

Only enough to keep the design open. `search` sends
`POST /api/search {"text"}`; `new --from <link>` and `add --release <link>`
send `POST /api/fetch {"link"}` and read the package file that the answer
names (app `TECHNICAL.md` 8.2, 8.3). `new --from` unpacks it with
`Package::unpack`, sets the new slug in the package line and the origin
in `mod.toml`. `add --release` copies the provider's WIT as 9.5 does and
writes the publisher instead of a `.watane-dev.json` path. Nothing in
sections 2 to 9 blocks this: the envelope, the client, `deps.json` with its
publisher, and the provider order already carry these cases.

## 11. Security properties

| Property | How it is enforced |
|---|---|
| No code of the mod runs natively | No build script, procedural macro, cargo config or toolchain file in the mod (5.2, 5.3); the copy holds only sources (6.3); unit tests run in Wasmtime (7); contract tests run in the host sandbox (8). |
| Allowlisted crates are fixed | The lock is compiled in; the vendor folder is verified against it before each build (6.2); cargo resolves only from it (6.4). Their build scripts and procedural macros run natively: they are reviewed when the allowlist changes. |
| The build is offline | `--offline`; the only source is a local folder; no proxy and no network setting reach cargo (`env_clear`). |
| No secret enters the output | `env_clear` (DEV-F-15); the dep-info check refuses any file read outside the mod, the allowlist, the sysroot and the target folder (6.4 step 6, 7.1). |
| No path of the device in the output | The remaps of 6.5. |
| Unit tests are sandboxed | No folder, environment or network; fixed args; time, memory and output limits (7.1). |
| dev has no authority | No key; requests go only to `localhost`, with no proxy; each install and publish in the home waits for the participant in the app (F-AGENT-4); the home host refuses without the app. |
| The preview has no participant data | Its own home under the cache, in preview mode (W5). |

Not protected (as `spec/TECHNICAL.md` section 6): another program of the
same user, which can change the cache, the vendor folder or the mod folder,
or read the preview token in `ready.json`; the maker's own
`~/.cargo/config.toml`.

## 12. Public Rust API

All in crate `watane_dev`. Errors are `Problem` values; dev has no other
error type. `Vec<Problem>` carries all errors of a stage.

```rust
// outcome.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(into = "&'static str")]
pub enum Code { UsageArgs, UsageNotAMod, UsageSlug, UsageExists, UsageProvider,
    EnvToolchain, EnvNoWasmTarget, EnvAllowlist, EnvWataneMissing, EnvPreviewStart,
    EnvHostNotRunning, ModInvalid, ModCargoKey, ModDependency, ModLogicFile, ModSymlink,
    ModNoContractTests, ModSelfDependency, ModDependencyCycle, ModProviderMissing,
    BuildResolve, BuildCompile, BuildReadsOutside, BuildImports, TestUnit, TestUnitLimit,
    TestContractFile, TestContract, PublishLocalDependency, HostDeclined, HostRefused,
    HostNoAnswer, Internal, UiExternal, UiInlineScript, TestUncovered }
impl Code { pub fn as_str(self) -> &'static str; pub fn class(self) -> Class; }
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Class { Mod = 1, Usage = 2, Env = 3, Refused = 4, Internal = 5 }
#[derive(Debug, Clone, Serialize)]
pub struct Problem { pub code: Code, pub message: String, pub file: Option<String>,
    pub line: Option<u32>, pub column: Option<u32>, pub fix: Option<String> }
impl Problem { pub fn new(code: Code, message: impl Into<String>) -> Problem;
    pub fn at(self, file: impl Into<String>, line: Option<u32>, column: Option<u32>) -> Problem;
    pub fn fix(self, fix: impl Into<String>) -> Problem; }
#[derive(Debug, Serialize)]
pub struct Outcome { pub schema: u32, pub command: &'static str, pub ok: bool,
    pub dir: Option<PathBuf>, pub result: Option<serde_json::Value>,
    pub errors: Vec<Problem>, pub warnings: Vec<Problem>, pub next: Option<String> }
impl Outcome { pub fn exit_code(&self) -> u8; pub fn to_json(&self) -> String; pub fn to_text(&self) -> String; }

// cli.rs (clap attributes left out)
pub struct Cli { pub json: bool, pub dir: Option<PathBuf>, pub command: Command }
pub enum Command { New { slug: String }, Check, Test, Serve { stop: bool }, Doctor,
    Add { provider: PathBuf },
    Install { home: Option<PathBuf>, wait: u64 },
    Publish { no_source: bool, home: Option<PathBuf>, wait: u64 } }
pub fn run(cli: Cli) -> Outcome;

// env.rs
pub struct Env { pub cache: PathBuf, pub unit_seconds: u64 }
impl Env { pub fn from_process() -> Result<Env, Problem>; }

// template.rs
#[derive(Serialize)] pub struct NewReport { pub slug: String, pub dir: PathBuf, pub files: Vec<String> }
pub fn new_mod(dir: &Path, slug: &str) -> Result<NewReport, Problem>;
pub fn render(text: &str, slug: &watane_pkg::Slug) -> String;

// S2 embeds its allowlist in check; the check entry point remains
// check(dir: &Path) -> Result<(watane_pkg::Mod, CheckReport), Vec<Problem>>.
// The full-pipeline Findings signature below joins with warnings later.
// check.rs
pub struct Findings { pub errors: Vec<Problem>, pub warnings: Vec<Problem> }
#[derive(Serialize)] pub struct ProviderReport { pub slug: String, pub publisher: Option<String>, pub path: Option<String> }
#[derive(Serialize)] pub struct CheckReport { pub slug: String, pub version: String, pub name: String,
    pub page: bool, pub logic: bool, pub grants: Vec<String>, pub exports: Vec<String>,
    pub events: Vec<String>, pub calls: Vec<String>, pub subscriptions: Vec<String>,
    pub providers: Vec<ProviderReport> }
pub fn check(dir: &Path, allow: &Allowlist) -> (Option<(watane_pkg::Mod, CheckReport)>, Findings);
pub fn check_manifest(text: &str, allow: &Allowlist) -> Vec<Problem>;

// allowlist.rs
pub const MANIFEST: &str; pub const LOCK: &str;
pub struct Allowlist { /* (name, version) -> checksum; the direct crates */ }
impl Allowlist { pub fn embedded() -> Allowlist;
    pub fn direct(&self) -> Vec<(String, semver::Version)>;
    pub fn version_of(&self, name: &str) -> Option<&semver::Version>;
    pub fn contains(&self, name: &str, version: &str, checksum: &str) -> bool; }
pub fn vendor_dir() -> Result<PathBuf, Problem>;
pub fn verify_vendor(vendor: &Path, allow: &Allowlist) -> Result<(), Problem>;

// toolchain.rs
pub struct Toolchain { pub cargo: PathBuf, pub rustc: PathBuf, pub sysroot: PathBuf, pub version: String }
pub fn find(env: &Env) -> Result<Toolchain, Problem>;

// builder.rs
pub struct BuildCtx<'a> { pub env: &'a Env, pub toolchain: &'a Toolchain, pub vendor: &'a Path, pub allow: &'a Allowlist }
#[derive(Serialize)] pub struct Built { pub wasm: String, pub bytes: u64, pub lock_updated: bool, pub seconds: f64 }
pub fn build(dir: &Path, slug: &watane_pkg::Slug, ctx: &BuildCtx) -> Result<Built, Vec<Problem>>;
pub fn cargo(ctx: &BuildCtx, copy: &Path, args: &[&str]) -> std::process::Command;
pub fn outside_reads(dep_info: &str, allowed: &[&Path]) -> Vec<PathBuf>;

// unit.rs
#[derive(Serialize)] pub struct UnitReport { pub passed: u32, pub failed: u32, pub ignored: u32,
    pub not_run: u32, pub failures: Vec<UnitFailure> }
#[derive(Serialize)] pub struct UnitFailure { pub test: String, pub message: String,
    pub file: Option<String>, pub line: Option<u32> }
pub fn unit_tests(dir: &Path, slug: &watane_pkg::Slug, ctx: &BuildCtx) -> Result<UnitReport, Vec<Problem>>;
pub fn parse_libtest(stdout: &str, stderr: &str, slug: &str) -> UnitReport;

// contract.rs
pub struct ContractFile { pub fakes: watane_host::Fakes, pub steps: Vec<Step> }
pub struct Step { pub target: Target, pub args: serde_json::Value, pub expect: Expect,
    pub emits: Option<Vec<ExpectedEvent>> }
pub enum Target { Call(String), Deliver { interface: String, function: String } }
pub enum Expect { Success, Returns(serde_json::Value), Fails(Option<String>) }
pub struct ExpectedEvent { pub event: String, pub args: serde_json::Value }
pub fn parse(text: &str) -> Result<ContractFile, String>;   // by hand from serde_json::Value, for exact messages
#[derive(Serialize)] pub struct ContractReport { pub files: u32, pub passed: u32, pub failed: u32, pub failures: Vec<ContractFailure> }
#[derive(Serialize)] pub struct ContractFailure { pub file: String, pub step: u32, pub call: String,
    pub message: String, pub expected: Option<serde_json::Value>, pub got: Option<serde_json::Value> }
pub fn contract_tests(dir: &Path, slug: &watane_pkg::Slug) -> Result<(ContractReport, Vec<Problem>), Vec<Problem>>;
pub fn run_steps(file: &str, steps: &[Step], slug: &str,
    call: &mut dyn FnMut(&Target, serde_json::Value) -> Result<watane_host::Outcome, watane_host::CallError>)
    -> (u32, Vec<ContractFailure>);   // (passed, failures)

// json_eq.rs
pub fn json_eq(a: &serde_json::Value, b: &serde_json::Value) -> bool;

// deps.rs
#[derive(Serialize)] pub struct AddReport { pub slug: String, pub version: String, pub path: String,
    pub wit_dir: String, pub imports: Vec<String>, pub subscriptions: Vec<String> }
pub fn add_local(dir: &Path, provider: &Path) -> Result<AddReport, Vec<Problem>>;
pub fn local_order(dir: &Path) -> Result<Vec<PathBuf>, Problem>;   // providers first, `dir` last
pub fn read_local(dir: &Path) -> Result<BTreeMap<String, String>, Problem>;   // .watane-dev.json

// api.rs
pub struct Client { /* base URL, ureq::Agent */ }
impl Client {
    pub fn for_home(home: &Path) -> Result<Option<Client>, Problem>;   // None: no run.json or no answer
    pub fn status(&self) -> Result<serde_json::Value, Problem>;
    pub fn install(&self, folders: &[PathBuf], wait: Duration) -> Result<serde_json::Value, Problem>;
    pub fn publish(&self, package: &watane_pkg::Package, wait: Duration) -> Result<serde_json::Value, Problem>;
}

// preview.rs
pub fn find_watane() -> Result<PathBuf, Problem>;
pub fn serve(dir: &Path, env: &Env) -> Result<serde_json::Value, Vec<Problem>>;
pub fn stop(dir: &Path, env: &Env) -> Result<serde_json::Value, Vec<Problem>>;
```

## 13. Provides to other subsystems

- The program `watane-dev`: the commands, the envelope and the results of
  section 2, and the exit statuses. The journey tests and coding agents use
  only this.
- `watane-dev test <folder>`: `app` and `host` may use it to build their
  fixture mods (app `TECHNICAL.md` section 15).
- The template and the contract test form (sections 4 and 8), for makers.
- The file `.watane-dev.json` belongs to dev; no other subsystem reads it.
- No library API: no sibling may depend on the crate `watane-dev`.

## 14. Needs from other subsystems

If a sibling's final `TECHNICAL.md` gives an item under another name, use
it and change this list.

**pkg** (`watane_pkg`):
- `Slug: FromStr + Display`, with the grammar of pkg section 3.
- `Mod::read(dir) -> Result<Mod, ModError>`; `ModError::Invalid(Vec<Violation { rule, file, message }>)`, `ModError::Io`.
- On `Mod`: `slug()`, `version()`, `name()`, `world()` (`exports`,
  `events`, `calls`, `subscriptions` as `InterfaceName`), `page()`,
  `logic()`, `grants()`, `deps()`, `check_component()`.
- `impl Display for Grant`, `Deps::iter()` (slug, publisher or none).
- `add_provider(dir, &provider, None)`.
- `pack(dir, include_source) -> Result<Package, PackageError>`;
  `Package::{id, as_bytes, has_source, paths, from_bytes}`.
- The layout: `wit/deps/watane-host/` is the host package; `logic.wasm` at
  the folder root; `pack` reads only its listed paths.

**host** (`watane_host` and the program `watane`):
- `HOST_WIT: &str`.
- `default_home() -> Result<PathBuf, HostError>`;
  `RunInfo::read(home) -> Result<Option<RunInfo>, HostError>` with `pid`
  and `port`.
- `Ready` (Serialize, Deserialize): the ready line, with `api`, `mcp`,
  `token`.
- `Harness::open(built_mod, Fakes)`, `Harness::call(function, args)`,
  `Harness::deliver(interface, function, args)`, `Harness::functions()`;
  `Fakes`, `Outcome { value, events }`, `EventRecord { interface, function, args }`,
  `CallError { code, message }` with a `code` text, `FunctionInfo`.
- The program: `watane --preview <dir>` prints the ready line on stdout
  and then writes only to stderr; SIGTERM stops it and removes `run.json`.
  In a preview, the toolset `preview` holds every function of every
  enabled mod, also of mods installed later.
- Local API: `GET /api/status` (`mode`, `home`, `app`);
  `POST /api/install {"folders": [...]}` → `{"installed": [InstalledMod]}`,
  waiting for the participant in a home; errors
  `{"error": {"code", "message"}}`, code `refused` for a no.
- A folder with the slug of an installed local mod updates it, and the state
  stays (identity `local/<slug>`).

**app**: `POST /api/publish` with the package bytes
(`application/octet-stream`) on the home host's port →
`{"release": "<publisher key>/<slug>@<version>", "site", "link"}`
after the participant confirms; code `refused` for a no; the consent view
shows whether the package holds source. Steps 4 and 5: `/api/search`,
`/api/fetch`.

**log**, **dist**: nothing directly; through the home host only.

## 15. Test plan

Each test computes its expectation without calling the code under test:
from literal values in the test, from files it reads itself, or from an
independent tool. Integration tests run the binary
(`env!("CARGO_BIN_EXE_watane-dev")`) with `WATANE_DEV_CACHE` in a
`tempfile` folder, and parse standard output with `serde_json` in the test.

### 15.1 Unit tests (in the modules)

| Module | Checks | Oracle |
|---|---|---|
| outcome | exit status for each class; envelope JSON of a fixed Outcome | literal JSON text and numbers |
| template | `render` leaves no `{{`; `{{crate}}` of `a-b-c` is `a_b_c` | literal strings |
| check | `check_manifest` on 20 literal manifests (each refused key of 5.2, each allowed form) | a table of (manifest, expected codes) in the test |
| allowlist | parse the embedded lock; `verify_vendor` on a temp vendor with one wrong checksum, one missing and one extra folder | literal crate names; expected `env.allowlist` |
| builder | S2: cargo configuration quoting of a path with `"` and a space, environment whitelist; later: `outside_reads` on literal `.d` texts, also with `\ ` | literal paths and environment keys |
| unit | `parse_libtest` on captured outputs: all ok; one FAILED; aborted mid-run with a panic at `/watane-dev/build/x/logic/src/lib.rs:40:9` | literal counts, names, `logic/src/lib.rs` line 40 |
| contract | `parse` of every field form and every refused form; `run_steps` with a closure for returns, fails (with and without text), emits (short and full names, order), numbers `1` and `1.0` | literal expected failures |
| json_eq | key order, numbers, nesting | literal pairs |
| deps | `.watane-dev.json` text after two adds | literal text |

### 15.2 Integration tests (`tests/`)

| File | Scenarios | Oracle |
|---|---|---|
| `envelope.rs` | DEV-S-1, 2, 3, 32 | the template file list as a literal; the envelope keys as a literal set |
| `check.rs` | DEV-S-5, 6, 7, 8 | the codes of the table of 2.2 |
| `builder.rs` | DEV-S-4, 9, 10, 11, 12, 13 | S-4: the test counts `#[test]` in `logic/src/lib.rs` with a regex and the steps in `tests/*.json` with `serde_json`, and loads `logic.wasm` with `wasmtime::component::Component` to find `mods:<slug>/api@0.1.0` among the exports; S-9: a fake toolchain folder whose `bin/cargo` writes a marker file if it runs; S-10: a secret file of random bytes in a temp folder, then a byte search over every file of the mod folder; S-12: the same template at two paths with two caches, `assert_eq!` of the bytes, and a search for `$HOME`, both caches and both mod paths (raw and canonical); S-13: run under `sandbox-exec -p '(version 1)(allow default)(deny network*)'` on macOS or `unshare -rn` on Linux, ignored with a reason where neither works |
| `unit_tests.rs` | DEV-S-14, 15, 16, 20; S4-2 result and template | source fixtures written into temporary mods assert inside Wasm that `read_dir("/")` fails and `var("HOME")` is unset, and try to write `<tempdir>/escape`; the test checks the file is absent; S-16 with `WATANE_DEV_UNIT_SECONDS=2` and a wall-clock bound after warming the build cache; memory growth and host-import traps are also exercised |
| `contract.rs` | DEV-S-17, 18, 19, 20 | literal expected and got values; fixture `tests/fixtures/recommendations/` (pkg section 9) imports `mods:reading-list/books` |
| `serve.rs` | DEV-S-21, 22, 23, 24 | S-21: GET of the page URL (with `Host: <local>.localhost:<port>`), compared with the bytes the test reads from `ui/index.html`; MCP `tools/list` and `tools/call` as JSON-RPC with `Authorization: Bearer <token>`, tool names compared with `<local>_api_add` and `<local>_api_get` built by the test from the slug; S-22: `tools/call` of `api.add` before the change, `api.get` returns 1 after; S-23: a TCP connect to the port fails, the folder is gone |
| `install_publish.rs` | DEV-S-25 to 31 | S-25: `deps.json` and the files of `wit/deps/a/` read by the test; the fake host (15.3) stores each request; S-29: the stored folders as literal paths; S-31: the stored body decoded with `watane_pkg::Package::from_bytes` (not code under test) and its `paths()` searched for `logic/src/` |
| `journey_j1.rs` | J1 on the real programs | `new` → break a contract → `test` fails with `test.contract` → restore → `test` ok → `serve` → page and MCP read → change `ui/` → `serve` → new bytes, same count |

S2 `builder.rs` drives `serve` for S-9 and S-11, and checks the built
component with `watane_pkg::Mod::check_component()`. S-4, S-10 and S-12
join with their later slices. The network-denied build test may be ignored
with the OS refusal reason when nested sandboxing is unavailable; it runs
on a validator with permission to create the sandbox.

Tests that need the real `watane` (S-21, 22, 23, J1) are
`#[ignore = "needs WATANE_BIN"]` and read `WATANE_BIN`. With the feature
`fake-host`, S-23, 24 and S-25 to 31 run against `fake-watane`.

### 15.3 The fake host (`src/bin/fake-watane.rs`, feature `fake-host`)

Arguments `--preview <dir>` or `--home <dir>`. It listens on
`127.0.0.1:0` with `tiny_http`, writes `<dir>/run.json`
(`{"watane":1,"pid","port","mode"}`), prints a ready line in the form of
`watane_host::Ready` (token `wt_fake` in a preview, `null` in a home), and
answers `GET /api/status` (`app: true` in a home), `POST /api/install` and
`POST /api/publish`. It stores each request body as
`<dir>/requests/<n>-<path>`. `<dir>/fake.json` (`{"answer": "approve" |
"decline"}`, default approve) sets the answer; decline answers 403
`{"error":{"code":"refused","message":"declined"}}`. An installed folder
gets local name = folder name and a page URL if it has `ui/`. It serves no
page and no MCP. SIGTERM ends it (the default action).

## 16. Build plan

- S4-2 (dev): the counter template includes `tests/counter.json`; check
  stage 6 and Wasmtime unit tests are implemented. `test` runs check, build
  and unit tests; its contract part stays null until S4-3. `serve` runs
  check and build only until S4-3. DEV-S-10 outside reads and DEV-S-12
  reproducible bytes remain deferred; `env_clear()` and remap flags remain.

| Stage | Build | Needs | Fake until it exists | Done when |
|---|---|---|---|---|
| 0 | crate, `cli`, `outcome`, `env`, text and JSON output, exit statuses | — | — | S-32 for `check` outside a mod: `usage.not_a_mod`, status 2 |
| 1 | `template`, `new`, `check` (5.1 to 5.3) | pkg stages 2 and 3: `Slug`, `Mod::read`, `Violation`, `Grant`, `World` | without pkg: a local slug regex and dev's own rules; the `mod.invalid` path is a test `#[ignore = "needs watane-pkg"]` | S-1, 2, 3, 5, 6, 7, 8; the template passes `Mod::read`. S1 ships the page-only cut: `.gitignore`, `AGENTS.md`, `CLAUDE.md`, `mod.toml`, `wit/world.wit`, `ui/index.html`, `ui/app.js`; no logic, host WIT or tests; `new.next` is `Edit ui/, then run: watane-dev serve --json` |
| 2 | `allowlist`, `scripts/vendor.sh`, `toolchain`, `builder` | host: `HOST_WIT`; pkg stage 4: `check_component` | `HOST_WIT` as a local const copied from `spec/TECHNICAL.md` section 4, marked for removal; `check_component` behind one fn, with an ignored test | S-9 to 13; the template builds |
| 3 | `unit`, `test` pipeline, check stage 6, template contract file | — | — | S4-2: S-14 to 16, S-20 check; S-4 with `contract: null` |
| 4 | `contract`, `json_eq` | host stage 5: `Harness` | `run_steps` tested with closures; integration tests ignored | S-17 to 20; S-4 complete |
| 5 | `preview`, `api`, `doctor`, `fake-watane` | host stage 6: the program, `Ready`, `RunInfo`, `/api/status`, `/api/install` | `fake-watane` | S-21 to 24 on the real `watane`; J1 passes |
| 6 (step 2) | `deps`, `add`, `install` | pkg stage 6: `add_provider`; app running for J2 | `fake-watane` | S-25 to 29 |
| 7 (step 3) | `publish` | pkg stage 5: `pack`; app: `/api/publish` | `fake-watane` | S-30, 31; dev's part of J3 |

Steps 4 and 5 are not designed (section 10).

## 17. Questions for spec/

Each has a proposed answer, which this specification uses.

1. **"Run the tests of the logic part" would run mod code natively.** That
   breaks the Builder boundary (spec §6). Proposal: unit tests run as Wasm
   in Wasmtime with no capabilities; add this to the §6 Builder row.
2. **"No build script"** cannot hold for allowlisted crates (`wit-bindgen`
   has a `build.rs`; its macro is a procedural macro). Proposal: no build
   script or procedural macro in the mod; allowlisted crates are reviewed
   and pinned.
3. **The allowlist** (spec §9, step 1): a vendored directory source that
   ships with `watane-dev`, pinned by a compiled-in `Cargo.lock`; the first
   set is `wit-bindgen`, `serde`, `serde_json`.
4. **The form of a contract test file** (spec §9, step 1): JSON, 8.1.
5. **A local provider has no location.** pkg's `deps.json` maps a slug to a
   publisher or `null`; `serve` and `install` must still build the local
   provider. Proposal: dev keeps the folder in its own `.watane-dev.json`,
   outside every package.
6. **The preview token is only in the ready line**, but a second `serve`
   must reuse the running preview. Proposal: dev keeps the ready line in
   its cache with mode 0600. The preview holds no participant data (W5), so
   this is within spec §6 "not protected: another program of the same user".
7. **F-PUB-4 "where the maker agrees"**: proposal: `publish` includes the
   source unless `--no-source`; the consent view shows it, and the
   confirmation is the agreement.
8. **Where journey tests live** (spec §9): J1 in `dev/tests/`, driving the
   real programs (app proposes the same).
9. **Wasmtime in dev**: spec §2 lists `dev` → `pkg`, `host` only. dev uses
   the `wasmtime` crate directly for unit tests. Proposal: a library, not a
   subsystem, so allowed; the same version as `host`.
10. **One fact in one place for mod authors**: the `watane.js` API and the
    value encoding belong to `host`, but the agent reads them in the
    template's `AGENTS.md`. Proposal: `host` exports a `GUIDE` text that the
    template embeds; until then the template points to host's table and a
    J1 test catches drift.
11. **Contract fakes are constant** (host `Fakes`): a fake cannot fail or
    answer by argument. Proposal: enough for step 1; revisit when a maker
    needs it.
