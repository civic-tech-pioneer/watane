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
| manager | (step 10) A mod with the grant to manage mods. |
| request | (step 10) An addition that a manager asks for: the app decides it, and the host acts only on the answer. |
| stream | (step 11) Numbered items from one producer to one consumer, in order, with backpressure. |
| origin client | (step 11) The mod whose authority a chain carries: the mod of the page, or the owner of the agent's handle. Consents are kept per origin client. |
| consent | (step 11) A participant's answer, kept with the grants: a workspace, the account `openai`, a toolset. |
| host request | (step 11) A call whose outside effect the host applies only after the call's chain has committed durably. |

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
- **HOST-F-24** If the call links of a plan would form a cycle, then the host shall refuse the plan and name the cycle. From step 9, a collect link counts as a link from the collector to the subscriber. Traces: F-RUN-5, F-RUN-9.
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

- **HOST-F-44** When the app plans with a bind, the host shall link that consumer to the picked mod in place of the provider that `deps.json` names, only if the picked mod has the same slug and offers every interface that the consumer needs, shall ask the participant for every new bind, and shall keep the bind across updates of the consumer until the picked mod is removed, which deletes the bind. Traces: F-BUILD-3, J5.
- **HOST-F-45** In a preview, the host shall link a provider by slug and interface, whatever its publisher. Traces: F-BUILD-2, W27.

### 2.6 Step 9: Extension points

- **HOST-F-46** When a mod calls a collect, the host shall call the same function in each enabled subscriber that holds the subscription grant, in local-name order, inside the collector's call, and return their lists joined in that order. The collector shall not learn who answered. Traces: F-RUN-9, J9.
- **HOST-F-47** If an answer of a collect fails, then the host shall leave it out, drop its writes and events, log it, and go on with the next subscriber. Only a failure that ends the whole call (its time or fuel) fails the collector. Traces: F-RUN-3, F-RUN-9.
- **HOST-F-48** In a contract test, the host shall answer a collect with its fake, or with the empty list when the file gives none. Traces: F-MAKE-3, F-RUN-9.

### 2.7 Step 10: Own manager

- **HOST-F-49** When a manager asks, the host shall give it every installed mod with its version, publisher, origin, enabled state, grants, links and binds. If a mod without the grant to manage uses a function of management, then the host shall fail that call with "not granted". Traces: F-INST-9, J10.
- **HOST-F-50** A manager shall be able to disable a mod, withdraw a grant of a mod, and close the page of a mod, without a question. Traces: F-INST-10, J10.
- **HOST-F-51** When a manager asks for an install, an update, an enable, a grant or a removal, the host shall pass the request to the app, give the manager an id to follow it by, and act only after the app's answer. A manager shall have at most one open request. Traces: F-INST-11, F-AGENT-4, J10.
- **HOST-F-52** The host shall ask the app before each install or update that a manager asked for, also one that adds no grant. Traces: F-INST-11, F-AGENT-4.
- **HOST-F-53** When a manager loses the grant to manage, the host shall end its open requests as denied and fail its later functions of management. Traces: F-INST-5, F-INST-9, J10.
- **HOST-F-54** If a call of a manager fails, then the host shall disable, withdraw and ask nothing for it. Traces: F-RUN-8.
- **HOST-F-55** In a preview, or in a home with no app, the host shall refuse each request of a manager at once. The reductions still work. Traces: F-AGENT-4, F-MAKE-4.
- **HOST-F-56** In a contract test, the host shall fake the functions of management and record each reduction and request of the mod under test, in order. Traces: F-MAKE-3.

### 2.8 Step 11: The agent layer

- **HOST-F-57** The host shall number the items of each stream from 1 and deliver them to the one consumer in order, in batches of at most 256 items or 1 MiB, at least one item. Traces: F-AGENT-5, J11.
- **HOST-F-58** If a batch fails, then the host shall deliver it once more with the same numbers, and shall end the stream `consumer-failed` when the second delivery fails too. Traces: F-AGENT-5.
- **HOST-F-59** When a consumer answers `defer` to a batch, the host shall drop that call's writes, keep the batch, deliver nothing more on the stream until the consumer calls `resume`, and then deliver the same batch with the same numbers; a deferral shall not count as a failure, and a stream deferred for 5 minutes without `resume` shall end `consumer-failed`. Traces: F-AGENT-5.
- **HOST-F-60** If a stream is full (256 items or 4 MiB in flight), then a `push` shall fail `full`, and a stream that stays full for 10 s without its consumer deferring it shall end `lagging`, both sides told. Traces: F-AGENT-5.
- **HOST-F-61** The host shall refuse a `push` that would pass 128 MiB of queued items for an origin client or 512 MiB for the host with `full`, and shall end the streams of a page connection that holds more than 16 MiB `lagging`. Traces: F-AGENT-5.
- **HOST-F-62** A page shall be the consumer of a stream only through the connection id that `watane.js` sends with `/call`, naming one of its own `/events` connections; the host shall keep one lossless queue for each connection and end its streams `gone` when it closes. Traces: F-AGENT-5, W8.
- **HOST-F-63** The host shall apply the requests of a call chain (a stream open, a timer, a start, a session function) and its stream pushes only after the chain's top-level call has succeeded and committed durably, and a call that fails or defers, together with every call nested in it, shall send nothing to `claude`, push nothing, open no stream and leave no timer. Traces: F-AGENT-6.
- **HOST-F-64** The host shall keep timers for each mod in its state: ids of at most 128 bytes, at most 256 live, delays up to 30 days; a timer shall survive a restart, one due during downtime shall fire soon after start, and `after` with a live id shall replace it. Traces: F-AGENT-6.
- **HOST-F-65** The host shall tell a mod the identity of its caller (`<publisher>/<slug>`, `local/<slug>`, `page`, `agent:<handle-name>`, or `host`), taken from the transport and never from a request field. Traces: F-AGENT-7.
- **HOST-F-66** The host shall start `claude` only for a mod that holds the grant `claude-code` or `claude-code-shell`, with a launch document that parses to the closed grammar, with arguments it fixes, and with a workspace consent held; a value outside the grammar shall be refused `bad-launch` naming the field. Traces: F-AGENT-8.
- **HOST-F-67** The host shall refuse a `say` that begins with `/`, an `answer` of `allow` to a check, and a changed input for any tool but `AskUserQuestion` and `ExitPlanMode`, and shall write nothing else to `claude`'s stdin. Traces: F-AGENT-8.
- **HOST-F-68** The host shall keep the consents of workspace, account `openai` and toolset per origin client with the grants, shall tell the requester `consent-pending {what}` once while one waits, and shall end the dependent streams and close the dependent handles when one is withdrawn. Traces: F-AGENT-9, F-INST-5.
- **HOST-F-69** The host shall send a mod's HTTPS stream only to a host that the mod's `Http` grants name, never with a credential header of the mod, and without redirects; the account `openai` shall go only to the two OpenAI endpoints, and the account `judge` only to the provider it was entered for, with `POST`, to its entered origin. Traces: F-AGENT-9, W9.
- **HOST-F-70** The host shall give an agent's handle only the tools of its toolsets, with the annotations `@parallel` and `@idempotent` of the tool mod, return one stream for a live job of the same handle and key, and refuse the management functions in a toolset unless the participant named them. Traces: F-AGENT-1, F-AGENT-4.
- **HOST-F-71** In a preview, the host shall never run the participant's `claude` and shall refuse both accounts: a Claude Code start shall run only the fake program the preview was given, else end `no-binary`. Traces: W5, AGT-F-31.
- **HOST-F-72** When the app reports that an account changed (a sign-out, a judge key removed), the host shall end every live stream that used that account `gone`. Traces: F-AGENT-9.
- **HOST-F-73** The host shall run an event delivery and a collect answer with the subscriber as origin client, never the emitter or the collector, and shall check at apply and at each timer firing that the origin client is still installed and enabled with the identity it carried; removing a mod shall leave no timer carrying it, so a later install with the same identity gains no authority from one. Traces: F-RUN-6, F-AGENT-7.
- **HOST-F-74** The host shall keep the participant's credentials (the watane home's account and data files, Claude's and other tools' login files) out of reach of `claude`'s file tools and sandbox, shall refuse a file-tool `allow` rule that is bare or not relative, and shall refuse an `allow` answer to a file tool whose path or pattern reaches outside the launch's workspaces. Traces: F-AGENT-5, F-AGENT-7.
- **HOST-F-75** A toolset consent shall cover only the functions its words named; a function the toolset gains later, a function of a mod that has since gained `Manage`, and a function of a different mod installed under a removed mod's local name shall reach no agent until the participant consents again. Traces: F-AGENT-1, F-AGENT-4, F-AGENT-7.

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
  still there. A plan of the installed `recs` with a new bind and no other
  change needs confirmation. After `remove` of K3's `shelf`, `recs` has no
  bind; installing K3's `shelf` again does not bind it.
- **S34 A bind that does not fit** (HOST-F-44). Given the same, with K3's
  `shelf` lacking the interface that `recs` calls. When `plan_with` gets the
  bind, then `missing` still names `K1/shelf`. A bind with `to` of another
  slug is `bad_request`.
- **S35 Preview by slug** (HOST-F-45). Given a preview and the folders of
  `shelf` (local) and `recs` whose `deps.json` pins `shelf` to K1. When
  both are installed, then `recs` links to `shelf`. In a participant home
  the same plan names `K1/shelf` as missing.
- **S36 Collect** (HOST-F-46). Given `menu` and the subscribers `item-a`
  and `item-b`, all granted, and `item-c` installed with its subscription
  grant withheld. When `menu`'s `api.items` runs, then it returns
  `["a", "b"]`, and `item-c` was not called.
- **S37 Failed answer** (HOST-F-47). Given S36 and `item-bad`, granted,
  whose answer writes to state and then traps. When `api.items` runs, then
  it returns `["a", "b"]`, the log names `item-bad`, and `item-bad`'s state
  is unchanged. With `item-spin`, whose answer spins, the call fails with
  `limit`/`time` or `limit`/`fuel`.
- **S38 Collect cycle** (HOST-F-24). When `menu` and `item-back`, which
  answers `menu`'s collect and calls `menu`'s `api`, are planned, then the
  plan fails with code `cycle` and the message names both.
- **S39 Collect fake** (HOST-F-48). Given a harness of `menu` with no fake,
  `api.items` returns `[]`; with the fake `{"mods:menu/menu": {"ask":
  ["x"]}}`, it returns `["x"]`.

### 3.5 Step 10

The mod `manager` (TECHNICAL 13.1) passes each function of management
through its `api`. Unless a scenario says otherwise, it holds the grant to
manage, beside `counter`, `shelf` and `recs` in a participant home whose
app is a fake that records what it is asked.

- **S40 Read** (HOST-F-49). When `manager`'s `api.mods` runs, then it
  returns four entries in the order `counter`, `manager`, `recs`, `shelf`;
  `recs` has the call link to `shelf` and its grants with `granted` true.
  Given `manager` installed with the grant to manage withheld, `api.mods`
  fails with `not_granted`.
- **S41 Reduce** (HOST-F-50). When `api.disable("counter")` runs, then the
  app was told `Disable(counter)` before, and `counter`'s `api.get` fails
  with `disabled`. When `api.withdraw` runs for `recs`'s call grant, then
  `recs`'s `api.suggest` fails with `not_granted`. `api.close-page("shelf")`
  tells the app `ClosePage(shelf)` and changes no catalog. `api.disable("nope")`
  returns the failure `no_mod`. `api.open-page("shelf")` tells the app `OpenPage(shelf)` after the call succeeds, without changing the catalog.
- **S42 Ask** (HOST-F-51). Given `recs`'s call grant withdrawn, when
  `api.request-grant` runs for it, then it returns an id of 32 hex
  characters, the app got `Ask::Grant` with `by` = `manager`, and
  `api.request(id)` is `pending`; after the app answers `Approved`, it is
  `approved`, and the grant is still withdrawn, because the app acts, not
  the host. `api.request-enable("counter")` for an enabled `counter` is the
  failure `bad_request`.
- **S43 Always asked** (HOST-F-52). An update of `counter` (a copy with
  one byte of `ui/app.js` changed, so no new grant), installed with `Requester::Mod(manager)`, calls the app's
  `install` once, with that requester; installed with `Requester::App`, it
  calls none. With `manager`'s grant withdrawn, the install with
  `Requester::Mod(manager)` is `refused`.
- **S44 One at a time** (HOST-F-51). While a request is pending, a second
  `request-*` returns the failure `request_open` naming the first id; after
  the app answers, a new request succeeds.
- **S45 Grant gone** (HOST-F-53). Given a pending request, when the
  participant withdraws `manager`'s grant to manage, then the request is
  `denied`, the app's future was dropped, and `api.mods` fails with
  `not_granted`.
- **S46 A failed call** (HOST-F-54). `api.disable-then-fail("counter")`
  fails with `trap`; `counter` is still enabled and the app was told
  nothing. `api.disable-then-read("counter")` returns `true`, and after
  it `counter` is disabled.
- **S47 No app, harness** (HOST-F-55, 56). In a preview with `manager` and
  `counter`, `api.request-enable("counter")` returns the failure `refused`
  and `api.disable("counter")` disables it. In a harness of `manager`,
  `api.disable("x")` returns ok and records `disable {"local":"x"}`, and
  `api.mods` returns `[]`; with the fake `{"watane:host/manage": {"mods":
  [...]}}` it returns that list. A served `manager`'s `/events` gets
  `{"type":"catalog"}` after the app disables `counter`.

### 3.6 Step 11

The fixture `relay` (TECHNICAL 13.1) is the consumer and producer, with the
test streams of TECHNICAL 19.9 (feature `test-streams`) as the producer of
host streams: a test source, a held consumer that takes nothing, a clock the
test moves, and the list of `items` calls per stream. Unless a scenario says
otherwise `relay` holds no grant beyond storage and its call of `latch`.

- **S48 Failed batch** (HOST-F-57, 58). Given a host stream from the test
  source to `relay` in mode `trap-once`. When the source pushes `"a"` and
  `"b"`, then the stream's deliveries are `[1,2]` failed, then `[1,2]` taken;
  `received` lists `["a","b"]` once. In mode `trap`, the deliveries are
  `[1,2]` failed twice and `relay`'s `ended` line is `<s> consumer-failed`.
- **S49 Defer** (HOST-F-59). Given `relay` in mode `defer`. When the source
  pushes `"a"`, the deliveries are `[1]` deferred, `received` is `[]` and the
  source is `paused`; the source pushes `"b"` and no delivery follows. When
  `relay`'s `mode("taken")` and `resume(s)` run, the next delivery is `[1,2]`
  taken, `received` is `["a","b"]`, and the stream is not ended. Two defers
  in a row do not end it. A stream left deferred for 5 minutes (the test
  clock advanced) ends `consumer-failed`.
- **S50 Full and lag** (HOST-F-60). Given a host stream to the held consumer
  (it never takes a batch and does not defer), the source's 257th push of 1
  byte gets `full`; when the test clock passes 10 s after that push, the
  source's `ended()` is `lagging`. With `relay` in mode `defer` as consumer,
  no `lagging` comes, whatever the clock (until the 5-minute backstop).
- **S51 Budgets** (HOST-F-61). Given 33 held streams of one origin client
  `o1`, the first 32 holding 4 MiB less 1 byte each, a push of 64 bytes to
  the 33rd gets `full` (the origin client's 128 MiB, not the stream's 4 MiB);
  given 4 origin clients each holding 128 MiB less 64 bytes, a push of 4 KiB
  for a fifth origin client gets `full` (the host's 512 MiB). A page connection
  that stops reading past 16 MiB ends, and each of its streams ends `lagging`.
- **S52 Page connection** (HOST-F-62). Given `relay` served with a page. When
  `/events` opens, then its first message is `{"type":"connection","id":"<32 hex>"}`;
  a `/call` of `api.open` with that id, then `api.push(s, "x")`, gives the page
  `{"type":"stream","stream":"<s>","items":[{"seq":1,"body":"x"}]}`; a `/call`
  with no id gets `not-yours` from `open`; when the connection closes, `relay`'s
  `ended` line for the stream is `gone`.
- **S53 Rollback** (HOST-F-63). When `api.after-then-fail("t", 10)` runs, then
  it fails with `trap`, the `timers` table has no `t`, and `fired` stays `[]`
  after 100 ms. When `relay-front`'s `after-then-fail("t", 10)` runs (its
  nested `relay.after` succeeds, then `relay-front` traps), the same holds;
  when its `push-then-fail(s, "x")` runs on a stream of `relay`, the consumer
  gets no item. In a harness, the same call records no request; `api.after`
  records `timers.after {"id":"t","delay-ms":10}`.
- **S54 Timers** (HOST-F-64). When `api.after("t", 50)` runs, `fired` is `["t"]`
  after 100 ms, once. `api.after("t", 5000)` then `api.after("t", 50)` fires
  once, at 50 ms. `api.after("t", 5000)` then `api.cancel-timer("t")` fires
  never. After `after("t", 5000)`, closing the host, waiting past the due time
  and opening it again, `fired` is `["t"]` soon after open.
- **S55 Timer limits** (HOST-F-64). The 257th live id fails `too-many`; an id
  of 129 bytes fails `bad-id`; a delay of 2 592 000 001 ms fails `bad-delay`
  and 2 592 000 000 ms succeeds.
- **S56 Identity** (HOST-F-65). `api.who` of `relay`, called by a page, reports
  the identity `page`; through `relay-front`'s `who`, published by K,
  `<K>/relay-front`; by a local `relay-front`, `local/relay-front`; by an agent of handle `h1`,
  `agent:h1`; in an item call of a host stream, `host` (the state key `who/<s>` of
  `relay`, read from `state/state.redb` after `drop(host)`). A `/call` body with a field `caller` is
  400 (S12 unchanged).
- **S57 Launch** (HOST-F-66, 67). Given a mod with `claude-code` granted and a
  launch with `"mode":"auto"`, `start` fails `bad-launch {field: "mode"}`; the
  same launch with an `allow` of `Bash(python3:*)` fails `bad-launch {field:
  "allow"}`; without the grant the call fails `not_granted`. With `fake-claude`
  (dev 18), `say` of `"/compact"` fails `slash-command`; an `answer` `allow` to
  a check fails `not-allowed-here`; `fake-claude`'s recorded stdin holds no
  other line than the ones written for each accepted call.
- **S58 Consent** (HOST-F-68). Given a launch with `"profile":"coder"` and a
  workspace new for the origin client, and a fake app that holds the answer,
  the fake app's `ConsentRequest` names the origin client, `via` the provider,
  the folder and `profile` `"coder"`; the stream's first item is
  `{"type":"consent-pending","what":"workspace:<folder>"}`, once, and
  `fake-claude` is not started; after `Granted`, `initialized` follows and
  `installed.json` holds `{"kind":"workspace","folder":"<folder>"}` for the
  origin client; after `withdraw_consent`, the stream ends `gone`.
- **S59 Accounts** (HOST-F-69, 71, 72). A request with `account: "judge"` from
  another mod, by `GET`, to another origin, to an origin not in `[http]`, or
  with an `account` of `"other"`, fails `destination`, and the scripted judge
  server (dev 18) counts no request; a URL `https://JUDGE.example:443/v` for
  the entered origin `https://judge.example` is sent; with no key it ends
  `no-account`; a redirect is not followed; `account_changed(Judge)` ends a
  live judge stream `gone`. `judge_origin("https://Judge.Example:443/")` is
  `"https://judge.example"`, and with `/v1` or `?q` it fails `bad_request`. In
  a preview with `--fake-claude`, a `claude-code` start runs that program; with
  none it ends `no-binary`; `account: "openai"` ends `no-account`.
- **S60 Tools** (HOST-F-70). Two `agent-tools.call` of one handle with key
  `k1` while the job lives return one stream id; `describe` lists
  `parallel: true` for a tool whose WIT doc comment holds `@parallel`; a
  handle of a toolset that names a `manage` function lists it only when the
  consent named it.
- **S61 Subscriber is its own origin** (HOST-F-73). Given `notes` holding a
  workspace consent for folder `F` and the account consent `openai`, and a
  mod `z` with `claude-code` granted, `Http` `api.openai.com` granted, and a
  granted subscription to an event of `notes` whose export calls
  `claude-code.start` with workspace `F` and `https-stream.start` with
  `account: "openai"`. When `notes` emits the event, then the fake app's
  `ConsentRequest`s name `origin` `z` (not `notes`), `fake-claude` is not
  started before `Granted`, and with `Refused` both streams end
  `consent-refused("workspace:F")` and `consent-refused("account:openai")`.
  The same holds when `notes` collects from `z` (9.4).
- **S62 Origin gone** (HOST-F-73). Given `z`, called by `notes`, that sets
  `timers.after("t", 50)` and opens a `claude-code` start in `notes`'s
  consented folder `F`. When the participant disables `notes`, then the
  start's stream ends `gone` and `fake-claude` is not running; when `t`
  fires, its start for `F` asks a consent with `origin` `z`. When `notes` is
  removed and a local mod is installed under the local name `notes` and given
  a consent for `F`, then `z`'s fired timer still asks with `origin` `z`.
- **S63 Protected reads** (HOST-F-74). A launch with `allow`
  `Read(//<home>/app/accounts/openai.json)`, `Read(~/.claude/**)` or
  `Read(../x)` fails `bad-launch {field: "allow"}`; `Read(src/**)` passes.
  The `--settings=` file that `fake-claude` records holds the deny rules
  `Read(//<home>/app/**)` and `Read(//<HOME>/.claude/**)`. When
  `fake-claude` sends a `can_use_tool` for `Read` with `file_path`
  `<home>/app/accounts/openai.json`, or `Grep` with `path` `<HOME>/.ssh`,
  then `answer` `allow(none)` fails `not-allowed-here` and the recorded stdin
  holds no `control_response` for it; the same ask with `file_path`
  `<workspace>/a.txt` is answered.
- **S64 Lookalike bytes** (HOST-F-67, 69). `say` of `"\u{FEFF}/skill"`,
  `"\u{200B}/skill"` and `"\u{00A0}/skill"` each fail `slash-command`, and
  the recorded stdin holds no `user` line for them. An `https-stream.start`
  with the header `host`, `authorization`, `PROXY-AUTHORIZATION` or `:authority`
  fails `forbidden-header`, and the scripted server counts no request.
- **S65 Toolset grows** (HOST-F-75). Given `notes` with a granted consent
  for toolset `t` = `["tool:api"]` while `api` has `a`, and a handle of `t`.
  When `tool` is updated so `api` also has `b`, then `describe` lists only
  `tool_api_a`, `call(.., "tool_api_b", ..)` fails `unknown-tool`, and
  `/mcp/agent` `tools/list` lists only `tool_api_a`; `installed.json` holds
  `{"kind":"toolset","toolset":"t","functions":["tool_api_a"]}`. A new
  `open(["t"])` asks with `functions` `["tool_api_a","tool_api_b"]`; after
  `Granted` both are listed.
- **S66 Origin gone, same identity** (HOST-F-73). Given a local mod `notes`
  (slug `notes`, identity `local/notes`) holding a workspace consent for
  folder `F`, and a local mod `z`, called by `notes`, that sets `timers.after("t",
  60000)` whose firing starts `claude-code` in `F`. When `notes` is removed,
  then the `timers` row of `z`'s `t` holds `"origin":"local/z"`
  (not `"local/notes"`). When another folder with slug `notes` is installed
  as a local mod (identity `local/notes`, local name `notes`) and given a
  consent for `F`, and `t` fires, then the fake app's `ConsentRequest` names
  `origin` `z`, and with `Refused` the start ends `consent-refused("workspace:F")`.
- **S67 Bare file-tool allow** (HOST-F-74). A launch with `allow` `Read`,
  `Write`, `Edit`, `Glob`, `Grep` or `NotebookEdit` (no parentheses) fails
  `bad-launch {field: "allow"}`; `Read(**)` passes. When `fake-claude` sends a
  `can_use_tool` for `Glob` with no `path` and `pattern` `../../.ssh/*`, or
  `pattern` `/etc/*`, then `answer` `allow(none)` fails `not-allowed-here` and
  the recorded stdin holds no `control_response` for it; `pattern` `src/*.rs`
  is answered.
- **S68 Toolset name reused** (HOST-F-75). Given `notes` with a granted
  consent for toolset `t` = `["tool:api"]` storing `["tool_api_a"]`. When
  `tool` is removed and another mod (another slug, local name `tool`, export
  `api` with `a`) is installed, then `installed.json` holds
  `{"kind":"toolset","toolset":"t","functions":[]}`, a handle of `t` lists no
  tool in `describe`, `call(.., "tool_api_a", ..)` fails `unknown-tool`, and a
  new `open(["t"])` asks with `functions` `["tool_api_a"]`. Likewise, with the
  first `tool` installed, when the participant grants it `Manage` (no update), the stored list
  becomes `[]` and `tool_api_a` is `unknown-tool` until a new consent.
