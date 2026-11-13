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
  src/doctor.rs         report every environment gap without stopping early
  src/env.rs            the cache folder and the limits
  src/template.rs       the embedded template; `new`
  src/check.rs          pkg rules plus the builder constraints
  src/allowlist.rs      the embedded allowlist; find and verify the vendor folder
  src/toolchain.rs      find cargo, rustc and the sysroot; check the target
  src/builder.rs        copy, lock, cargo build, dep-info check, logic.wasm
  src/unit.rs           build the unit tests; run them in Wasmtime
  src/contract.rs       the contract test file; run it on the host Harness
  src/json_eq.rs        equality of two JSON values for tests
  src/deps.rs           `add <mod folder>`; .watane-dev.json; local and released providers in order
  src/build_on.rs       step 5: `new --from`, `add --release`, released providers in the cache (10.1)
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
default = ["fake-host"]
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

Distribution builds use `cargo build --release --no-default-features` to omit
the fake host. Default builds include it so integration tests need no feature flag.

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
  search [<text>...] [--publisher <key>] [--interface <name>] [--home <path>]   (step 4)
  new [<slug>] --from <link> [--home <path>]                (step 5)
  add --release <link or release> [--home <path>]           (step 5)
  publish --push [...]                                      (step 6)
```

- `--help` describes every command, positional argument and option. `serve --stop`
  stops the preview and deletes its data; its help states this before use.
- `--json` and `--dir` are global (`clap` `global = true`). `--dir` is the
  mod folder, default the current folder. dev canonicalizes it.
- `--wait` defaults to 600 seconds: the time that the participant has to
  answer in the app.
- With `--json`, standard output holds exactly one JSON object and a
  newline. Standard error may hold progress lines, for people only. Before
  release compilation, write one `building <slug>` line to stderr.
  Unit compilation adds no line; cold and warm builds use the same text
  without a timing hint. stdout remains the single result object.
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
| `command` | string | `new`, `check`, `test`, `serve`, `doctor`, `add`, `install`, `publish`, `search`. `new --from` is `new`; `add --release` is `add`. |
| `ok` | bool | `errors` is empty. |
| `dir` | string or null | The mod folder; null when the command has none (`doctor`, `search`). |
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
| Refused | 4 | The host or the participant said no, failed, or did not answer. |
| Internal | 5 | A fault of dev, or an I/O error. |

| Code | Class | When | Message (template) | Fix |
|---|---|---|---|---|
| `usage.args` | 2 | clap rejects the arguments | the clap text | `watane-dev --help` |
| `usage.not_a_mod` | 2 | the folder has no `mod.toml` | `<dir> is not a mod folder: it has no mod.toml` | `Run the command in a mod folder, or pass --dir <folder>. Create a mod with: watane-dev new <slug>` |
| `usage.slug` | 2 | `watane_pkg::Slug::from_str` refuses it | `"<s>" is not a valid slug: <pkg reason>` | `Use lowercase words of letters and digits joined by "-", each starting with a letter, at most 40 bytes, not a WIT keyword. Example: reading-list` |
| `usage.exists` | 2 | `new` into an existing path | `<path> exists` | `Choose another slug, or pass --dir <new folder>` |
| `usage.provider` | 2 | `add`: the path is not a mod folder | `<path> is not a mod folder`, plus ` (<canonical path> has no mod.toml)` when the folder exists under another name | for a relative `<path>`: `A relative provider folder is read from the current folder, not from --dir. Run add in the provider's folder or pass its absolute path`; else — |
| `env.toolchain` | 3 | no rustup, no cargo, no rustc | `no Rust toolchain found: <detail>` | `Install rustup with your package manager, then run: rustup default stable` |
| `env.no_wasm_target` | 3 | the sysroot has no `wasm32-wasip2` | `the Rust toolchain at <sysroot> has no wasm32-wasip2 target` | `rustup target add wasm32-wasip2` |
| `env.allowlist` | 3 | the vendor folder is missing or differs from the allowlist | `the crate allowlist at <path> is missing` / `... differs from the allowlist of this watane-dev: <detail>` | `'<abs dev>/scripts/vendor.sh' '<path>'` (6.2; no export needed); a resolved logic crate outside the allowlist: `Reinstall watane-dev, or set WATANE_DEV_VENDOR` |
| `env.watane_missing` | 3 | the program `watane` is not found | `the program watane was not found (<where>)` | `Install watane, or set WATANE_BIN to the program` |
| `env.preview_start` | 3 | the preview host printed an error, exited, or gave no ready line in 30 s | `the preview did not start: <host error or last 20 log lines>` | `Read <log path>` |
| `env.host_not_running` | 3 | no `run.json`, no answer, a preview in that home, or a home host with no app | `no watane app runs for the home <home>` (or the specific cause) | `Start the watane app, then run this command again` |
| `mod.invalid` | 1 | each `pkg` `Violation` | `[<rule>] <message>`, with the violation's `file`; preserve wit-parser `--> <file>:<line>:<col>` locations when present, using the more specific WIT file relative to the mod | — |
| `mod.cargo_key` | 1 | a key of `logic/Cargo.toml` outside 5.2 | ``logic/Cargo.toml: `<key>` is not allowed`` | `Remove it. Allowed: <list>` |
| `mod.dependency` | 1 | a dependency outside the allowlist, or of a refused form | ``dependency `<name>` is not in the allowlist`` / `... must be a version requirement` / `... requires <req>, the allowlist has <version>` | `Use only: <direct crates with versions>` |
| `mod.logic_file` | 1 | a file of `logic/` outside 5.3 | `logic/build.rs is not allowed: the builder runs no build script` (one text for each kind) | `Remove <file>` |
| `mod.symlink` | 1 | a symbolic link in the mod folder | `<file> is a symbolic link` | `Replace it with a file` |
| `mod.no_contract_tests` | 1 | a logic part and no `tests/*.json` | `the mod has a logic part and no contract test` | `Add tests/<name>.json (see AGENTS.md)` |
| `mod.self_dependency` | 1 | `add` of the mod itself | `a mod cannot depend on itself` | `Pass the folder of another mod to add` |
| `mod.dependency_cycle` | 1 | the local providers make a cycle | `<a> -> <b> -> <a> is a cycle` | From `add`: `Do not add <provider slug>: it already depends on this mod`. From traversal: `Remove mods:<slug> from deps.json and wit/deps/<slug>/ in <absolute folder>` for the edge that closes the cycle. |
| `mod.provider_missing` | 1 | a local provider has no folder in `.watane-dev.json`, or the folder is gone | `the local provider <slug> has no known folder` | `Run: watane-dev add <folder of <slug>>` |
| `build.resolve` | 1 | cargo cannot resolve with the allowlist | cargo's text | `Use only the crates and features of the allowlist: <list>` |
| `build.compile` | 1 | each rustc error | the rustc message, with its location | — |
| `build.reads_outside` | 1 | the compiler read a file outside the folders of 6.4 step 6 | `the build read <path>, outside the mod` | `Remove the include_bytes!, include_str!, include! or #[path] that names it` |
| `build.imports` | 1 | each `Violation` of `Mod::check_component` | `[component] <message>` | — |
| `test.unit` | 1 | each failed unit test | the panic message | — |
| `test.unit_limit` | 1 | a unit test binary passed the time or memory limit | `unit tests stopped at <test> after <n> s` / `... used more than 512 MiB` | — |
| `test.contract_file` | 1 | a test file does not parse or has a wrong field | serde's text, or the field | `See the contract test form in AGENTS.md` |
| `test.contract` | 1 | each failed contract step | `<file> step <n> (<call>): expected <x>, got <y>` | — |
| `publish.version_exists` | 1 | the app already published this slug and WIT package version, or POST races with another publish (409 `version_exists`) | `<slug> <version> is already published as <release>; its link is <link>`; if null: `...; it has no link yet: set your site's URL in the watane app`; for POST: the app's message unchanged | `This version is already published. To publish changes, raise the version in wit/world.wit, then run publish again` (file `wit/world.wit`) |
| `publish.local_dependency` | 1 | a provider in `deps.json` is local (`null`) | `mods:<slug> is a local provider; it has no release` | `Publish <slug> first (if it is already published, that run reports its release and link), then run: watane-dev add --release <that link>` |
| `usage.no_source` | 2 | `new --from` of a release whose package holds no source (step 5) | `the release <ReleaseRef> holds no source: its publisher published it with --no-source` | `Build on a release that includes its source, or depend on this one with: watane-dev add --release <link>` |
| `mod.release_provider_missing` | 1 | a provider in `deps.json` with a publisher has no package in the cache (step 5, 10.1) | `the provider <ReleaseRef> is not in the cache of watane-dev` | `Start the watane app, then run: watane-dev add --release <ReleaseRef>` |
| `host.declined` | 4 | the host answers with error code `refused` | install: `The person declined installing <slug> in their watane app`; publish: `The person declined publishing <slug> from their watane app` (or the mod folder) | install: `Ask the person whether they want <slug> installed in their watane app, then run install again with --wait <seconds>`; publish: `Ask the person whether they want <slug> published, then run publish again with --wait <seconds>` |
| `host.refused` | 4 | 4xx or other non-5xx error answers of the host or app; fetch 502 `fetch_failed` is also a refusal | `[<code>] <message>` | — |
| `host.failed` | 4 | app 5xx, except 503 `unavailable` and fetch 502 `fetch_failed` | `[<code>] <message>` | `The watane app failed on this request; ask the person to read the watane app's log, then run this command again` |
| `host.no_answer` | 4 | `--wait` passed | `no answer from the home host in <n> s` | `Run the command again with a longer --wait`. If the person approved meanwhile, publish reports the release and its link on retry |
| `internal` | 5 | anything else | the error chain | — |

App 503 `unavailable` is `env.host_not_running` with message
`the watane app of <home> runs but could not open its data: <app message>`
and fix `Ask the person to restart the watane app, then run this command again`.
Other app 5xx use `host.failed` as above; fetch 502 `fetch_failed` follows 10.1.
4xx keeps its existing refusal code and fix.

Warning codes (step 5 first): `publish.provider_unlisted` (the app's
publish answer has `listed: false` for a provider; message `<key>/<slug> is
not listed in your channel, so people who install <slug> need a channel or
an index that names where it is published`, fix `Ask the person to list
<key>/<slug> in their own index in the watane app`), `ui.external` (a file of `ui/` names an `http://` or
`https://` URL in `src`, `href`, `import` or `fetch(`; the content policy of
the host blocks it), `ui.inline_script` (`ui/*.html` holds a `<script>`
without `src`; the content policy blocks it), `test.uncovered` (an
exported function is called by no contract step), `preview.previous` (a
`serve` refused by the test pipeline while a preview for that folder still
answers; message `the preview at <page url> still serves the previous build`
(the API URL only when the served mod has no page),
fix `Fix the errors, then run: watane-dev serve --json`). Warnings never
change `ok` or the exit status.

For a violation in managed `deps.json`, `wit/deps/` or `.watane-dev.json`,
the fix names an author action rather than telling the agent to edit that
file. For an unused provider the message is `deps.json lists mods:<slug> but
wit/world.wit imports none of its interfaces`; for an own provider it is
`deps.json lists this mod itself as mods:<slug>`. For an unused provider:
import its interface in `wit/world.wit`, or
ask before deleting `wit/deps/<slug>/` and `mods:<slug>` from `deps.json`.
For an own provider the fix is `Ask before deleting wit/deps/<slug>/ and
mods:<slug> from deps.json: a mod cannot depend on itself`. dev passes both
messages through from pkg. If pkg instead reports the own slug as unused
(including a deps.json-only entry with no copied WIT), dev compares it with
the mod's own WIT declaration and uses the self-dependency fix, never the
import advice. For `wit/deps/<folder> holds this mod's own package mods:<slug>`,
use `Ask before deleting wit/deps/<folder>/: a mod cannot depend on itself`:
the folder comes from the violation's file, even when it differs from the slug.
There is no dependency-removal command today. Other managed dependency
violations point to `watane-dev add <provider folder>` to refresh the files.

### 2.3 Text output

For each error: `error[<code>]: <message>`, then `  --> <file>:<line>:<column>`
and `  fix: <fix>` when known. The same for warnings with `warning[...]`.
Then one line `<command>: ok` or `<command>: failed`, the main fields of
`result` as `key: value` lines, and `next: <next>`.
Nested objects emit one line per field with dotted keys (for example
`allowlist.vendor: /abs/vendor`, `toolchain.wasm32_wasip2: true`). Lists
of objects emit indexed dotted keys, one field per line (for example,
`pages.0.slug: x`, `pages.0.url: http://x.localhost/`). Lists of plain values
join with `, `; an empty list prints `none` (for example,
`calls: none`). Strings are unquoted, null is `missing`, and
booleans and numbers use their plain text. No JSON containers or quotes
are added to these values.
Argument errors use the first known subcommand name found in the arguments
for `<command>`, or `watane-dev` if none is found; JSON uses the same name.
They end with `<command>: failed`, never a blank command name.

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
are the fields of `watane_pkg::World`. `path` is the value recorded for
the slug in `.watane-dev.json`, or null when none is recorded.
When the first error is mod-class (including `mod.invalid`), `next` is
`Fix the errors, then run: watane-dev check --json`. Env-class failures
retain `watane-dev doctor --json`.

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
S4 runs unit tests and then contracts, retaining unit failures while running
contracts so both failures reach the agent.
A part that did not run is null. `build`, `unit` and `contract` are null
for a mod with no logic part. `not_run` counts the tests after a test that
aborted the run (7.2). In a failure, `expected` and `got` are JSON values
or null. `next` on success: `watane-dev serve --json`. On failure,
the first error determines `next`: `test.unit` or `test.unit_limit` uses
`Fix the unit test <test name>, then run: watane-dev test --json`;
`test.contract` step failures use `Fix <file> step <n> (<call>), then run:
watane-dev test --json`. Otherwise use its fix when known, or
`watane-dev test --json`. Environment discovery failures retain
`watane-dev doctor --json`; doctor itself uses the fix instruction below.
`check` currently performs no environment discovery.
Contract step failures report the 1-based line where the step object starts;
`step` remains its 1-based index. Host-open failures have no step location.
Pkg currently omits parser locations for malformed WIT worlds; those errors
retain null line and column. When its message includes a parser location,
dev preserves the line, column and the specific WIT file.

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
as `Authorization: Bearer <token>`. On success, `next` is
`Give the person this page: <url>`, using the served mod's own (last)
`pages` entry, with no punctuation after the URL. With `reused: false`,
prepend: `The MCP url and token are new.
Set or update your agent's MCP config with result.mcp.url and result.mcp.token
(see AGENTS.md, "MCP tools for your agent").` In text mode the keys are
`mcp.url` and `mcp.token`; JSON mode keeps `result.mcp.*`. This MCP instruction
is also
returned when the mod has no page. With `reused: true`, the page instruction
is unchanged, or null when the mod has no page. On failure with an
`env.*` first error, `next` is `watane-dev doctor --json`. A consumer's
unit/limit or contract step failure uses the test recovery text above with
`watane-dev serve --json`; otherwise its first fix, or the plain serve
rerun. A local provider failure retains `watane-dev test --json
--dir <absolute provider folder>` (single-quoted for the shell when it has
other characters than letters, digits, `/._-`). **serve --stop**:
`{"stopped": true, "home": "..."}`; `stopped` is false when none ran, and
that is not an error. When `stopped` is true, `next` is
`The preview and its data are gone. To start a new one, run: watane-dev serve --json`.

**doctor**
```json
{"toolchain": {"cargo": "/abs", "rustc": "/abs", "sysroot": "/abs", "version": "rustc 1.97.1 ...", "wasm32_wasip2": true},
 "allowlist": {"vendor": "/abs", "direct": ["serde 1.0.229", "serde_json 1.0.154", "wit-bindgen 0.63.0"], "crates": 57},
 "watane": "/abs/watane",
 "home": {"path": "/abs/.watane", "running": false, "app": false}}
```
A missing part is null and adds its error. A discovered toolchain without
its Wasm target remains an object with `wasm32_wasip2: false` and adds
`env.no_wasm_target`; discovery still reads its version and never runs cargo.
`doctor` reports every gap, not only the first. A home host that does not
run is no error. `running` requires a readable `run.json` and a status
answer with `watane: 1` within 2 seconds; `app` is that answer's boolean.
On success, `next` is `watane-dev serve --json` when the current folder
(or `--dir`) contains `mod.toml`, otherwise `watane-dev new <slug> --json`.
On a gap, it is `Run each error's fix, then run: watane-dev doctor --json`.

**new --from** (step 5)
```json
{"slug": "ratings", "dir": "/abs/ratings", "files": ["AGENTS.md", "..."],
 "origin": "<Alice key>/reading-list@0.1.0", "from_slug": "reading-list",
 "providers": ["<key>/tags@0.2.0"],
 "mentions": [{"file": "logic/src/lib.rs", "line": 3}]}
```
`files` as for `new`. `providers` are the released providers fetched into
the cache (10.1). `mentions` lists each line (at most 200, in path and line
order) of `wit/*.wit`, `logic/` and `ui/` and `tests/` that still names the
old slug: `<from_slug>` or its `_` form as a whole word (bounded by no
`[a-z0-9_-]`). It is empty when the slug is unchanged. It gives only file
and line, never the text: the source is the publisher's, and the agent
reads it in the file as data (section 11). `next`: with
mentions, `Replace <from_slug> with <slug> (and <from_snake> with
<snake>) at each result.mentions entry, then run: watane-dev test --json`;
without, `Edit wit/world.wit, logic/src/lib.rs and ui/, then run:
watane-dev test --json`.

**add --release** (step 5)
```json
{"slug": "ratings", "version": "0.1.0", "path": null, "wit_dir": "wit/deps/ratings",
 "publisher": "<Carol key>", "release": "<Carol key>/ratings@0.1.0",
 "link": "watane:<Carol key>@https://carol.example.org/mods/#ratings",
 "providers": ["<Carol key>/ratings@0.1.0"],
 "imports": ["import mods:ratings/api@0.1.0;"], "subscriptions": []}
```
`link` is the given link, or null when a ReleaseRef was given. `providers`
are all released packages fetched into the cache, this one first. `next`:
the `add` text, plus ` Contract tests use a fake of it (see AGENTS.md).`

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
From the host's `InstalledMod` list, in install order, retaining exactly these five fields.
`next`: `Give the person this page: <url>` for the last entry with a page,
else null. On `env.host_not_running`, `next` is
`watane-dev install --json --dir <canonical mod folder> --home <resolved home>`,
including `--dir` when it was omitted from the original command, adding
` --wait <n>` only when `--wait` was explicitly passed. Quote each path for the
shell when it contains characters other than letters, digits or `/._-`.
Run it once the person has started the app. Other `env.*` errors use
`watane-dev doctor --json`.

A declined install returns `host.declined` with the consumer slug (or mod
folder), says the person declined installing it in their watane app, and
asks whether they want it installed before running install again with
`--wait <seconds>` (9.6). Its `next` is the first error's fix. Every refusal
has a `next`; when no more specific follow-up applies, use the first error's
fix, or `watane-dev test --json` if no fix is known.

**publish** (step 3)
```json
{"home": "/abs/.watane",
 "release": "<publisher key>/reading-list@0.1.0",
 "package": "<64 hex>", "source": true, "site": "/abs/site folder", "link": "watane:...#reading-list",
 "providers": [{"provider": "<key>/tags", "listed": true}]}
```
`providers` (step 5) is the app's answer as it is (app 8.1), `[]` when the
answer has none; each `listed: false` adds `publish.provider_unlisted`.
`home` is the resolved home; `package` is the posted package id and `source`
is `Package::has_source()` (false also when the mod has no source files).
`release`, `site` and `link` come from the app's answer; `link` is null
while no site URL is set. With a link, `next` is
`Put the site folder on your static web host`; without one, it is
`Set your site's URL in the watane app, then run publish again to get the link`.
That rerun checks existing releases and reports the release and current link.
It keeps `publish.version_exists` and exit 1, with `result` containing only
`home`, `release` and `link` from the listed release (home is resolved).
In publish text output only, a null `link` prints `link: none yet`; JSON
keeps null. The general null-to-`missing` rule of 2.3 is unchanged.

Every publish refusal has a `next`:

- `publish.version_exists`: the version-bump fix of 2.2.
- `publish.local_dependency`: `watane-dev publish --json --dir <absolute provider folder>`
  for the first local provider whose folder `.watane-dev.json` knows; quote the
  paths for the shell as above. If `--home <h>` was passed, append
  `--home <absolute h>` to keep the same home, without contacting it.
  If none is known, use the first error's fix.
- `env.host_not_running`: repeat publish with `--json`, canonical `--dir`,
  home and explicit wait, as in 9.7 step 2; other environment gaps use doctor.
- Pipeline failures retain their test `next`; other failures use the first
  error's fix, or `watane-dev test --json` when no fix is known.

**search** (step 4)
```json
{"home": "/abs/.watane", "indexes": 1,
 "results": [{"release": "<key>/reading-list@0.1.0", "name": "Reading list",
              "summary": "...", "publisher": {"key": "<key>", "name": "Alice", "fingerprint": "..."},
              "link": "watane:<key>@https://alice.example.org/mods/#reading-list",
              "status": "ok", "checked_at": 1762783600000, "unchecked_since": null,
              "via": [{"kind": "index", "index": "<Ivy key>", "through": null, "note": "good for books"}],
              "offers": ["mods:reading-list/books@0.1.0"], "events": [], "uses": []}]}
```
`home` is absolute. `indexes` and `results` are the app's answer as it is
(app 8.2), in its order; dev keeps results as untyped JSON, including unknown
fields. Text output flattens them as in 2.3. `next`:
with results, null; with none and `indexes` 0, `Ask the person to add an
index in the watane app, then run search again`; with none otherwise,
`Try fewer words, or search with --interface <name>`.

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
  vendor/                optional allowlist bootstrap, discovered without an override
  cargo-home/            CARGO_HOME of every cargo run
  tmp/                   TMPDIR of every cargo run
  build/<slug>/          the copy of the mod sources for one build
  build/<slug>.lock      a file lock for the duration of a build of that slug
  target/                the shared cargo target folder of all mods
  packages/<id>          step 5: verified released packages; <id> = PackageId::of(bytes), computed by dev
  providers/<id>/        step 5: Package::unpack of packages/<id>, for previews
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
  },
  "release_providers": {
    "tags": {"release": "<key>/tags@0.2.0", "package": "<64 hex>", "link": null}
  }
}
```

Keys are slugs; `local_providers` values are paths relative to the mod
folder, with `/`. `release_providers` (step 5, default `{}`) names the
released providers of this mod and of its providers, with the package id
of each in the cache (10.1), and the link that `add --release` was given
(null for a ReleaseRef or a provider of a provider); `add --release` and
`new --from` write it.
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
with these sections in this order (section 10 joins with its slice):
1. **This folder**: a table of the files above and who edits each.
   `wit/deps/`, `deps.json` and `.watane-dev.json` are written by
   `watane-dev`; do not edit them.
2. **Order of work**: first `watane-dev doctor --json`; run each error's
   `fix` before going on; for `env.allowlist`, run it as given: dev finds
   the filled folder without an exported variable, including in a sandbox
   without write access to the watane checkout. Edit `wit/world.wit`, then `logic/src/lib.rs`, then
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
   `watane.on(handler)` opens the event stream and gets `{"type":"changed"}`
   on stream open and after a call that changed data, plus the mod's own
   events as `{"type":"event", ...}`; show the one-line `ui/app.js` pattern
   that re-reads on `changed`. The JSON form of WIT values (the table of host
   `TECHNICAL.md` 6.9: records as objects, `option` as null or the value,
   variants as `"case"` or `{"case": payload}`, `result` as `{"ok": v}` or
   `{"err": e}`).
7. **Try the page without a browser**: use `result.pages[i].url` from
   `serve --json` for `POST /call` with its exact `Origin`, JSON content
   type and `fn`/`args`; responses are `value` or `error`. A headless
   browser may hang on open `/events`; curl checks calls, the person checks looks.
8. **MCP tools for your agent**: `result.mcp.url` and `result.mcp.token`,
   the bearer header and config updates after a new preview starts;
   Claude Code HTTP setup and headless allowed tools; Codex's server table
   with `url`, `http_headers` and person-set `default_tools_approval_mode = "approve"`;
   other harnesses use Streamable HTTP and the person allows tools.
9. **Tests**: unit tests run as WebAssembly with no files, no environment
   and no host imports: test pure functions there. A panic ends the whole
   unit test run, so the tests after it do not run. Contract tests: the
   form of 8.1 in short, with an `emits` example, failure matching and
   reports. S4 runs contracts; fake linking and `deliver` examples arrive
   with links between mods (step 2).
10. **Building on another mod**: `watane-dev add <mod folder> --json`, then
   copy the needed lines of `result.imports` or `result.subscriptions`
   into the world.
11. **Results**: the envelope fields, the exit statuses, and the advice
   "read `errors[].fix`; for `build.compile` read `file` and `line`".
12. **Rules**: do not ask questions; finish with a passing
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
5. `<rustc> --version` gives the version for `doctor`, including when
   step 4 found no Wasm target. Build and test still stop before cargo.

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
- `scripts/vendor.sh [<out>]` writes the vendor directory to `<out>`, relative
  to the caller's folder when relative, or to `dev/allowlist/vendor` with
  no argument. It copies the allowlist manifest, lock and empty library to
  a temporary workspace, writes nothing else in the checkout, and tries
  `cargo vendor --offline --locked --versioned-dirs` there first, falling
  back to normal registry access only if needed. The packager installs it
  as `<prefix>/share/watane-dev/vendor`.
- dev selects `WATANE_DEV_VENDOR` when set (even if absent); otherwise the
  first existing folder of `<folder of executable>/../share/watane-dev/vendor`,
  `<CARGO_MANIFEST_DIR at compile time>/allowlist/vendor`, `<cache>/vendor`.
  With none present, the error names `<cache>/vendor`.
- The fix is always `'<abs dev>/scripts/vendor.sh' '<path>'`. For a missing
  override it targets that override; without an override it targets the
  absolute cache vendor path. No export is needed: the override is already
  set, or dev discovers the cache vendor automatically. `doctor`, `test`
  and `serve` share this fix through `allowlist::vendor_dir`.
- **Verification**, in doctor and before each build: the vendor folder holds exactly one
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

Before release compilation, dev writes one progress line to stderr:
`building <slug>`. Unit compilation adds no line, so the release-plus-unit
pipeline prints once per mod. The rule is identical for cold and warm
builds and makes no first-build timing claim. Both builds use
`builder::compile`; `ac1_release_and_unit_build_progress_stays_on_stderr`
proves the single line and unchanged stdout without compiling a mod.

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
4. Before compilation, dev deletes
   `<cache>/target/wasm32-wasip2/release/deps/<crate>-*.d` and `<crate>.d`
   (`<crate>`: the package name with `-` replaced by `_`). Dev refreshes
   the copied `logic/src/lib.rs` modification time so Cargo recompiles the
   mod even when its binary dependency cache still considers it fresh.
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
6. **Reads** (DEV-S-10): dev reads the new `deps/<crate>-*.d` and `deps/<crate>.d` files (make syntax:
   `target: dep dep ...`, with `\ ` for a space in a path). Every path in
   them must lie under the copy folder, the vendor folder, the sysroot or
   `<cache>/target`. Else one `build.reads_outside` for each path, and dev
   writes no `logic.wasm` (DEV-F-14). rustc lists the files of
   `include_bytes!`, `include_str!`, `include!` and `#[path]` there.
   Relative dependencies resolve from the copy's `logic/` working folder;
   canonicalization resolves `..` and symlinks before checking roots. A
   successful compilation without a mod dependency file is an internal error.
7. dev copies the `.wasm` to `<mod>/logic.wasm`, then runs
   `watane_pkg::Mod::read(dir)?.check_component()`. Each violation is
   `build.imports`, and dev deletes `<mod>/logic.wasm` again.

### 6.5 Reproducible output (DEV-S-12)

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
   `profile.test` true. Before compilation dev deletes the old
   `debug/deps/<crate>-*.d` and `<crate>.d` files; after compilation the
   reads check of 6.4 step 6 scans the new files before any binary executes.
   An outside read also removes `<mod>/logic.wasm`.
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

Unit failure fixes name the actual failed test, including a limit:
`Fix the unit test <test name>, then run: watane-dev test --json`.
The test/serve pipeline uses this action for its own consumer's `next`,
with the command being run (2.4); local providers retain 9.2's --dir rerun.

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

Call imports are answered by the file's fakes; a missing fake fails the
step with `trap` and `no fake`. Delivery invokes only subscription exports;
a different export or missing function fails with `no_function`. `emits`
compares the events of either kind of step. Uncovered-function warnings
remain deferred until `Harness::functions()` exists.

An open failure is one file-level
`test.contract` Problem; no step failure is recorded since no step ran.
A malformed file reports `test.contract_file` with its file
and, for invalid JSON syntax, its line and column. File errors do not
prevent the remaining files from running.

Contract step failure fixes are `Fix <file> step <n> (<call>), then run:
watane-dev test --json`. A consumer's serve failure uses the same action
with `watane-dev serve --json` for `next`; 9.2's local provider rerun
takes precedence.

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
  timeout (2 s for `status` and `releases`, `--wait` for install and publish, 60 s in a
  preview). dev sends no `Origin` header. An error answer is
  `{"error": {"code": "...", "message": "..."}}`: code `refused` is
  `host.declined`; any other code is `host.refused`; a timeout is
  `host.no_answer`. Release-list errors are always `host.refused`; a publish
  409 `version_exists` uses 9.7.

### 9.2 Local providers and order

`deps::local_order(dir)` reads `watane_pkg::Deps::read(dir)` and follows each provider with no
publisher, through the folder that `.watane-dev.json` names for its slug
(`mod.provider_missing` when there is none or it has no `mod.toml`), depth
first, and returns the folders with providers before their users and `dir`
last. A folder seen twice on one path is `mod.dependency_cycle`. Step 5:
a provider with a publisher is the folder `<cache>/providers/<id>/` of its
`release_providers` entry, whose ReleaseRef must equal
`<publisher>/<slug>@<version in wit/deps/<slug>/>`; else
`mod.release_provider_missing`. Its own providers follow the same rule,
and it runs no test pipeline (it is a built package). Each
folder runs the `test` pipeline (sections 5 to 8). Traversal reads dependency
metadata without validating the world, so a pending edit after `add` does
not hide a cycle or a provider's test failure. The first folder that
fails stops the command. Each Problem of a provider gets the prefix
`<provider slug>: ` in its message, its `file` is absolute, and its `fix`
and the command's `next` are
`watane-dev test --json --dir <absolute provider folder>`.

### 9.3 `serve`

1. Resolve `deps::local_order(dir)` (9.2), then run the whole `test` pipeline
   (check, build, unit, contract) for each folder, providers first.
   On any error return the test errors without starting or updating a
   preview. Look for an answering preview using the canonical-path hash
   (as in 9.4), its cached ready line and `Client::for_home`. If one answers,
   add `preview.previous` with the served mod's page URL, or the API URL
   when it has no page. Construct URLs from the validated ready port and
   cached installed local name as in step 5; do not follow cached URLs.
   After installation, refresh the cached Ready mods in install order,
   retaining page presence so later source edits cannot hide the old page.
   Failure to find or verify a preview
   adds no warning and leaves the test errors unchanged. This applies only
   to `serve`, never `serve --stop`.
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
   released provider installs from its cache folder as a local mod; the
   preview links it by slug (W27). A
   folder with the slug of an installed local mod updates it; its state
   stays, because the host keys state by identity (`local/<slug>`).
5. Report `pages` from the `installed` answer (`slug`, `local`, `page`),
   build `api` and `mcp.url` from the port in `P/ready.json`, using
   `http://localhost:<port>/api/` and `http://localhost:<port>/mcp/preview`.
   Set `mcp.toolset` to `preview` and take `mcp.token` from the cached ready
   token; without a token, report `mcp: null`. Never follow ready-line URLs.

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

Resolve a relative provider folder against the current working folder, independently of `--dir`.
From a provider folder, `add . --dir <consumer>` works with a relative or
absolute consumer folder and records the same relative provider path as
`add <absolute provider> --dir <consumer>`. The command regression
`add::ac4_add_dot_from_provider_with_relative_and_absolute_consumer` proves both forms.
Run elsewhere (the consumer's parent, a subfolder of the provider), `.` is
`usage.provider` naming the canonical folder and the current-folder rule (2.2);
`add::regress_s12b_add_dot_outside_provider_names_folder_and_cwd_rule` proves it.

1. Canonicalize the folders; a missing provider or one without `mod.toml`
   is `usage.provider`. The same canonical folder as this mod is
   `mod.self_dependency`. Traverse the proposed provider edge before
   `Mod::read(provider)`: a path back to this folder is
   `mod.dependency_cycle`, even if the provider has not yet pasted its
   import lines after `add`. Otherwise a failed provider read is
   `usage.provider`.
2. `watane_pkg::add_provider(dir, &provider, None)`: it copies the WIT into
   `wit/deps/<slug>/` and sets `"mods:<slug>": null` in `deps.json`. Its
   errors are `mod.invalid`.
3. Set `local_providers.<slug>` in `.watane-dev.json` to the path of the
   provider relative to this folder.
4. Report an `import <iface>;` line for each interface of
   `provider.world().exports` and an `export <iface>;` line for each of
   `provider.world().events` (full names with version).

### 9.6 `install` (step 2)

Before any mod traversal or test pipeline, resolve home: `--home`, else
`watane_host::default_home()`; canonicalize when it exists. Read its `RunInfo`.
`GET /api/status` must match the home, port and protocol version and
say `mode: "home"` and `app: true`; else `env.host_not_running` with the
cause ("the host of <home> is a preview" / "the host of <home> runs
without the app"). Run 9.2, then one `POST /api/install` with
`{"folders": [...]}`. The host packs the folders, plans, and waits for the
participant in the app. `--wait <seconds>` (default 600) bounds this request.
No answer is `host.no_answer`: `no answer from the home host in <n> s`,
fix `Run the command again with a longer --wait`. A `refused` answer is
`host.declined`: say the person declined installing the requested consumer
slug (or its mod folder) in their watane app. The fix asks the person
whether they want it installed, then says to run install again with
`--wait <seconds>` (2.2). `api::tests::regress_s12b_declined_fix_does_not_ask_to_correct_and_retry`
proves the message and fix; the fix must not say "correct". App 5xx errors
follow 2.2: 503 `unavailable` is `env.host_not_running`, message
`the watane app of <home> runs but could not open its data: <app message>`,
fix `Ask the person to restart the watane app, then run this command again`.
Other 5xx answers from install, publish, search and releases are `host.failed`
with `[<code>] <message>` and the app-log fix of 2.2. 4xx keeps
`host.refused` and its current fixes (or `host.declined` for consent).
Step 5: a `missing_providers` answer that names
a provider with a publisher gets the fix `Install <ReleaseRef> in the
watane app (from its link), then run install again`. When
`.watane-dev.json` `release_providers.<slug>.link` is present, use
`Install <ReleaseRef> in the watane app from its link <link>, then run install again`.
A home installs a release only through the app. dev reports the `installed` answer.
Pipeline failures send no install request and retain the provider-specific
next command and fixes of `serve`.

### 9.7 `publish` (step 3)

1. Read `Deps::read(dir)` before validating the mod or contacting a host.
   Each provider with no publisher is one `publish.local_dependency` at
   `deps.json`; report all local providers and stop before the pipeline.
2. Resolve the home and check app status through the same path as `install`
   (9.6), before the pipeline. Missing-app `next` repeats `publish` with
   the canonical mod folder, home and any explicit `--wait` (shell quoted).
2b. `Client::releases()` sends `GET /api/releases` with the status timeout
   (2 s). Match the mod's slug and version from `wit/world.wit`, using pkg's
   `Mod` identity. Invalid mods retain the test pipeline's diagnostics. On a
   match, return one `publish.version_exists` (2.2), with the existing release
   and its current link (or the missing-site-URL message), file `wit/world.wit`,
   `result` containing `home`, `release` and `link`, and `next` equal to the
   fix. Run no test pipeline, pack or POST. Another
   slug or version does not block publishing. Error answers are `host.refused`.
3. Test the mod (sections 5 to 8). Failures send no publish request and
   retain the test pipeline's errors and `next`, as `install` does.
4. `watane_pkg::pack(dir, !no_source)`: include source unless `--no-source`.
5. `Client::publish_with_links(&package, wait, &links)` posts to `/api/publish` with
   `Content-Type: application/octet-stream` and `Package::as_bytes()` as
   the body, using the proxy-free localhost transport of `install`.
   `--wait` defaults to 600 seconds. The app asks the participant, who sees
   whether source is included; consent is the agreement to include it
   (F-PUB-4). Step 5: the header `Watane-Provider-Links` carries the
   non-null `link` of each `release_providers` entry whose slug is a
   provider in `deps.json` (app 8.1), and the request timeout is `--wait`
   times (1 + the number of providers with another publisher's key), since
   the app asks for the release and then for each listing. The own key is
   inferred from `/api/releases` when it has a release; before the first
   publication, count every published provider as potentially foreign, giving
   enough time for every listing. `dist` signs and writes the site folder. dev reports only
   `home`, the answer's `release`, `site`, `link`, `package` (`Package::id()`)
   and `source` (`Package::has_source()`), as in 2.4, and step 5's `providers`.

A `refused` answer is `host.declined`: `The person declined publishing
<slug> from their watane app`, with fix `Ask the person whether they want
<slug> published, then run publish again with --wait <seconds>`.
The CLI supplies the slug; the fixed `Client::publish` interface carries
only a package and duration. A timeout uses the message and fix of 9.6;
a 409 `version_exists` answer (a race after step 2b) is
`publish.version_exists`, with the app's message unchanged and the same
version-bump fix, file and `next` as the preflight refusal. Other errors are
`host.refused` with `[<code>] <message>`. For these host failures, `next` is
the fix. Success chooses the link-dependent instruction of 2.4.

## 10. Step 4: `search`; step 5: `new --from`, `add --release`

`search` needs no mod folder (`dir` is null). It resolves the home and
checks the app status as `install` does (9.6, step 1: `env.host_not_running`
with the same causes). On a missing app, `next` repeats
`watane-dev search --json <same words> [--publisher <key>] [--interface <name>] --home <absolute home>`;
quote each word and option value for the shell as in 2.4 (including empty words).
No mod folder is read, even with `--dir`. Then send one `POST /api/search` with
`{"text": <the words joined with one space, or "">, "publisher", "interface"}`
over the proxy-free localhost transport, with a 30 s timeout (a search
makes no network request in the app). dev checks nothing of the query
itself: a `400` answer is `host.refused` with `[bad_request] <message>`,
whose fix is `Check --publisher (a 52-character key) and --interface (for
example mods:reading-list/books or mods:reading-list/books@0.1.0)`. A
search gives no authority and asks the participant nothing. Other error
answers are `host.refused` with `[<code>] <message>`, including `refused`.
For every search request error, `next` is the error's fix. dev never
installs a result: the person does that in the app (F-AGENT-4).

### 10.1 Released packages (step 5)

`Client::fetch(target)` sends `POST /api/fetch` with `{"link": <text>}` or
`{"release": <ReleaseRef text>}` (app 8.3) over the proxy-free localhost
transport, with a 330 s timeout (dist's package timeout plus 30 s). It
reads at most `MAX_PACKAGE_BYTES` + 1 bytes (more is `host.refused`
`[too_large]`). `Package::from_bytes` must accept the body and
`PackageId::of(body)` must equal the `Watane-Package` header; else
`host.refused` `[bad_answer] <detail>`. The package's WIT slug and version
must also match `Watane-Release`.
The `Watane-Release` header parses as a `ReleaseRef` and must
match the request: equal to the asked ReleaseRef, or with the key and
slug of the asked link. Missing or unparseable `Watane-Release` or
`Watane-Package`, a non-package body, a mismatched package id, or a WIT
slug/version mismatch are all `host.refused` `[bad_answer] <detail>`;
nothing is stored. Each uses the fix `The watane app answered with a package
that does not match the request; ask the person to update the watane app,
then run this command again`. `Link` belongs to dist, which dev does
not depend on: dev reads only the key between `watane:` and the first `@`,
and the slug after the last `#`, to match `Watane-Release`; the app validates
the link and answers 400 when it is invalid. dev trusts
the app of its own home for the signature check (dist 5.7); it checks the
bytes again because they cross a socket. Error answers are `host.refused`
`[<code>] <message>`; a 400 gets the fix `Pass a mod link
watane:<key>@<url>#<slug> or a release <key>/<slug>@<version>`.
409 `fetch_refused` keeps `host.refused` `[fetch_refused] <message>`,
fix `Check the link or release with the person who shared it: the watane
app found no release to fetch for it`. 502 `fetch_failed` also stays
`host.refused`, fix `Check the link and that its publisher's site is
reachable, then run this command again`. Other 5xx follow 9.6
(503 `unavailable` → `env.host_not_running`; others → `host.failed`).
No fetch refusal fix asks the agent to check the app.

`store(package)`: write `<cache>/packages/<id>` (temp file, rename) and,
remove any `<cache>/providers/<id>.<pid>.tmp` left by this process id.
When `<cache>/providers/<id>` is absent, unpack into that temporary folder
and rename it to `<cache>/providers/<id>`.
When the destination already exists, keep it and remove the temporary folder;
a concurrent store winning the rename is handled the same way. A killed
run's `<id>.<other>.tmp` is never read and never blocks a store.
The paths come only from the id dev computed.

`fetch_providers(dir)`: for each provider of `Deps::read(dir)` with a
publisher, the ReleaseRef `<publisher>/<slug>@<version of
wit/deps/<slug>/>`. If `release_providers.<slug>` names it and its cache
folder `<cache>/providers/<id>` exists, use it (never a `.tmp` folder);
else `Client::fetch({"release"})` and `store`. Then
the same for the providers of that cached folder. At most 32 packages per
command; more is `host.refused` with `too many released providers`.
Record each one in `release_providers` (sorted keys). Return the list.

### 10.2 `new [<slug>] --from <link>` (step 5)

1. `<slug>`, when given, must parse (`usage.slug`). With `--dir` that path
   is the folder, else `./<slug>`; it must not exist (`usage.exists`).
2. Resolve the home and check the app status as `install` (9.6 step 1).
3. `Client::fetch({"link"})`. `!package.has_source()`: `usage.no_source`,
   and nothing is written.
4. With no `<slug>`, the slug is the release's, and the folder check of
   step 1 runs now.
5. `package.unpack(folder)` (pkg 6.4: a new folder only; it refuses an
   existing one). Write dev's own `AGENTS.md`, `CLAUDE.md` and
   `.gitignore` of the template (section 4): a package never holds them
   (pkg 6.1), so the instructions that the agent reads come only from
   `watane-dev`, never from the publisher.
6. Set `[origin] release = "<ReleaseRef>"` in `mod.toml` (a `toml::Table`
   edit that replaces an existing `[origin]`; W24: the direct parent).
7. When the slug differs, replace the slug in the package line of
   `wit/world.wit` and keep the version, so a same-slug derivative can fit
   in place of its origin (W26). Collect `mentions` (2.4). dev changes no
   other file: the coding agent edits the code.
8. `fetch_providers(folder)`, then `Mod::read(folder)`: a failure here is
   reported (`mod.invalid`) with the folder kept, because the agent fixes
   it from `mentions`. Any failure before step 5 writes nothing; a failure
   in steps 5 to 7 removes the folder.

### 10.3 `add --release <link or release>` (step 5)

1. Resolve the home and check the app status as `install` (9.6 step 1).
2. `Client::fetch`: a text that starts with `watane:` is a link, else a
   ReleaseRef (`usage.args` when it parses as neither). `store` it.
3. The provider is the cached folder's `Mod`. Its slug equal to this
   mod's: `mod.self_dependency`.
4. `watane_pkg::add_provider(dir, &provider, Some(publisher))` (pkg 4.8):
   its WIT and its own `wit/deps/` entries; `deps.json` gets the key.
   Remove `local_providers.<slug>` from `.watane-dev.json`; set
   `release_providers.<slug>` with the link when one was given.
5. `fetch_providers(dir)` for the providers of the provider.
6. Report as `add` (9.5 step 4), with the fields of 2.4.

No consent: the app fetches and follows, and installs nothing (app 8.3).
Contract tests fake the provider (section 8), so `test` needs no network
and no app. `serve` installs the cached folders in the preview (9.2, 9.3).
`install` in a home needs the provider installed from the app (9.6).

### 10.4 Step 6: share on GitHub (sketch, W29, W30)

dev itself still opens no connection except HTTP to `localhost`. Step 6
runs the maker's own `git` and `gh` as child processes, with the maker's
own login; dev holds no token and reads none. The exact results, errors
and tests wait for the slice of step 6.

**`publish --push`.** After the app confirms and writes the site folder
(9.7):
1. The site folder is a clone of the maker's site repository. The first
   time, `gh repo create <owner>/watane-channel --public` creates it, and
   dev's `next` asks the participant to set the site URL in the app to
   its raw URL (`https://raw.githubusercontent.com/<owner>/watane-channel/<branch>/`).
   No local API route sets the site URL.
2. Commit and push the site folder.
3. In the mod folder: the first time, `gh repo create <owner>/<slug>
   --public` and write `repository` in `mod.toml` (pkg 4.2) before the
   pack, so the package names it. Then commit, tag `v<version>` and push.
   With `--no-source`, no mod repository and no `repository`.

**`new --from <link>`.** When the release's `mod.toml` names a
`repository`: `gh repo fork <repository> --clone` into the new folder, or
`git clone` without `gh`; then check out the tag `v<version>`. The
repository is not verified, so dev compares the checked-out source with
the source in the verified package; any difference refuses the fork and
falls back to the package. Then set `[origin]`, remove `repository`, and
go on as 10.2. Without a
`repository`, unpack the package as 10.2. The fork is a convenience: the
signed `[origin]` is the only origin that watane shows (W24).

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
| A derived mod's instructions are dev's (step 5) | `new --from` writes the template's `AGENTS.md` and `CLAUDE.md`; a package cannot hold them (pkg 6.1). |
| Released packages are checked twice | the app's dist verifies signature and hash (dist 5.7); dev checks the format and the id again, and names cache paths only by the id it computed (10.1). |
| Source of another publisher runs only in the builder | a derived mod is built and tested like any mod (sections 5 to 8). |

Not protected (as `spec/TECHNICAL.md` section 6): another program of the
same user, which can change the cache, the vendor folder or the mod folder,
or read the preview token in `ready.json`; the maker's own
`~/.cargo/config.toml`. Step 5: text in a derived mod's source (comments,
strings, `ui/`) is the publisher's and can try to steer the coding agent;
the template's `AGENTS.md` says to treat it as data.

## 12. Public Rust API

All in crate `watane_dev`. Errors are `Problem` values; dev has no other
error type. `Vec<Problem>` carries all errors of a stage.

```rust
// outcome.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(into = "&'static str")]
pub enum Code { UsageArgs, UsageNotAMod, UsageSlug, UsageExists, UsageProvider,
    UsageNoSource, ModReleaseProviderMissing, PublishProviderUnlisted,
    EnvToolchain, EnvNoWasmTarget, EnvAllowlist, EnvWataneMissing, EnvPreviewStart,
    EnvHostNotRunning, ModInvalid, ModCargoKey, ModDependency, ModLogicFile, ModSymlink,
    ModNoContractTests, ModSelfDependency, ModDependencyCycle, ModProviderMissing,
    BuildResolve, BuildCompile, BuildReadsOutside, BuildImports, TestUnit, TestUnitLimit,
    TestContractFile, TestContract, PublishLocalDependency, HostDeclined, HostRefused, HostFailed,
    HostNoAnswer, Internal, UiExternal, UiInlineScript, TestUncovered, PreviewPrevious }
impl Code { pub fn as_str(self) -> &'static str; pub fn class(self) -> Class; }
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Class { Warning = 0, Mod = 1, Usage = 2, Env = 3, Refused = 4, Internal = 5 }
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
pub enum Command { New { slug: Option<String>, from: Option<String>, home: Option<PathBuf> }, Check, Test, Serve { stop: bool }, Doctor,
    Add { provider: Option<PathBuf>, release: Option<String>, home: Option<PathBuf> },
    Install { home: Option<PathBuf>, wait: Option<u64> },
    Publish { no_source: bool, home: Option<PathBuf>, wait: Option<u64> },
    Search { text: Vec<String>, publisher: Option<String>, interface: Option<String>, home: Option<PathBuf> } }
pub fn run(cli: Cli) -> Outcome;

// env.rs
pub struct Env { pub cache: PathBuf, pub unit_seconds: u64 }
impl Env { pub fn from_process() -> Result<Env, Problem>; }

// template.rs
#[derive(Serialize)] pub struct NewReport { pub slug: String, pub dir: PathBuf, pub files: Vec<String> }
pub fn new_mod(dir: &Path, slug: &str) -> Result<NewReport, Problem>;
pub fn render(text: &str, slug: &watane_pkg::Slug) -> String;
pub fn write_guides(dir: &Path, slug: &watane_pkg::Slug) -> Result<(), Problem>;

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
pub enum Target { Call(String), Deliver(String) }
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

// build_on.rs
#[derive(Clone, Deserialize, Serialize)]
pub struct ReleaseProvider { pub release: watane_pkg::ReleaseRef, pub package: watane_pkg::PackageId, pub link: Option<String> }
pub fn provider_release(dir: &Path, slug: &watane_pkg::Slug, publisher: &watane_pkg::PublisherKey) -> Result<watane_pkg::ReleaseRef, Problem>;
pub fn store(package: &watane_pkg::Package, env: &Env) -> Result<PathBuf, Problem>;
pub fn fetch_providers(dir: &Path, client: &Client, env: &Env) -> Result<Vec<watane_pkg::ReleaseRef>, Vec<Problem>>;
pub fn new_from(destination: Option<&Path>, slug: Option<&str>, target: &str, client: &Client, env: &Env) -> Result<serde_json::Value, Vec<Problem>>;
pub fn add_release(dir: &Path, target: &str, client: &Client, env: &Env) -> Result<serde_json::Value, Vec<Problem>>;

// api.rs
pub struct Client { /* base URL, ureq::Agent */ }
impl Client {
    pub fn for_home(home: &Path) -> Result<Option<Client>, Problem>;   // None: no run.json or no answer
    pub fn home_status(home: &Path) -> Option<serde_json::Value>;
    pub fn for_app(home: &Path) -> Result<Client, Problem>;   // 9.6: a home host with the app, else env.host_not_running
    pub fn install(&self, folders: &[PathBuf]) -> Result<Vec<InstalledMod>, Problem>;   // preview: 60 s
    pub fn install_wait(&self, folders: &[PathBuf], wait: u64) -> Result<Vec<InstalledMod>, Problem>;   // home: --wait seconds
    pub fn search(&self, text: &str, publisher: Option<&str>, interface: Option<&str>) -> Result<serde_json::Value, Problem>;   // POST /api/search, 30 s
    pub fn releases(&self) -> Result<serde_json::Value, Problem>;   // GET /api/releases, status timeout (2 s)
    pub fn publish_with_links(&self, package: &watane_pkg::Package, wait: Duration, links: &[String]) -> Result<serde_json::Value, Problem>;
    pub fn fetch(&self, target: &str) -> Result<(watane_pkg::ReleaseRef, watane_pkg::Package), Problem>;
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
- S39 needs own-package violations to name the actual `wit/deps/<folder>`,
  even when the folder differs from the mod slug. Current pkg only recognizes
  the matching-folder copy and reports other copies as a duplicate WIT package.
  Dev's mapper uses a literal violation until pkg supplies this diagnostic;
  the corresponding command test is `needs watane-pkg` (17.13).
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
- `Harness::open(built_mod, Fakes)`, `Harness::call(function, args)`;
  `Fakes`, `Outcome { value, events }`, `EventRecord { interface, function, args }`,
  `CallError { code, message }` whose `CallCode` is serialized to its wire
  text by `CallError` (used for `fails` matching).
  `Harness::deliver(function, args)` invokes a short subscription name;
  `Harness::functions()` and
  `FunctionInfo` join with S5 toolsets, before uncovered-function warnings.
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
after the participant confirms; code `refused` for a no. A duplicate version
returns 409 `version_exists` with message `release <ref> already exists at seq <n>`.
`GET /api/releases` returns 200
`{"releases":[{"slug":string,"version":string,"release":string,"seq":number,"link":string|null}]}`,
one object per own release in seq order (empty: `{"releases":[]}`), using the
current site URL; no site URL means `link: null`. Dist not open gives 503
`unavailable`. This supports discovery before building and after a publish
wait expires; dev maps an error answer to `host.refused`. The consent view
shows whether the package holds source.
`POST /api/search` takes `{"text":string,"publisher":string|null,"interface":string|null}`
and returns `{"indexes":number,"results":[...]}` (app 8.2); 400 is
`bad_request`, 503 is `unavailable`. Stage 8 uses fake-watane while S29 builds
the real route. Step 5: `POST /api/fetch` with `{"link"}` or `{"release"}`
answers the package bytes with `Watane-Release` and `Watane-Package`
headers (app 8.3); `/api/publish` adds `providers` (app 8.1).

**pkg** (step 5): `Package::unpack`, `PackageId::of`, `MAX_PACKAGE_BYTES`,
`ReleaseRef: FromStr`, `add_provider(dir, &provider, Some(key))`, and
`Mod::origin`; `mod.toml` `[origin]` (pkg 4.2).

**host** (step 5): a preview links a provider by slug (W27, host 4.2).

**log**, **dist**: nothing directly; through the home host only.

## 15. Test plan

Each oracle is literal, read independently from fixture files, or computed by
an independent tool; never call the code under test for the expectation.
Default `cargo test -p watane-dev` has a 120 s wall budget with binaries compiled.
Cold builds, real preview life cycles and journeys are `slow:`; asserted
execution deadlines are `timing:`. The S16 missing-vendor regression runs by default (S17-1).
The vendor-script integration runs offline only when registry metadata confirms
that the local cache has the crates; otherwise its ignored
`ac3_vendor_script_with_registry_downloads` counterpart runs the bootstrap.

Cache experiment (S13): original cold 33.41 s, same path warm 6.78 s, copied cache 13.68 s; the cache-path remap changes Cargo's rustflags fingerprint, so `tests/support/mod.rs` uses `OnceLock<TempDir>` to build one template per binary and shares that original cache folder, never a copy.

Tests keep their mod folders independent. The builder's per-slug lock guards
source preparation and compilation; the unit fixture also serializes its
runs. Tests of two caches, stale dep-info, or missing vendor keep isolated
caches. Only the first two need cold builds and are slow. Commands which
stop before compilation need no warmed cache. No product code changes.

### 15.1 Unit tests (in the modules)

| Module | Tests and rules | Level / oracle |
|---|---|---|
| outcome | `ac3_exit_classes_and_problem_envelope`, `ac3_previous_preview_is_only_a_warning` | unit: literal status 1–5, codes, JSON and warning text |
| template | existing render, page and guide tests | unit: literal strings and file contents |
| check | `ac3_twenty_literal_manifests`, `ac3_invalid_toml_names_file_and_line`, `ac3_dev_s5_and_s8_file_codes_locations_and_fixes`, `ac3_dev_s6_and_s7_manifest_codes_locations_and_fixes`, `ac3_check_repairs_host_wit_and_skips_git_links`, `ac3_logic_needs_contract_but_page_only_does_not` | unit: every builder check code, file, line, message and fix; missing locations are literal null |
| allowlist | `ac3_source_vendor_fix_is_absolute_bootstrap_command`, `ac3_custom_vendor_fix_targets_that_path`, `ac3_vendor_lookup_order_and_fix_texts`, existing pins/checksum tests | unit: literal pins, error, lookup order and bootstrap output-path fix |
| toolchain | `ac3_doctor_toolchain_gap_texts_and_fixes` | unit: toolchain and missing-target error/fix |
| builder | `ac3_dep_info_parses_escaped_spaces_and_checks_all_allowed_roots`, `ac6_cargo_config_quotes_paths_and_restricts_environment`, `ac3_compiler_failure_code_location_and_fix` | unit: literal paths, flags and environment keys |
| unit | existing `ac1_parse_*`, `ac2_parse_*`; `ac3_panic_classification_counts_location_and_fix`, `ac3_wasm_exit_trap_and_limit_classifications`, `ac3_unit_host_imports_are_unknown_import_traps` | unit: literal libtest output; tiny literal WASI commands prove unsuccessful exit, interrupt, memory limit and trap classifications without Cargo |
| contract | existing parse/run_steps tests, `ac3_single_event_equality_and_failure_payloads` | unit: literal returns, failures, events, short/full names, order, numeric equality, delivery and fakes |
| preview | `ac4_mcp_from_literal_ready`, `ac6_serve_posts_all_folders_once_in_provider_order`, `ac7_previous_warning_requires_answering_preview_and_names_page` | unit: literal Ready/JSON, loopback server; no real host |
| cli | existing provider-failure and next-page tests; `ac4_install_next_uses_last_available_page`, `ac7_install_help_and_default_wait`, `ac8_template_explains_home_install` | unit: absolute next/fixes, first failing provider, consumer page, install follow-up/help/guide |
| api | `ac2_wrong_host_status`, `ac5_install_timeout_and_host_errors`, `ac5_install_timeout_in_response_body`, `ac7_missing_released_provider_names_app_install_fix`, `ac3_fetch_refuses_corrupt_headers_bytes_and_app_errors` | loopback unit: literal status and error answers, delayed headers and body |
| json_eq | `ac7_json_equality_literal_pairs` | unit: literal number, type, nesting, order and large-integer pairs |
| deps | `ac3_missing_provider_text_and_fix`, `ac3_local_order_and_missing_folders` | unit: literal order, missing-provider message/fix |
| build_on | `ac1_package_declaration_accepts_comments_and_whitespace`, `ac1_files_are_sorted_by_relative_path_text` | unit: WIT package declaration with comments and spacing; textual path order |

### 15.2 Scenario proofs (`tests/` unless qualified)

Names below identify current proofs, including the slow set. Planned scenarios
without an implementation had no test before S13; this table does not imply
they pass. Command tests keep exit status, envelope and code wiring; detailed
rules belong to 15.1.

| Scenario | Test name(s) | Level |
|---|---|---|
| DEV-S-1 | `envelope::ac1_new_counter_template_envelope` | integration |
| DEV-S-2 | `envelope::ac2_existing_and_invalid_slug_change_nothing` | integration |
| DEV-S-3 | `envelope::ac2_existing_and_invalid_slug_change_nothing` | integration |
| DEV-S-4 | `unit_tests::ac1_ac2_template_unit_counts_and_warm_pipeline`, `contract::ac1_template_counts_and_component_export` | integration, shared warm cache; slow component-export pipeline |
| DEV-S-5 | `check::tests::ac3_dev_s5_and_s8_file_codes_locations_and_fixes`; `builder::ac4_serve_rejects_before_cargo` | unit; integration wiring |
| DEV-S-6 | `check::tests::ac3_dev_s6_and_s7_manifest_codes_locations_and_fixes`; `builder::ac4_serve_rejects_before_cargo` | unit; integration wiring |
| DEV-S-7 | `check::tests::ac3_dev_s6_and_s7_manifest_codes_locations_and_fixes`; `builder::ac4_serve_rejects_before_cargo` | unit; integration wiring |
| DEV-S-8 | `check::tests::ac3_dev_s5_and_s8_file_codes_locations_and_fixes`; `check::ac2_check_counter_mod_and_rejections` | unit; integration wiring |
| DEV-S-9 | `builder::ac5_missing_target_stops_before_cargo`, `doctor::ac5_missing_target_retains_toolchain_without_running_cargo`; `toolchain::tests::ac3_doctor_toolchain_gap_texts_and_fixes` | integration; unit |
| DEV-S-10 | `builder::ac1_build_refuses_absolute_bytes_strings_and_relative_outside_reads`, `unit_tests::ac2_unit_build_refuses_outside_reads_before_running_tests`, `unit_tests::ac3_fixed_mod_discards_stale_release_and_unit_dependencies` | slow |
| DEV-S-11 | `builder::ac7_build_sees_no_maker_secret`, `builder::ac4_test_build_does_not_receive_shell_secret` | slow |
| DEV-S-12 | `builder::ac5_same_mod_at_different_paths_and_caches_has_identical_path_free_wasm` | slow: two cold caches |
| DEV-S-13 | `builder::ac6_build_with_network_denied` → `ac6_build_counter_component` | slow; OS refusal permitted for nested sandbox |
| DEV-S-14 | `unit_tests::ac4_unit_tests_have_no_files_environment_or_network`, `ac4_unit_host_imports_trap`; `unit::tests::ac3_unit_host_imports_are_unknown_import_traps` | integration, warm cache; slow Rust import wiring; unit trap |
| DEV-S-15 | `unit_tests::ac2_panic_names_assert_and_counts_remaining_tests`; `unit::tests::ac3_panic_classification_counts_location_and_fix` | integration; unit |
| DEV-S-16 | `unit_tests::ac11_endless_test_stops_with_exact_message` (default suite, exact status/code/message, 60 s hang guard); `unit_tests::ac10_endless_test_stops_with_exact_message_within_deadline` (7 s); `unit::tests::ac3_wasm_exit_trap_and_limit_classifications` | integration; timing; unit classification |
| DEV-S-17 | `contract::ac3_each_contract_file_starts_from_fresh_state` (committed Wasm); `contract::ac3_literal_fresh_counter_failure_and_fresh_state_per_file` (slow: full pipeline); `contract::tests::ac7_run_steps_uses_closure_arguments_and_continues_after_failures` | integration; unit |
| DEV-S-18 | `contract::ac1_s11_fake_answers_provider_call`, `ac1_s11_missing_fake_fails` | integration: committed Wasm fixture |
| DEV-S-19 | `contract::ac2_s11_deliver_retains_state_and_emits`, `ac2_s11_deliver_compares_emitted_events`, `ac2_s11_deliver_rejects_non_subscription`; `contract::tests::ac4_run_steps_events_exact_order_names_and_failed_calls`, `ac3_single_event_equality_and_failure_payloads` | integration: prebuilt; unit |
| DEV-S-20 | `unit_tests::ac5_logic_requires_contract_file_but_page_only_does_not`, `contract::ac8_serve_does_not_start_preview_for_failed_or_absent_contracts`; `check::tests::ac3_logic_needs_contract_but_page_only_does_not` | integration; slow serve pipeline; unit |
| DEV-S-21 | `serve::ac4_real_preview_serves_page_and_csp`, `ac9_real_counter_page_calls_logic`, `ac1_preview_mcp_lists_and_calls_counter` | slow: needs WATANE_BIN |
| DEV-S-22 | `serve::ac5_serve_reuses_preview_and_updates_page`, `ac2_edit_keeps_data_and_updates_logic_and_page`, `ac2_reused_preview_keeps_mcp` | slow: needs WATANE_BIN |
| DEV-S-23 | `serve::ac6_stop_closes_port_and_removes_preview`, `ac6_stop_works_after_mod_files_are_broken`; stop regression tests in `serve.rs` | slow: needs WATANE_BIN; integration stop regressions |
| DEV-S-24 | `serve::ac7b_missing_host_has_environment_error_and_doctor_next` | integration |
| DEV-S-25 | `add::ac1_dev_s25_add_copies_wit_and_reports_lines`, `ac1_two_adds_write_sorted_local_paths_and_preserve_consumer_world` | integration |
| DEV-S-26 | `add::ac2_dev_s26_cycle_changes_nothing`, `ac3_self_and_missing_provider` | integration |
| DEV-S-27 | `install::ac1_no_host_precedes_pipeline`; `api::tests::ac2_wrong_host_status`; `install::ac2_real_home_without_app` | integration without build; loopback unit; slow real host |
| DEV-S-28 | `install::fake::ac3_declined` | integration: page-only, no Rust build |
| DEV-S-29 | `install::fake::ac4_page_only_result_and_failed_pipeline`; `install::fake::ac4_provider_order_result_and_failed_pipeline` | integration: page-only result, next and no request on failure; slow integration: order of two dependent Rust templates |
| DEV-S-30 | `publish::ac1_local_providers_precede_status_and_pipeline` | integration: no build or host request |
| DEV-S-31 | `publish::ac3_ac6_page_package_and_answer`, `publish::ac3_logic_source_selection` | page-only integration; slow Rust source selection |
| DEV-S-33 | `search::s33_results_pass_through` | integration against `fake-watane` |
| DEV-S-34 | `search::s34_no_index_next` | integration against `fake-watane` |
| DEV-S-32 | `envelope::ac8_json_argument_errors_use_envelope`, `check::ac2_check_counter_mod_and_rejections`; `outcome::tests::ac3_exit_classes_and_problem_envelope` | integration; unit all exit classes |
| DEV-S-35 | `build_on::ac1_derive_source_guides_origin_mentions_and_default_slug` | integration: fake fetch, source package |
| DEV-S-36 | `build_on::ac2_no_source_writes_nothing` | integration: no-source package |
| DEV-S-37 | `build_on::ac3_exists_precedes_fetch_and_absent_app_writes_nothing` | integration: no build |
| DEV-S-38 | `build_on::ac4_add_release_records_publisher_wit_link_and_rejects_bad_target`, `build_on::ac4_transitive_releases_are_fetched_by_reference_and_bounded`, `api::tests::ac4_released_provider_walk_stops_at_thirty_two_packages` | integration: prebuilt provider |
| DEV-S-39 | `build_on::ac5_offline_contract_and_fake_preview_provider_order`, `build_on::ac5_missing_released_cache_has_actionable_fix`, `build_on::ac5_real_preview_installs_released_provider_before_consumer` | integration: warm consumer pipeline; slow real preview |
| DEV-S-40 | `build_on::ac6_publish_unlisted_warning_provider_links_and_default_list`, `build_on::ac6_publish_passes_recorded_provider_link`, `cli::tests::ac6_publish_timeout_allows_each_foreign_provider_listing` | integration: page; warm consumer pipeline |
| J1 | `journey::ac1_j1_reading_list_on_real_programs` | journey: slow, needs WATANE_BIN |
| S11 | `journey_s11::ac4_s11_call_and_event_across_two_served_mods` | journey: slow, needs WATANE_BIN |

S17-1 moves `ac3_literal_fresh_counter_failure_and_fresh_state_per_file` unchanged
to the slow set: it drives the full check/build/unit/contract pipeline;
`ac3_each_contract_file_starts_from_fresh_state` proves fresh state per file on
the committed recommendations component.
`ac6_bad_contract_file_names_file` uses the committed recommendations component,
so its parser/location proof needs no template build.

The successful unit pipeline also proves unchanged dependency timestamps, avoiding
a second command just to check cache reuse.

Removed command assertions and their named replacements (S13):

| Removed test/assertions | Replacement proving the same rule |
|---|---|
| `check::ac3_check_rejects_builder_inputs`: build script, build-dependencies, proc macro, rand, named file and allowed-crates fix | `check::tests::ac3_dev_s5_and_s8_file_codes_locations_and_fixes`, `ac3_dev_s6_and_s7_manifest_codes_locations_and_fixes`; command code/status wiring retained in `builder::ac4_serve_rejects_before_cargo` |
| `contract::ac4_events_match_short_and_full_names_and_exact_lists`: each literal event list, status, failure code, step, expected and got | `contract::tests::ac3_single_event_equality_and_failure_payloads`, `ac4_run_steps_events_exact_order_names_and_failed_calls`; contract error code/status remains in `ac3_literal_fresh_counter_failure_and_fresh_state_per_file` |
| `contract::ac5_expected_call_failures`: boolean, matching substring, mismatching substring, unexpected success and status/code | `contract::tests::ac5_run_steps_fails_boolean_and_substring_and_unexpected_results`; command error wiring remains in `ac3_literal_fresh_counter_failure_and_fresh_state_per_file` |
| `add::ac4_local_order_and_missing_folders`: provider order, both missing-folder forms, error code/text/fix | moved verbatim to `deps::tests::ac3_local_order_and_missing_folders` with direct fixture construction; also `ac3_missing_provider_text_and_fix` |
| `envelope::ac8_public_outcome_types_match_section_12`: class, status, code serialization, cloned problem envelope | moved to `outcome::tests::ac3_exit_classes_and_problem_envelope` |
| `contract::ac6_bad_contract_file_names_file`: command exit assertion | direct prebuilt-fixture integration still asserts `test.contract_file` and its file; `ac3_literal_fresh_counter_failure_and_fresh_state_per_file` keeps mod exit-class wiring, `contract::tests::ac6_parse_rejects_literal_invalid_files` proves syntax refusal |
| `serve::ac7b_missing_host_has_environment_error_and_doctor_next`: template logic fixture replaced by page-only fixture | same command error/fix/next assertions; logic pipeline reaching missing host remains in slow `builder::ac6_build_counter_component` |

The real-host slow set reads `WATANE_BIN`. Run it with
`cargo build -p watane-host` then
`WATANE_BIN=$PWD/../target/debug/watane cargo test -p watane-dev -- --include-ignored`.
DEV-S-13 may be refused by the outer OS sandbox; `timing:` tests require a
quiet machine. `envelope::ac4_ignored_tests_explain_their_cost_or_refusal`
checks the ignore reasons; `unit_tests::ac1_ac2_template_unit_counts_and_warm_pipeline`
checks cache identity and unchanged dependency artifact timestamps across reuse.
S12b adds page-only decline/text command pipelines and loopback unit stubs
to the default suite; no additional Rust fixture build or warm cache is needed.
The ordered successful install needs two logic mods (pkg forbids imports in
a page-only mod), so `install::fake::ac4_provider_order_result_and_failed_pipeline`
is slow and uses one isolated cache for both templates.
`api::tests::ac5_install_timeout_and_host_errors` proves the wait/error mapping;
`install::fake::ac6_fake_preview_status_publish_and_ready` proves the fake transport
with a real page package. S21c adds `publish::ac2_no_app_precedes_pipeline`,
`ac4_declined_names_publishing_and_asks_before_retry`,
`ac5_failed_pipeline_sends_nothing` and `ac7_text_output_and_help_walk`.
`api::tests::ac4_publish_request_and_answer` and
`ac4_publish_timeout_and_host_errors` prove the binary request and error mapping.
`api::tests::ac5_search_errors_preserve_app_messages_and_fixes` proves search
400/503/refused mapping; `ac5_search_timeout_is_thirty_seconds` is a timing
proof of the 30 s bound. `search::ac8_search_text_flattens_results` proves text output.
`publish::ac3_logic_source_selection` is slow because it builds and tests
Rust; the default publish proofs use page-only mods with a test source file,
adding no cold Rust fixture build to the 120-second budget.

S39 recovery proofs (default suite unless marked):

| Acceptance | Proof |
|---|---|
| AC1 | `build_on::tests::ac1_store_recovers_partial_temps_and_keeps_existing_provider`: partial current/killed temps, complete unpack, existing-folder preservation and no current temp |
| AC2 | `api::tests::ac2_fetch_errors_name_link_and_publisher_recovery`: exact 409/502 fix and message over loopback |
| AC3 | `api::tests::ac3_fetch_refuses_corrupt_headers_bytes_and_app_errors`: missing/malformed headers, mismatched release/id/body/WIT, invalid bytes, no store in fetch |
| AC4 | `api::tests::ac4_app_5xx_names_data_failure_or_app_log_for_every_request`, `outcome::tests::ac3_exit_classes_and_problem_envelope`, `search::ac2_search_app_failure_names_app`: all five routes, exact home/message/fix, code/class/exit |
| AC5 | `api::tests::ac5_missing_released_provider_includes_stored_link`; `ac7_missing_released_provider_names_app_install_fix` retains no-link text |
| AC6 | `cli::tests::ac6_failed_test_and_serve_name_first_failure_and_fallback`, `unit::tests::ac3_panic_classification_counts_location_and_fix`, `ac3_wasm_exit_trap_and_limit_classifications`, `contract::ac6_contract_recovery_names_file_step_and_call`; existing cli provider tests retain --dir |
| AC7 | `check::tests::ac7_own_provider_diagnostics_use_identity_and_actual_folder`, `check::ac7_deps_only_own_slug_never_suggests_importing_itself`; `check::ac7_own_package_in_different_folder_names_actual_folder` needs pkg's folder-specific own-package diagnostic |
| AC8 | `envelope::ac8_s39_recovery_contract_is_documented` checks 2.2, 9.6, 10.1 and this section; scenario behavior is unchanged, so FUNCTIONAL.md is unchanged |

### 15.3 The fake host (`src/bin/fake-watane.rs`, feature `fake-host`)

Arguments `--preview <dir>` (run a fake preview host) or `--home <dir>`
(run a fake home host with app consent). `--help` and `-h` print usage
with both arguments and their purposes, then exit 0. Wrong arguments
print usage and exit non-zero. `install::fake::ac2_fake_help_and_wrong_arguments`
proves both help flags and invalid invocations. It listens on
`127.0.0.1:0` with `tiny_http`, writes `<dir>/run.json`
(`{"watane":1,"pid","port","mode"}`), prints a ready line in the form of
`watane_host::Ready` (token `wt_fake` in a preview, `null` in a home), and
answers `GET /api/status` (`app: true` in a home), `GET /api/releases`, `POST /api/install` and
`POST /api/publish`, `POST /api/search` and `POST /api/fetch`. Search records its JSON body as
`<dir>/requests/<n>-search` and returns status 200 with `fake.json`'s `search`
value unchanged when set (any JSON body), otherwise `{"indexes":0,"results":[]}`.
Search ignores the consent `answer` setting. `POST /api/fetch` likewise ignores
consent and records its JSON body as `<dir>/requests/<n>-fetch`. `fake.json`
`"fetch"` is an object keyed by link text or ReleaseRef text. Each value is
`{"release":"<ReleaseRef>","package":"<path of a package file>"}`. A known key
answers 200 with those bytes, `Content-Type: application/octet-stream`,
`Watane-Release` from the entry and `Watane-Package` computed from the bytes.
An unknown key answers 409 `fetch_refused`. `fake.json` `"providers"`
(default `[]`) is copied into the publish answer unchanged. For publish header
assertions, the fake records `Watane-Provider-Links` in `<n>-provider-links`.
It stores each request body as
`<dir>/requests/<n>-<path>`. `<dir>/fake.json` (`{"answer": "approve" |
"decline"}`, default approve) sets the answer; decline answers 403
`{"error":{"code":"refused","message":"declined"}}`. An installed folder
gets local name = mod slug (including cached folders named by package id)
and a page URL if it has `ui/`. Publish approves
by decoding and unpacking the posted package: `release` is
`<fake key>/<slug>@<version>`, `site` is `<dir>/site`, and `link` is
`https://fake.invalid/#<slug>`. `GET /api/releases` returns `{"releases":[]}`
before any publish, then the releases published in this home (slug, version,
release, seq starting at 1, and link), retained in `releases.json` across
fake restarts. `fake.json` may preset `releases` (including an empty list to
simulate a stale app view), and `link` (including null); the link override
also applies to listed releases, modeling a site URL set after publication.
Invalid packages get 400 `invalid_package`;
it records raw package bytes and removes the temporary unpacked folder.
Status requests are not recorded; POST requests are numbered from 1, with names such as `1-install` and `2-publish`. Install uses pkg
metadata and package ids and returns the host `InstalledMod` type. It serves no
page and no MCP. SIGTERM ends it (the default action).

## 16. Build plan

- S4 (dev): the counter template includes `tests/counter.json`; `test` runs
  check, build, Wasmtime unit tests and contracts through the host Harness.
  `serve` runs that whole pipeline before touching a preview. S11 links
  the file's fakes through Harness and runs `Target::Deliver` through
  `Harness::deliver`. Only `test.uncovered` waits for `Harness::functions`.
  S8 completes DEV-S-10 outside reads and DEV-S-12 reproducible bytes; both release and unit builds scan compiler dependency files.

| Stage | Build | Needs | Fake until it exists | Done when |
|---|---|---|---|---|
| 0 | crate, `cli`, `outcome`, `env`, text and JSON output, exit statuses | — | — | S-32 for `check` outside a mod: `usage.not_a_mod`, status 2 |
| 1 | `template`, `new`, `check` (5.1 to 5.3) | pkg stages 2 and 3: `Slug`, `Mod::read`, `Violation`, `Grant`, `World` | without pkg: a local slug regex and dev's own rules; the `mod.invalid` path is a test `#[ignore = "needs watane-pkg"]` | S-1, 2, 3, 5, 6, 7, 8; the template passes `Mod::read`. S1 ships the page-only cut: `.gitignore`, `AGENTS.md`, `CLAUDE.md`, `mod.toml`, `wit/world.wit`, `ui/index.html`, `ui/app.js`; no logic, host WIT or tests; `new.next` is `Edit ui/, then run: watane-dev serve --json` |
| 2 | `allowlist`, `scripts/vendor.sh`, `toolchain`, `builder` | host: `HOST_WIT`; pkg stage 4: `check_component` | `HOST_WIT` as a local const copied from `spec/TECHNICAL.md` section 4, marked for removal; `check_component` behind one fn, with an ignored test | S-9 to 13; the template builds |
| 3 | `unit`, `test` pipeline, check stage 6, template contract file | — | — | S4-2: S-14 to 16, S-20 check; S-4 with `contract: null` |
| 4 | `contract`, `json_eq` | host stage 5: `Harness` | `run_steps` tested with closures; integration tests ignored | S-17 to 20; S-4 complete |
| 5 | `preview`, `api`, `doctor`, `fake-watane` | host stage 6: the program, `Ready`, `RunInfo`, `/api/status`, `/api/install` | `fake-watane` | S-21 to 24 on the real `watane`; J1 passes |
| 6 (step 2) | `deps`, `add`, provider traversal in `serve`; `install` checks app status first, tests providers and requests home install | pkg stage 6: `add_provider`; app running for J2 | `fake-watane` | S-25 to 29 |
| 7 (step 3) | `publish`: local-provider refusal, app status, tests, pack, consent POST, result | pkg stage 5: `pack`; app: `/api/publish` | `fake-watane` | S-30, 31 implemented; dev's part of J3 |
| 8 (step 4) | `search`: app status, POST, result, `next` (section 10) | app: `/api/search` (app 8.2) | `fake-watane` answers `/api/search` from `fake.json`, with an empty default | S-33, 34 implemented; dev stage 8 complete |
| 9 (step 5) | `build_on`: 10.1 to 10.3, `release_providers`, 9.2 released providers, `publish.provider_unlisted`, the `missing_providers` fix of 9.6; template `AGENTS.md` lines for `new --from`, `add --release` and source as data | app: `/api/fetch` (app 8.3); host stage 9 (the preview rule) | `fake-watane` answers `/api/fetch` from packages named in `fake.json` | S-35 to S-40 pass; dev's part of J5 |

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
   must reuse the running preview. Resolved: dev keeps the ready line in
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

12. **Pending dependency edits after `add`** (S11-3, dev): pkg rejects an
    unused provider until the agent pastes its import or subscription line.
    Traverse `Deps::read` before validating worlds, and check the proposed
    cycle before reading the provider as a `Mod`. This preserves the fixed
    pkg interface and reports DEV-S-26 without modifying either folder.


13. **Own package copied under a different folder (S39).** pkg currently only
    emits the own-package provider violation for `wit/deps/<own slug>`.
    Required owner behavior: report `file = wit/deps/<actual folder>` and
    `wit/deps/<actual folder> holds this mod's own package mods:<own slug>`
    for any copied own-package folder. Dev maps this shape now and fakes it
    in a unit test; the integration proof waits for pkg. S39's scope forbids
    changes outside dev, so this owner requirement is recorded here.
