# dev: functional specification

What `watane-dev` does, free of implementation. "dev" below is the program
`watane-dev`. Requirement ids are `DEV-F-<n>`. Each `Traces:` line cites
ids of `spec/FUNCTIONAL.md`. Each scenario `DEV-S-<n>` is an acceptance
test; `TECHNICAL.md` section 15 names its oracle.

## 1. All commands

- **DEV-F-1** When a command runs with `--json`, dev shall write exactly one JSON object on standard output, which holds the result or the errors of the command. Traces: F-MAKE-5, J1.
- **DEV-F-2** dev shall give each error a stable code, a message, the file and line where the error has one, and a fix where dev knows one. Traces: F-MAKE-5, F-QUAL-3.
- **DEV-F-3** When a command fails, dev shall exit with the status of the class of its first error: 1 for the mod, 2 for the use of the command, 3 for the environment, 4 for a refusal by the host or the participant, 5 for an internal error. Traces: F-MAKE-5.
- **DEV-F-4** dev shall hold no key, need no account, sign nothing and approve nothing. Traces: F-MAKE-6, F-AGENT-4.
- **DEV-F-5** dev shall open no network connection, except HTTP to `localhost`, on the port that a host of watane names in its own files. Traces: F-QUAL-1, F-MAKE-6.

## 2. Make (step 1)

### 2.1 Create

- **DEV-F-6** When the maker runs `new <slug>`, dev shall create a mod folder from the template: a page, a logic part, a WIT world with one interface of calls and one interface of events, unit tests, a contract test and a guide for the coding agent. Traces: F-MAKE-1, J1.
- **DEV-F-7** If the slug is not valid, or the target folder exists, then dev shall create nothing and name the cause. Traces: F-MAKE-5.
- **DEV-F-8** The template mod shall pass `test` with no change. Traces: F-MAKE-1, F-MAKE-2.

### 2.2 Check

- **DEV-F-9** When the maker runs `check`, dev shall check the mod folder against the rules of `pkg` and the constraints of the builder, and report each violation. Traces: F-MAKE-2.
- **DEV-F-10** When a check passes, dev shall report the grants that the mod asks for, its exports, its imports and its events. Traces: F-MAKE-2, F-INST-1.
- **DEV-F-11** If the logic part has a build script, a procedural macro, a cargo configuration, a toolchain file, a symbolic link, or a dependency outside the allowlist, then dev shall refuse the mod and name the item. Traces: F-MAKE-2, F-RUN-2.

### 2.3 Build

- **DEV-F-12** dev shall build the logic part with no network access and only with the crates of the allowlist that ships with dev. Traces: F-MAKE-2, F-QUAL-1.
- **DEV-F-13** If the Rust toolchain has no `wasm32-wasip2` target, then dev shall stop before it builds and print the command that adds the target. dev shall install nothing. Traces: F-MAKE-5.
- **DEV-F-14** If the compiler read a file outside the sources of the mod, the allowlist, the toolchain and the output of the build, then dev shall refuse the build and write no `logic.wasm`. Traces: F-MAKE-2.
- **DEV-F-15** The build shall see no environment variable of the maker. Traces: F-MAKE-2.
- **DEV-F-16** Two builds of the same sources, with the same toolchain and allowlist, in different folders and on different devices, shall give the same `logic.wasm` bytes. Traces: F-PUB-2, F-PUB-3.
- **DEV-F-17** If the built component imports an interface that its world does not import, then dev shall refuse it. Traces: F-RUN-2, F-RUN-5.

### 2.4 Test

- **DEV-F-18** When the maker runs `test`, dev shall check the mod, build it, run its unit tests and run its contract tests, and report every failure. Traces: F-MAKE-2, F-MAKE-3, J1.
- **DEV-F-19** dev shall run the unit tests of the logic part as WebAssembly, with no file, no environment variable, no network, and a limit on time and memory. Traces: F-MAKE-2, F-RUN-2.
- **DEV-F-20** dev shall run each contract test file in a new host that holds only the mod under test, with fresh state, on the same runtime as an installed mod. Traces: F-MAKE-3, F-MAKE-4.
- **DEV-F-21** In a contract test, the fake of its file shall answer each call to an interface of another package. A call with no fake shall fail. Traces: F-MAKE-3.
- **DEV-F-22** A contract step shall pass only when the result, or the failure, and the events where the step names them, equal what the step expects. Traces: F-MAKE-3, F-RUN-6.
- **DEV-F-23** If a mod has a logic part and no contract test file, then `test` shall fail. Traces: F-MAKE-3.
- **DEV-F-24** If an exported function is called by no contract step, then dev shall warn. Traces: F-MAKE-3.

### 2.5 Preview

- **DEV-F-25** When the maker runs `serve`, dev shall test the mod and its local providers, start the program `watane` on a preview home of its own, install them there, and report the page address of each mod, the address of the toolset and its token. Traces: F-MAKE-4, F-AGENT-1, F-AGENT-2, J1.
- **DEV-F-26** The preview shall give each mod exactly the grants that the mod declares, and shall hold no identity and no data of the participant. Traces: F-MAKE-4.
- **DEV-F-27** When the maker runs `serve` while a preview of the same folder runs, dev shall install the new revision in that preview and keep the data of the preview. Traces: J1.
- **DEV-F-28** When the maker runs `serve --stop`, dev shall stop the preview and delete its home. Traces: F-MAKE-4.
- **DEV-F-29** If the program `watane` is missing or the preview does not start, then dev shall report the cause, with the end of the preview log. Traces: F-MAKE-5, F-QUAL-3.

### 2.6 Environment

- **DEV-F-30** When the maker runs `doctor`, dev shall report the toolchain, the target, the allowlist, the program `watane` and the home host, and print a command for each gap. Traces: F-MAKE-5.

## 3. Run together (step 2)

- **DEV-F-31** When the maker runs `add <mod folder>`, dev shall copy the WIT of that provider into the mod, record the provider in `deps.json` and its folder in a file of dev, and report the import and subscription lines that the mod can use. dev shall not change `wit/world.wit`. Traces: F-RUN-5, F-RUN-6, J2.
- **DEV-F-32** If the provider is the mod itself, or the dependency makes a cycle, then dev shall refuse it. Traces: F-INST-4.
- **DEV-F-33** When the maker runs `install`, dev shall test the mod and its local providers, ask the running home host to install them, wait for the answer of the participant, and report it. Traces: F-INST-1, F-INST-2, F-INST-3, F-INST-4, F-AGENT-4, J2.
- **DEV-F-34** If no home host runs, then `install` and `publish` shall fail and tell the maker to start the app. Traces: F-AGENT-4, F-QUAL-3.

## 4. Publish (step 3)

- **DEV-F-35** When the maker runs `publish`, dev shall test the mod, pack it with its source unless the maker gives `--no-source`, send it to the running home host, and report the release or the refusal. Traces: F-PUB-2, F-PUB-4, F-AGENT-4, J3.
- **DEV-F-36** If the mod depends on a provider with no release, then `publish` shall fail before it sends anything, and name the provider. Traces: F-PUB-2.

## 5. Find and build on (steps 4 and 5)

- **DEV-F-37** (step 4) When the maker runs `search` with words, a publisher or an interface, dev shall ask the running app of the home and report its verified results, with the indexes that list each one and its freshness marks. dev shall never install a result. Traces: F-FIND-3, F-FIND-4, F-AGENT-4.
- **DEV-F-38** (step 5) When the maker runs `new [<slug>] --from <link>`, dev shall get the release through the running app, make a mod folder from its source with dev's own agent instructions, record the release as its origin, set the slug in the package line, and report each line that still names the old slug. Traces: F-BUILD-1, J5.
- **DEV-F-39** (step 5) When the maker runs `add --release <link>`, dev shall get the release through the running app, add a dependency on its interface as for a local provider, with its publisher, and keep the packages of it and its released providers for previews. Traces: F-BUILD-2, J5.
- **DEV-F-40** (step 5) If the release of `new --from` holds no source, then dev shall refuse it and write nothing. Traces: F-BUILD-1, F-PUB-4.
- **DEV-F-41** (step 5) When a mod depends on a published provider, `test` shall need no network, and `serve` shall install the kept package of that provider in the preview. Traces: F-BUILD-2, F-MAKE-4, J5.
- **DEV-F-42** (step 5) When `publish` succeeds and the app could not list a provider of another publisher in the own index, dev shall warn and say why it matters. Traces: F-BUILD-2, W25.

## 6. Acceptance scenarios

| Id | Given | When | Then |
|---|---|---|---|
| DEV-S-1 | an empty folder | `new reading-list --json` | exit 0; the folder holds exactly the template file list; `wit/world.wit` starts with `package mods:reading-list@0.1.0;`; `result.slug` is `reading-list`. |
| DEV-S-2 | a folder `reading-list` exists | `new reading-list --json` | exit 2; error `usage.exists`; the folder is unchanged. |
| DEV-S-3 | `new Reading_List` | — | exit 2; error `usage.slug`; nothing is created. |
| DEV-S-4 | a new template mod | `test --json` | exit 0; unit `passed` equals the number of `#[test]` functions in the template; contract `passed` equals the number of steps in `tests/*.json`. |
| DEV-S-5 | the template with `logic/build.rs` added | `check --json` | exit 1; error `mod.logic_file` names `logic/build.rs`. |
| DEV-S-6 | the template with `rand = "0.9"` in `[dependencies]` | `check --json` | exit 1; error `mod.dependency` names `rand`; the fix lists the allowed crates. |
| DEV-S-7 | the template with `[build-dependencies]`, or `[lib] proc-macro = true` | `check --json` | exit 1; error `mod.cargo_key` names the key. |
| DEV-S-8 | the template with a symbolic link in `logic/src` | `check --json` | exit 1; error `mod.symlink`. |
| DEV-S-9 | a toolchain whose sysroot has no `wasm32-wasip2` | `test --json` | exit 3; error `env.no_wasm_target`; the fix is `rustup target add wasm32-wasip2`; cargo did not run. |
| DEV-S-10 | `lib.rs` with `include_bytes!` of a file outside the mod | `test --json` | exit 1; error `build.reads_outside` names the file; no `logic.wasm`; the bytes of the file are in no file of the mod folder. |
| DEV-S-11 | `lib.rs` uses `env!("SECRET")` and the shell sets `SECRET` | `test --json` | exit 1; error `build.compile`. |
| DEV-S-12 | one mod folder at two paths, two caches | `test` in each | equal `logic.wasm` bytes; neither holds the home path, the cache path or the mod path. |
| DEV-S-13 | the build runs with network access denied to the process | `test --json` | exit 0. |
| DEV-S-14 | a unit test that reads `/`, an environment variable and writes a file | `test --json` | the reads fail inside the test; no file is written on the device. |
| DEV-S-15 | a unit test `assert_eq!(1, 2)` named `fails_on_purpose` | `test --json` | exit 1; error `test.unit` names `fails_on_purpose`. |
| DEV-S-16 | a unit test with an endless loop | `test --json` | exit 1 within the limit plus 5 s; error `test.unit_limit`. |
| DEV-S-17 | a contract step `{"call":"api.add","returns":2}` on a fresh counter | `test --json` | exit 1; error `test.contract` with file, step 1, call `api.add`, expected 2, got 1. |
| DEV-S-18 | a mod that imports `acme:books/list` and a test file with a fake for it | `test --json` | the mod receives the fake answer; with no fake, the step fails with "no fake". |
| DEV-S-19 | a step that names `emits` | `test --json` | the step passes only for the same events in the same order. A `deliver` step reaches a subscription export. |
| DEV-S-20 | a mod with a logic part and no `tests/*.json` | `test --json` | exit 1; error `mod.no_contract_tests`. |
| DEV-S-21 | a tested template mod | `serve --json` | exit 0; a GET of `result.pages[0].url` returns the bytes of `ui/index.html`; an MCP `tools/list` with `result.mcp.token` lists the calls of the mod. |
| DEV-S-22 | a running preview where `api.add` was called once | change `ui/index.html`, `serve --json` again | the page returns the new bytes; `api.get` returns 1; `result.reused` is true. |
| DEV-S-23 | a running preview | `serve --stop --json` | exit 0; the API address refuses connections; the preview home is gone. |
| DEV-S-24 | `WATANE_BIN` names a missing file | `serve --json` | exit 3; error `env.watane_missing`. |
| DEV-S-25 | two template mods `a` and `b` | in `b`: `add ../a --json` | `wit/deps/a/world.wit` equals `a/wit/world.wit`; `deps.json` is `{"mods:a": null}`; `result.imports` holds `import mods:a/api@0.1.0;`; `result.subscriptions` holds `export mods:a/changes@0.1.0;`. |
| DEV-S-26 | `a` depends on `b` | in `b`: `add ../a --json` | exit 1; error `mod.dependency_cycle`. |
| DEV-S-27 | no home host runs | `install --json` | exit 3; error `env.host_not_running`; the fix says to start the app. |
| DEV-S-28 | a home host that the participant answers with "no" | `install --json` | exit 4; error `host.declined`. |
| DEV-S-29 | `b` depends on local `a`, a home host that approves | in `b`: `install --json` | exit 0; the host received the folders of `a` and `b`, `a` first; `result.installed` lists both. |
| DEV-S-30 | a mod with a local provider | `publish --json` | exit 1; error `publish.local_dependency` names the provider; the host received no request. |
| DEV-S-31 | a mod with no local provider, a home host that approves | `publish --no-source --json` | exit 0; `result.release` is set; the sent package holds no `logic/src/`. |
| DEV-S-32 | any command | `--json` | standard output parses as one JSON object with the fields of the envelope. |
| DEV-S-33 | a home app whose search answers one result | `search reading --json` | exit 0; `result.results` equals the app's results; the app received `{"text":"reading","publisher":null,"interface":null}`; `next` is null. |
| DEV-S-34 | a home app with no chosen index | `search reading --json` | exit 0; `result.results` is empty; `next` says to ask the person to add an index in the app. |
| DEV-S-35 | a home app whose `/api/fetch` answers a package of `reading-list` with source | `new ratings --from <link> --json` | exit 0; the folder holds the package files plus dev's `AGENTS.md`, `CLAUDE.md`, `.gitignore`; `mod.toml` has `[origin] release` = the release; `wit/world.wit` starts with `package mods:ratings@<the release's version>;`; `result.mentions` names the line of `logic/src/lib.rs` with `reading_list`. |
| DEV-S-36 | the same with a package with no source | `new ratings --from <link> --json` | exit 2; error `usage.no_source`; no folder. |
| DEV-S-37 | a folder `ratings` exists | `new ratings --from <link> --json` | exit 2; error `usage.exists`; the app received no fetch request. |
| DEV-S-38 | a template mod `stats`, a home app whose `/api/fetch` answers Carol's `ratings` | in `stats`: `add --release <link> --json` | exit 0; `deps.json` is `{"mods:ratings": "<Carol key>"}`; `wit/deps/ratings/` equals the package's `wit/` files; `.watane-dev.json` names the package in `release_providers`; `result.imports` holds `import mods:ratings/api@0.1.0;`. |
| DEV-S-39 | DEV-S-38 with the import line pasted and a contract test that fakes `ratings` | `test --json` with no app running | exit 0. Then `serve --json`: the preview installed `ratings` and `stats`, `ratings` first. |
| DEV-S-40 | DEV-S-39, an app whose publish answer has `providers: [{"provider": "<Carol key>/ratings", "listed": false}]` | `publish --json` | exit 0; warning `publish.provider_unlisted` names `<Carol key>/ratings`. |
