# dev: functional specification

What `watane-dev` does, free of implementation. "dev" below is the program
`watane-dev`. Requirement ids are `DEV-F-<n>`. Each `Traces:` line cites
ids of `spec/FUNCTIONAL.md`. Each scenario `DEV-S-<n>` is an acceptance
test; `TECHNICAL.md` section 15 names its oracle.

## 1. All commands

- **DEV-F-1** When a command runs with `--json`, dev shall write exactly one JSON object on standard output, which holds the result or the errors of the command. Traces: F-MAKE-5, J1.
- **DEV-F-2** dev shall give each error a stable code, a message, the file and line where the error has one, and a fix where dev knows one. Traces: F-MAKE-5, F-QUAL-3.
- **DEV-F-3** When a command fails, dev shall exit with the status of the class of its first error: 1 for the mod, 2 for the use of the command, 3 for the environment, 4 for a refusal by the host or the participant, 5 for an internal error. Traces: F-MAKE-5.
- **DEV-F-4** dev shall hold no key, need no account, sign nothing and approve nothing. `publish --push` (step 6) uses the maker's own login of the GitHub CLI; dev holds no token of it. Traces: F-MAKE-6, F-AGENT-4, F-PUB-8.
- **DEV-F-5** dev shall open no network connection, except HTTP to `localhost`, on the port that a host of watane names in its own files. In step 6, the maker's own `git` and `gh`, which dev runs, connect to the maker's code host. Traces: F-QUAL-1, F-MAKE-6, F-PUB-8.

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
- **DEV-F-39** (step 5) When the maker runs `add --release <link or ReleaseRef>`, dev shall get the release through the running app, add a dependency on its interface as for a local provider, with its publisher, and keep the packages of it and its released providers for previews. Traces: F-BUILD-2, J5.
- **DEV-F-40** (step 5) If the release of `new --from` holds no source, then dev shall refuse it and write nothing. Traces: F-BUILD-1, F-PUB-4.
- **DEV-F-41** (step 5) When a mod depends on a published provider, `test` shall need no network, and `serve` shall install the kept package of that provider in the preview. Traces: F-BUILD-2, F-MAKE-4, J5.
- **DEV-F-42** (step 5) When `publish` succeeds and the app could not list a provider of another publisher in the own index, dev shall warn and say why it matters. Traces: F-BUILD-2, W25.

## 5a. Share on GitHub (step 6)

- **DEV-F-43** When the maker runs `publish --push`, dev shall publish as `publish` does, then push the site folder to the maker's site repository and the source of the package to the mod's own repository, tagged with the release (`<key>/<slug>@<version>`), with the maker's own `git` and `gh`. `--owner <account>` selects the GitHub account or organisation for repositories dev creates; by default it uses the gh login. The first time, dev shall create each repository on GitHub and name the mod repository in `mod.toml` before it packs. For a mod made with `new --from` from a repository, the mod repository shall be a GitHub fork of that repository where GitHub allows it, and otherwise a new repository with a warning. Traces: F-PUB-4, F-PUB-8, W29, W30, J3.
- **DEV-F-44** dev shall commit to the mod repository only the files of the package that the participant agreed to publish, shall never force a push, and shall never push to a repository that holds history it did not push for this mod. A derived mod's repository also carries the public history of its origin's repository up to the tag. Traces: F-PUB-4, F-PUB-8, W29.
- **DEV-F-45** If `git`, `gh` or the login of `gh` is missing, then `publish --push` shall fail before it tests, changes or sends anything, and say what to install or what the person must run. Traces: F-MAKE-5, F-PUB-8.
- **DEV-F-46** If a push fails, then dev shall report the release and the parts that were pushed, and give the commands that finish the push without a new publish. Traces: F-QUAL-3, F-PUB-8.
- **DEV-F-47** When the release of `new --from` names a repository, dev shall clone it at the tag named by the release (`<key>/<slug>@<version>`) and use the clone only when its source equals the source of the verified package, keeping no file of it that the package does not hold. Otherwise dev shall make the mod from the package and warn with the cause. Traces: F-BUILD-1, W24, W29, J5.

## 5b. Extension points (step 9)

- **DEV-F-48** A contract test shall fake a collect of the mod under test with the whole list of answers, and use the empty list when the file gives no fake. A `deliver` step to a collect that the mod answers shall check its answer. Traces: F-MAKE-3, F-RUN-9, J9.

## 5c. Own manager (step 10)

- **DEV-F-49** A contract test shall fake the management interface of the host, and check, in order, each reduction and request that a call of the mod under test makes. Traces: F-MAKE-3, F-INST-9, F-INST-11, J10.

## 5d. The agent layer fakes (step 11)

- **DEV-F-50** dev shall provide a fake `claude`, a program that the host starts in place of `claude`, that speaks the stream-json and control protocol of Claude Code from a script of the maker or the test: its `initialize` answer and `init` line, streamed text, finished messages, permission asks, tool checks and their failures, an exit or a crash. It shall refuse a flag that the host never passes and record everything the host writes to it. Traces: F-MAKE-3, F-MAKE-4, F-AGENT-5, AGT-F-31.
- **DEV-F-51** dev shall provide a scripted server for tests that plays an OpenAI Responses endpoint from a script (text, usage limits, retryable errors, tool calls, models), checks and records every request, and also serves a judge origin that counts every request it receives and answers safe, unsafe or unsure, is down, slow or unreadable, or answers a redirect. It shall listen on the loopback only. Traces: F-MAKE-3, F-AGENT-6, F-AGENT-9, AGT-F-31.
- **DEV-F-52** When the maker runs `serve` on a mod that starts Claude Code, dev shall offer the fake `claude` to the preview in place of the real grants and the participant's accounts, and report when the fake is missing. Traces: F-MAKE-4, AGT-F-31.

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
| DEV-S-41 | (step 6) a page-only mod with no `repository`, `gh` logged in as `alice`, an approving app | `publish --push --json` | exit 0; `alice/watane-channel` and `alice/reading-list` were created; `mod.toml` and the posted package's `mod.toml` hold `repository = "https://github.com/alice/reading-list"`; the remote's `main` of the site repository holds the site folder's files; the tag `<Alice key>/reading-list@0.1.0` of the mod repository holds exactly the package's files; `result.push.site.raw` is `https://raw.githubusercontent.com/alice/watane-channel/main/`. |
| DEV-S-42 | DEV-S-41, then the version raised to 0.2.0 | `publish --push --json` | exit 0; no repository was created (`created: false`); both remotes fast-forwarded; the mod repository has the tags `<key>/reading-list@0.1.0` and `<key>/reading-list@0.2.0`. |
| DEV-S-43 | no `git` on `PATH`; or no `gh` | `publish --push --json` | exit 3; `env.git_missing` or `env.gh_missing`; the app received no publish request; `mod.toml` is unchanged. |
| DEV-S-44 | `gh` not logged in; or git with no user name and email | `publish --push --json` | exit 3; `env.gh_auth`, whose fix asks the person to run `gh auth login`; or `env.git_identity`; the app received no request. |
| DEV-S-45 | DEV-S-41 with a mod remote that refuses every push | `publish --push --json` | exit 4; `publish.push_failed` names the push and its fix is `git -C '<folder>' ... push ...`; `result.release` is set; the site was pushed; the remote has no tag. |
| DEV-S-46 | `publish --push --no-source --json` | — | exit 0; no mod repository was created; `mod.toml` is unchanged; `result.push.mod` is null; the site was pushed. With a folder whose git has an `origin` and no `repository`, plain `--push` exits 1 with `publish.origin_differs`; with no `repository` and a non-empty `alice/reading-list` that dev did not push, it exits 1 with `publish.repository_exists`; in both, the app was not asked, nothing was created and `mod.toml` is unchanged. |
| DEV-S-47 | DEV-S-41's release, and its remote with that release's tag plus a `README.md` | `new ratings --from <link> --json` | exit 0; `result.fork.tag` is the release; `ratings` is a git repository whose `HEAD` is the tag's commit on branch `main`, with remote `upstream`; no `README.md`; files and `mod.toml` as DEV-S-35, with no `repository`. |
| DEV-S-48 | DEV-S-47 with the tag moved to a commit that changes `ui/index.html` | `new ratings --from <link> --json` | exit 0; warning `new.fork_differs` names `ui/index.html`; no `.git`; the files equal those of DEV-S-35. |
| DEV-S-49 | DEV-S-47 with the remote deleted | `new ratings --from <link> --json` | exit 0; warning `new.fork_unavailable`; no `.git`; the files equal those of DEV-S-35. |
| DEV-S-50 | DEV-S-47's `ratings` with its import fixed, `gh` logged in as `carol`, an approving app | `publish --push --json` | exit 0; `result.push.mod.forked` is true; `remotes/carol/ratings` is a fork of `alice/reading-list` whose default branch is `watane`, holding Alice's tagged commit and Carol's commit with the tag `<Carol key>/ratings@0.1.0`; its `main` equals `alice/reading-list`'s `main`; no warning. A rerun after a declined first run uses that fork and still sets the default branch. |
| DEV-S-51 | DEV-S-50 where `carol` already has a fork of `alice/reading-list` under another name; or a derived mod made from the package (no `upstream`) | `publish --push --json` | exit 0; warning `publish.fork_unavailable`; `remotes/carol/ratings` was created (not a fork); the existing fork is unchanged; with the clone, its `main` holds Alice's tagged commit and Carol's commit. |
| DEV-S-52 | a mod `menu` whose world imports its own `menu { ask: func() -> list<string>; }` and exports `api { items: func() -> list<string>; }` returning `ask()`; and a mod `item` that exports `mods:menu/menu` with `ask` returning `["a"]` | `test --json` with `{"steps": [{"call": "api.items", "returns": []}]}` and then `{"fakes": {"mods:menu/menu": {"ask": ["x", "y"]}}, "steps": [{"call": "api.items", "returns": ["x", "y"]}]}` in `menu`; `{"steps": [{"deliver": "menu.ask", "returns": ["a"]}]}` in `item` | exit 0 for both mods; each step passes. |
| DEV-S-53 | (step 10) host's fixture `manager` (it imports `watane:host/manage` and passes each function through its `api`); W, a `mod-info` (host 6.4) of `weather` with every field written out | `test --json` with `{"fakes": {"watane:host/manage": {"mods": [W]}}, "steps": [{"call": "api.mods", "returns": [W]}, {"call": "api.disable", "args": {"local": "weather"}, "returns": {"ok": null}, "manages": [{"fn": "disable", "args": {"local": "weather"}}]}, {"call": "api.request-remove", "args": {"local": "notes", "delete-data": true}, "returns": {"ok": "r1"}, "manages": [{"fn": "request-remove", "args": {"local": "notes", "delete-data": true}}]}]}`; then a second file whose only step is that `api.disable` step with `"manages": []` | exit 0 for the first file; the second fails with `test.contract` at step 1 (`api.disable`), naming the expected and the made `manages`. |
| DEV-S-54 | (step 11) `fake-claude` with the script `{"steps":[{"do":"delta","text":"hi"},{"do":"message","text":"hi"},{"do":"idle"}]}` beside it | started with the fixed arguments of host 19.3 and sent a valid `initialize` | it writes the `initialize` answer, the `system/init` line, the stream events of `hi`, the finished message and the idle line, in this order; the log holds the arguments, the environment and the one stdin line. |
| DEV-S-55 | `fake-claude` | started with `--safe-mode` or without `--restricted` | exit 64; standard error names the argument. |
| DEV-S-56 | `fake-claude` with an `ask` step with `await: true`, then `expect-user` | the test writes a `control_response` allow, then `{"type":"apply_flag_settings"}` | the first is logged; the second exits 70 and is logged. |
| DEV-S-57 | `fake-claude` with a `check` step with `timeout: true`, and with `crash` | each is run | the first writes no more for that check; the second ends by `SIGKILL` with nothing flushed. |
| DEV-S-58 | `ScriptedResponses` with the script `[Sse([Delta("a"), Completed{3,1}])]` | an https client that trusts `ca_pem()` posts to `/v1/responses` | the answer is `text/event-stream` with the two events as lines; `requests()` holds the post whole. |
| DEV-S-59 | the same with `[UsageLimit{3600}, Status{500}, Sse([Cut])]` | three posts | the first is 429 with the usage-limit body, the second 500, the third ends with no `response.completed`; a fourth repeats the third. |
| DEV-S-60 | the same with `models(["gpt-x"])` | `GET /v1/models`, then `GET /v1/other` | the first lists `gpt-x`; the second is 404 and recorded. |
| DEV-S-61 | the same | a client that trusts only the system roots connects | the TLS handshake is refused; the server's address is `127.0.0.1`. |
| DEV-S-62 | the judge with `[Safe(["ok"]), Unsafe(["bad"]), Unsure([]), Down, Unreadable, Redirect("https://127.0.0.1/x")]` | six posts with the right key | the bodies, the reset, the unreadable 200 and the 307 are as scripted; `count()` is 6. |
| DEV-S-63 | the judge with `expect_key("k")` | a post with a wrong key, then a post that is never sent | the first is 401 and counted, so `count()` is 1; nothing else was received. |
