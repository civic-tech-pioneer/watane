# host: functional specification

What `host` does, free of implementation. Ids `HOST-F-<n>`. Each
requirement traces to `spec/FUNCTIONAL.md`. The fixture mods named in the
scenarios (`counter`, `echo`, `hog`, `shelf`, `recs`, `net`) are defined in
`TECHNICAL.md` section 13.

## 1. Terms

| Term | Meaning here |
|---|---|
| home | One data directory with one running host. A home is a participant home or a preview home, fixed at creation. |
| local name | The name that a host gives an installed mod. It names the page origin and the tools. |
| identity | `<publisher>/<slug>`, or `local/<slug>` for a local mod. An update keeps it. |
| call | One call of a function of an interface that a mod exports from its own package. |
| top-level call | A call that a page, an agent or an event delivery starts. Its nested calls share its budget. |
| link | The binding of an import or a subscription of a mod to one installed provider. |
| plan | What an install would do: the mods, their grants, their providers, and what is missing. |
| app | The in-process party that answers a confirmation. A host has no app, or one app. |

## 2. Requirements

### 2.1 Step 1: Make (preview, runtime, pages, tools)

- **HOST-F-1** The host shall run each home in the mode that was fixed when the home was created: participant home or preview. Traces: F-MAKE-4, F-RUN-2.
- **HOST-F-2** If a program opens a home in a mode other than its own, then the host shall refuse to open it. Traces: F-MAKE-4.
- **HOST-F-3** If a home is held by a running host, then a second host shall refuse to open it. Traces: F-RUN-8.
- **HOST-F-4** When the program `watane` listens, it shall print exactly one JSON line on standard output, with the port and the mode, and in a preview the toolset address and its token. Traces: F-MAKE-4, F-MAKE-5, F-AGENT-2.
- **HOST-F-5** When a mod is installed in a preview, the host shall grant it exactly the grants that it declares, without a question. Traces: F-MAKE-4.
- **HOST-F-6** The host shall run each call of a logic part in a new sandboxed instance, with no directory, no environment variable, no argument and no socket. Traces: F-RUN-2.
- **HOST-F-7** If a top-level call exceeds its fuel, memory, wall-time or nested-call-depth limit, then the host shall stop it and report the limit by name. Traces: F-RUN-3.
- **HOST-F-8** The host shall keep private state for each mod, and commit the writes of a call only when that call succeeds. Traces: F-RUN-8.
- **HOST-F-9** If a mod without the storage grant reads or writes its state, then the host shall fail the call with "not granted". Traces: F-RUN-2, F-RUN-7.
- **HOST-F-10** The host shall tell a logic part who called it (`page`, `agent:<handle>` or `mod:<local name>`), from the entry point of the call and never from the request. Traces: F-RUN-5, F-AGENT-1.
- **HOST-F-11** The host shall serve the page of each mod only on the origin of that mod, with a content policy that allows only that origin. Traces: F-RUN-4.
- **HOST-F-12** When a page calls a function, the host shall accept the request only with the exact origin of that mod, and call only an export of that mod. Traces: F-RUN-4.
- **HOST-F-13** While a page listens for events, the host shall send it a "changed" message after each call that changed the state of its mod, an "event" message for each event that its own mod emits, and a "reload" message after its mod is updated. Traces: F-RUN-4, F-RUN-6, J1.
- **HOST-F-14** The host shall offer each call of a toolset as one MCP tool, named `<local name>_<interface>_<function>`. Traces: F-AGENT-1, F-AGENT-3.
- **HOST-F-15** The host shall accept an MCP request only with a live token for that toolset, and shall take the agent handle from the token. Traces: F-AGENT-1, F-AGENT-2.
- **HOST-F-16** If a request to the local API or to MCP carries an `Origin` header, then the host shall refuse it. Traces: F-RUN-4, F-AGENT-2.
- **HOST-F-17** In a participant home, the host shall issue a token only when the app asks in its process. In a preview, the program `watane` shall issue one token for the preview toolset. Traces: F-AGENT-2.
- **HOST-F-18** When a token is ended, the host shall refuse each later request with it. Traces: F-AGENT-2.
- **HOST-F-19** The host shall run one built mod alone, with a fake for each import that it calls, on the same runtime that runs installed mods. Traces: F-MAKE-3, F-MAKE-4.
- **HOST-F-20** The host shall report each failure with a fixed machine-readable code and a message that names the cause. Traces: F-MAKE-5, F-QUAL-3.
- **HOST-F-21** If a mod has no part, has more than one page or logic part, or its component imports anything outside its world, then the host shall refuse to install it. Traces: F-RUN-1, F-RUN-2.

### 2.2 Step 2: Run together (catalog, links, events, consent)

- **HOST-F-22** When a caller asks for a plan, the host shall list each mod with its publisher or "local", its grants, its providers and whether it is new or an update, and name each provider that is neither installed nor supplied. Traces: F-INST-1, J2.
- **HOST-F-23** The host shall install the providers of a mod before the mod. Traces: F-INST-4.
- **HOST-F-24** If the call links of a plan would form a cycle, then the host shall refuse the plan and name the cycle. Traces: F-RUN-5.
- **HOST-F-25** In a participant home, the host shall install a plan that adds a mod or a grant only after the app confirms it, and only with the grants that the participant approved. Traces: F-INST-2, F-AGENT-4.
- **HOST-F-26** If a participant home has no app, then the host shall refuse each install that needs a confirmation. Traces: F-AGENT-4.
- **HOST-F-27** When an update of an installed mod asks for no new grant, the host shall install it without a question, and keep its local name, its state and its granted grants. Traces: F-INST-7, J1.
- **HOST-F-28** The host shall install a mod from a built folder without a signature, and mark it as local. Traces: F-INST-3.
- **HOST-F-29** The host shall give each installed mod a local name that is unique in that home and follows the grammar of `TECHNICAL.md` 3.2. Traces: F-AGENT-3.
- **HOST-F-30** The host shall link an import or a subscription only to an installed mod of the WIT package and the publisher that `deps.json` pins, at a compatible version. Traces: F-RUN-5, F-RUN-6.
- **HOST-F-31** When a mod calls an import that is not linked or not granted, the host shall fail that call with "not granted" and still run the mod for its other calls. Traces: F-RUN-2, F-RUN-5.
- **HOST-F-32** When a call that emitted events succeeds, the host shall deliver each event as one call into each granted subscriber, in the order of emission. The emitter shall not learn its subscribers. Traces: F-RUN-6.
- **HOST-F-33** If an event chain reaches 8 hops, then the host shall drop the event and log it. Traces: F-RUN-3, F-RUN-6.
- **HOST-F-34** Where a mod has a network grant, the host shall allow its outgoing HTTPS requests only to the hosts that the grant names. Traces: F-RUN-7.
- **HOST-F-35** The participant shall be able to enable, disable and remove each mod, and withdraw and give back each grant that the mod declares. A removal deletes the state only when the participant asks. Traces: F-INST-5.
- **HOST-F-36** When a grant is withdrawn, the host shall fail the next call that needs it with "not granted". Traces: F-INST-5, J2.
- **HOST-F-37** The host shall keep the installed mods, their grants, their links and their state across a restart. Traces: J2.
- **HOST-F-38** The host shall let the app add the routes of `dist` to the local API, on the same port and under the same guards. Traces: F-PUB-2, F-FIND-3.
- **HOST-F-39** The host shall give, for each installed mod, its version, publisher, grants, links, origin release and enabled state. Traces: F-APP-5, F-BUILD-1.

### 2.3 Step 3: Publish (verify releases)

- **HOST-F-40** Before a plan with a package of another participant, the host shall check the signature of its release, that the release names the hash of this package, and that slug and version agree. If a check fails, then the host shall refuse and name the check. Traces: F-INST-6, F-QUAL-3, J3.
- **HOST-F-41** The host shall mark a mod of another participant with its publisher, and the mod of each release only once per identity. Traces: F-INST-1, F-APP-5.

### 2.4 Steps 4 and 5 (only what must not be blocked)

- **HOST-F-42** (step 4) The host shall keep the release of each installed mod, so that `dist` and the app can compare it with updates and withdrawals. Traces: F-INST-7, F-INST-8.
- **HOST-F-43** (step 5) A link shall name its provider by local name, so that a provider that the participant picks can be bound. Traces: F-BUILD-3.

### 2.5 Step 5: Build on

- **HOST-F-44** When the app plans with a bind, the host shall link that consumer to the picked mod in place of the provider that `deps.json` names, only if the picked mod has the same slug and offers every interface that the consumer needs, and shall keep the bind across updates of the consumer until the picked mod is removed. Traces: F-BUILD-3, J5.
- **HOST-F-45** In a preview, the host shall link a provider by slug and interface, whatever its publisher. Traces: F-BUILD-2, W27.

## 3. Acceptance scenarios

Each scenario names the requirements it checks. A test drives the public
API, the HTTP server or the program `watane`.

### 3.1 Step 1

- **S1 Preview start** (HOST-F-1, 4, 5, 17). Given an empty directory D.
  When `watane --preview D --install <counter>` starts, then its first
  stdout line is a JSON object with `mode` "preview", a `port`, an `mcp`
  address ending in `/mcp/preview`, a `token` that starts with `wt_`, and
  one entry in `mods` with `local` "counter".
- **S2 Mode is fixed** (HOST-F-2). Given D is a preview home. When
  `watane --home D` starts, then it prints an error line with code
  `mode_mismatch` and exits with status 1.
- **S3 One host per home** (HOST-F-3). Given a host runs on D. When a second
  host opens D, then it fails with code `busy`.
- **S4 State commits on success** (HOST-F-8). Given `counter` installed.
  When `api.add` is called twice, then `api.get` returns 2. When
  `api.add-then-fail` is called, then the call fails with code `trap`, and
  `api.get` still returns 2.
- **S5 No storage grant** (HOST-F-9). Given `counter` installed with the
  storage grant withdrawn. When `api.add` is called, then it fails with
  code `not_granted` and the message names `watane:host/state`.
- **S6 Fuel** (HOST-F-7). When `hog`'s `api.spin` is called, then it fails
  with code `limit` and limit `fuel` within 10 seconds.
- **S7 Memory** (HOST-F-7). When `hog`'s `api.grow(2000)` (2000 pages of
  64 KiB) is called, then it fails with limit `memory`.
- **S8 Wall time** (HOST-F-7). Given a host with a time limit of 200 ms and
  unlimited fuel. When `api.spin` is called, then it fails with limit
  `time` after at least 200 ms and before 2 s.
- **S9 No device access** (HOST-F-6). When `hog`'s `api.probe` lists `/`,
  reads `HOME` and opens a TCP socket, then it returns `["no-dir",
  "no-env", "no-socket"]`.
- **S10 Caller** (HOST-F-10). When `shelf`'s `items.who` is called from the
  page, from MCP with a token of handle `alice-agent`, and through `recs`'s
  `api.who-of-shelf`, then it returns `page`, `agent:alice-agent` and
  `mod:recs`.
- **S11 Page origin** (HOST-F-11, 12). When `POST /call` reaches the origin
  of `counter` with `Origin: http://echo.localhost:<port>`, then the answer
  is 403 with code `forbidden`. With the exact origin, it is 200.
- **S12 Page content policy** (HOST-F-11). When `/` of a mod is fetched,
  then the answer has the `Content-Security-Policy` of `TECHNICAL.md`
  8.3, byte for byte.
- **S13 Events stream** (HOST-F-13). Given a page listens to `/events` of
  `counter`. When `api.add` succeeds, then the stream sends
  `{"type":"changed"}`. When `api.get` succeeds, then it sends nothing
  within 500 ms. When a changed `counter` is installed again, then it sends
  `{"type":"reload"}`.
- **S14 Tools** (HOST-F-14, 15). Given toolset `t` lists `counter:api`. When
  `tools/list` is sent with a token for `t`, then the names are
  `counter_api_add`, `counter_api_add-then-fail`, `counter_api_get` and
  `counter_api_who`, in sorted order.
- **S15 Token** (HOST-F-15, 16, 18). A request to `/mcp/t` without a token,
  with a token for another toolset, with an ended token, or with an
  `Origin` header, is refused with 401, 403, 401 and 403.
- **S16 Contract harness** (HOST-F-19). Given `recs` alone, with a fake for
  `shelf/items.all` that returns `["Dune"]`. When `api.suggest` is called,
  then it returns `"Dune"` and the outcome records the event that `recs`
  emitted.
- **S17 Refused package** (HOST-F-21). When a folder with neither
  `logic.wasm` nor `ui/index.html` is planned, then the plan fails with
  code `invalid_mod`.

### 3.2 Step 2

- **S18 Plan order** (HOST-F-22, 23, 28). Given an empty participant home.
  When `recs` and `shelf` are planned together, then the plan lists `shelf`
  first, both with publisher `null` (local), and `recs` with providers
  `["shelf"]`.
- **S19 Missing provider** (HOST-F-22). When `recs` alone is planned, then
  `missing` names the local provider `shelf` at version 0.1.0, and an install fails with
  code `missing_providers`.
- **S20 Cycle** (HOST-F-24). When two mods that import each other are
  planned, then the plan fails with code `cycle` and the message names
  both.
- **S21 Confirmation** (HOST-F-25). Given a home with an app that denies.
  When `counter` is installed, then the app was asked once, the result is
  code `refused`, and `mods/` holds no `counter`. With an app that approves
  with the storage grant withheld, `counter` is installed without it.
- **S22 No app** (HOST-F-26). Given a home host with no app. When
  `POST /api/install` asks for `counter`, then the answer is 403 with code
  `refused`.
- **S23 Quiet update** (HOST-F-27). Given `counter` installed and counted
  to 2. When a changed build with the same grants is installed, then the
  app is not asked, and `api.get` returns 2.
- **S24 Calls and events** (HOST-F-30, 32). Given `shelf` and `recs`
  installed with all grants. When `shelf`'s `items.add("Dune")` succeeds,
  then `recs` has received `events.added("Dune")` exactly once, and
  `recs`'s `api.suggest` returns `"Dune"` through its call to `shelf`.
- **S25 Withdrawn grant** (HOST-F-31, 36). Given S24. When the call grant
  of `recs` on `shelf/items` is withdrawn, then `recs`'s `api.suggest` fails
  with code `not_granted`, and `recs`'s `api.ping` still succeeds.
- **S26 Hop limit** (HOST-F-33). Given two mods that each re-emit the
  event they receive. When one emits, then exactly 8 deliveries happen.
- **S27 Network** (HOST-F-34). Given `net` granted `example.org` only.
  When it requests `https://example.com/` or `http://example.org/`, then
  the request fails with `HTTP-request-denied` and no connection is made.
  A request to `https://example.org/` succeeds (a network test, not run by
  default).
- **S28 Restart** (HOST-F-37). Given S24. When the host is closed and
  opened again, then `recs`'s `api.suggest` still returns `"Dune"`.
- **S29 Remove** (HOST-F-35). When `counter` is removed with
  `delete_data = false` and installed again, then `api.get` returns the
  old count. When it is removed with `delete_data = true` and installed
  again, then `api.get` returns 0.
- **S30 Extra routes** (HOST-F-38). Given the app adds `GET /api/hello`.
  Then it answers on the API host, a request to it with an `Origin` header
  gets 403, and the same path on a mod origin gets 404.

### 3.3 Step 3

- **S31 Signed release** (HOST-F-40, 41). Given a package of `counter` and
  a release signed by key K that names its hash. When it is planned, then
  the entry has publisher K, identity `K/counter` and release
  `K/counter@0.1.0`.
- **S32 Changed byte** (HOST-F-40). When one byte of a file in that package
  changes, then the plan fails with code `verify` and check
  `package_hash`. When one byte of the release changes, then
  `Entry::decode` refuses it, so it cannot reach the host. When a release
  signed by K names the hash of this package but another summary, then the
  check is `mismatch` and the message names `summary`.

### 3.4 Step 5

- **S33 Bind a picked provider** (HOST-F-44). Given `recs` released by K2,
  whose `deps.json` pins `shelf` to K1, and `shelf` released by K3 (same
  WIT). When `plan` gets both, then `missing` names `K1/shelf`. When
  `plan_with` gets them with the bind `{consumer: "K2/recs", provider:
  "K1/shelf", to: "K3/shelf"}`, then nothing is missing, the `recs` entry
  lists that bind, and after install `recs`'s `api.who-of-shelf` returns
  `mod:recs` through K3's `shelf`. After a restart the bind and the link are
  still there.
- **S34 A bind that does not fit** (HOST-F-44). Given the same, with K3's
  `shelf` lacking the interface that `recs` calls. When `plan_with` gets the
  bind, then `missing` still names `K1/shelf`. A bind with `to` of another
  slug is `bad_request`.
- **S35 Preview by slug** (HOST-F-45). Given a preview and the folders of
  `shelf` (local) and `recs` whose `deps.json` pins `shelf` to K1. When
  both are installed, then `recs` links to `shelf`. In a participant home
  the same plan names `K1/shelf` as missing.
