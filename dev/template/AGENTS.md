# This folder

| Path | Who edits it |
|---|---|
| mod.toml | You: name and summary |
| wit/world.wit | You: contract, slug and version |
| logic/Cargo.toml | You: allowed dependencies |
| logic/src/lib.rs | You: logic |
| ui/ | You: page and scripts |
| tests/*.json | You: contract tests |
| wit/deps/, deps.json, .watane-dev.json | watane-dev; do not edit |
| logic.wasm, logic/Cargo.lock | watane-dev builds these |
| AGENTS.md, CLAUDE.md | Keep these guides |

## Order of work

Edit wit/world.wit, then logic/src/lib.rs, then ui/. Run
`watane-dev test --json` until ok is true, then run
`watane-dev serve --json` and give result.pages[0].url
to the person.

## The world

Keep `package mods:{{slug}}@<version>;` and `world {{slug}}`.
The package version is the mod version. Import only what the mod calls:
each import asks for a grant that the participant approves. An import of
an interface of your own package sends events. WIT names use kebab-case;
a keyword used as a name needs `%`.

## The logic part

Implement `exports::mods::{{crate}}::<iface>::Guest` and use `export!(Mod)`.
WIT kebab-case becomes Rust snake_case. Types live in their interface's
module. Read the host interface comments in wit/deps/watane-host/host.wit.
A panic fails the call and undoes its writes. There are no files,
environment variables or sockets. Each call gets a fresh instance;
store persistent values through watane::host::state.

## Crates

Only these direct crates and versions are available:
{{crates}}

No build script, procedural macro, .cargo/ or rust-toolchain file in the mod.
Keep `[workspace]` empty and `[lib] crate-type = ["cdylib"]`.

## The page

Use script files in ui/, with no inline script, CDN or other site.
Load `/watane.js` before app.js. `watane.call("<iface>.<fn>", args)`
returns a promise for the value or rejects with `code` and `message`.
For a call without arguments, omit args or pass null.
Records are JSON objects; option is null or the value; variants are
"case" or {"case": payload}; result is {"ok": v} or {"err": e}.
Read once at startup and refresh after your page's calls.

## Tests

Unit tests run as WebAssembly with no files, no environment and no host
imports: test pure functions there. A panic ends the whole unit test run;
tests after it are counted in not_run. Fix the failure and run test again.
Keep at least one tests/*.json file for a logic part. Each contract file
starts with fresh state. Steps call an exported function and assert its
return value or failure; emits checks the events in order. For example:

```json
{"steps": [{"call": "api.add", "returns": 1,
            "emits": [{"event": "changes.counted", "args": {"count": 1}}]}]}
```

Contract execution, provider fakes and event delivery arrive later, with
links to their guide and examples. Until then result.contract is null.

## Results

With --json every command prints one envelope: schema (1), command, ok,
dir, result, errors, warnings and next. Read errors[].fix; for
build.compile read file and line. Unknown locations are null.
Exit statuses: 0 success, 1 mod, 2 usage, 3 environment, 4 refusal,
5 internal error. Warnings do not change the exit status.

## Rules

Work autonomously; do not ask questions. Finish with a passing
`watane-dev test --json`, then `watane-dev serve --json`.
