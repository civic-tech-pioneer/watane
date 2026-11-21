# app: functional specification

What the app does, free of implementation. "The app" is the desktop
application of FUNCTIONAL.md section 1. Ids `APP-F-<n>`. Each `Traces:`
line cites spec/FUNCTIONAL.md. Step 1 needs nothing from the app (J1 uses a
preview). Steps 4 and 5 are sketched only, so that no design blocks them.
The main mod and the manager (stage I, W36) are designed and not built.

## Step 2: Run together

### Start and library

- **APP-F-1** When the app starts, the app shall open the home of the participant, run its host in the app process, and serve the host on the loopback interface. Traces: F-APP-1, J2.
- **APP-F-2** If the app cannot open the home, then the app shall show the cause, run no mod, and accept no request. A busy loopback port keeps the main window open with `Port P is in use, maybe by another watane app. Start this one with --port <n> or WATANE_PORT=<n> to use another port, or --port 0 for any free port.` The same line goes to stderr once. Traces: F-QUAL-3.
- **APP-F-3** The manager shall show a library with one entry for each installed mod: its name, its publisher or "local", its version, and whether it is off, has an update, has a problem or is the main mod. When no mod is installed, the library shall show no entry and offer the three ways to add one: a link, an index and a folder. Traces: F-APP-1, J2.
- **APP-F-4** The app shall show the consent mark of the home in the header of each app view. Traces: F-APP-4.

### Mod views

- **APP-F-5** When the participant opens a mod that has a page, the app shall show that page in a window of its own. The window shall load the origin that the host serves for that mod. Traces: F-APP-3, F-RUN-4.
- **APP-F-6** The app shall title each mod window with the name of the mod and its publisher, or "local". The page shall not change this title. Traces: F-APP-3.
- **APP-F-7** When the participant opens a mod whose window is open, the app shall focus that window and open no second one. Traces: F-APP-3.
- **APP-F-8** A mod window shall reach no app command and no plugin command. Traces: F-RUN-4, F-APP-4.
- **APP-F-9** If a mod page tries to navigate to another origin, open a new window, or start a download, then the app shall refuse it. Traces: F-RUN-4.
- **APP-F-10** Each mod window shall start with empty web storage and keep none after it closes. Traces: F-RUN-4, F-INST-5.
- **APP-F-11** When the participant disables or removes a mod, the app shall close the window of that mod before it asks the host. Traces: F-INST-5, J2.

### Consent

- **APP-F-12** When the host or dist asks for a confirmation, the app shall show the request in the consent view. The app shall answer "approve" only when the participant approves in that view. Traces: F-APP-4, F-AGENT-4, F-INST-2.
- **APP-F-13** The app shall answer confirmations inside its process. No request from outside the process shall answer one. Traces: F-AGENT-4.
- **APP-F-14** The consent view shall stay above every mod window, on every desktop space, and take the focus when it opens. Traces: F-APP-4.
- **APP-F-15** The consent view shall show the consent mark of the home. Traces: F-APP-4.
- **APP-F-16** The approve control shall accept no input until 800 ms after the consent view last got the focus. The approve control shall not be the default control. Traces: F-APP-4.
- **APP-F-17** The app shall show one request at a time, in the order of arrival.
- **APP-F-18** If the participant closes the consent view, or the app quits, or a request has no answer for 10 minutes, then the app shall answer "deny" to that request. Traces: F-AGENT-4, W19.
- **APP-F-19** For an install plan, the consent view shall show each mod in the order of install, with its name, version, publisher (with "signature verified") or "local", the version it replaces if any, each grant in words with each new grant marked, and the provider mods it uses. The heading is `Install <name>` for one new mod, `Update <name> to <version>` for one replacement, or `Install <n> mods` for several. Install origins start with the mod name (else slug), version and publisher label, followed by the full release reference after ` — `. The participant shall be able to withhold each grant; the app shall then approve only the other grants. Traces: F-INST-1, F-INST-2, F-INST-3, F-INST-4, F-INST-7, J2.

### Manage mods

- **APP-F-20** When the participant chooses one or more mod folders, or drops them on the manager, the app shall ask the host for a plan of all of them and install the plan after consent. Traces: F-INST-1, F-INST-2, F-INST-4, J2.
- **APP-F-21** If a plan needs a provider that is not installed and not supplied, then the app shall name each missing provider and install nothing. Traces: F-INST-4, F-QUAL-3.
- **APP-F-22** The app shall show, for each mod, its name and installed version next to each other outside Details, publisher or "local", grants in words, page origin and package id. Traces: F-APP-5.
- **APP-F-23** The participant shall be able to open, disable, enable and remove each mod, withdraw each grant, and give back a withdrawn or withheld grant. A page whose call is not granted shows `not granted` with the host's message. Traces: F-INST-5, J2.
- **APP-F-24** When the participant removes a mod, the app shall ask whether to delete its data, with "keep data" as the default. Traces: F-INST-5.

### Main mod and manager (stage I)

- **APP-F-62** At start, the app shall open the window of the main mod when one is set and enabled, and otherwise the main window with the manager. Traces: F-APP-1, F-APP-7, J10.
- **APP-F-63** The participant shall be able to make any installed mod with a page the main mod, and to unset it, in the details of that mod. When no main mod is set and a mod that proposes itself as main is installed new, whether the participant, a local program or a managing mod asked for the install, the app shall make it the main mod, and the consent view for that install shall say that it becomes the start window: `<name> opens when watane starts, in place of the manager.` An update shall never make a mod main. The library shall mark the main mod with `Opens at start`. When the main mod is removed, no mod is main. Traces: F-APP-7.
- **APP-F-64** The app menu shall hold `Mods…` (⇧⌘M on macOS, Ctrl+Shift+M elsewhere), which opens or focuses the main window on the manager, and `Settings…` (⌘, on macOS, Ctrl+, elsewhere), which opens it on the settings. The usual edit commands (cut, copy, paste, undo, select all) shall work in every field and page. The app shall quit when its last window other than the consent view closes. Traces: F-APP-7.
- **APP-F-65** The manager shall hold the views Library, Find and Updates, and the details of one mod. The views of the identity, the site, the backup, the agents and the own machines shall be in Settings, not in the manager. Followed channels and indexes shall be in Find. Traces: F-APP-8.
- **APP-F-66** The details of a mod shall show its connections, each a link to that mod's details: the mods it calls (`Uses`), the mods whose events or collects it receives (`Adds to`), the mods that call it (`Used by`) and the mods that receive its events or collects (`Extended by`). Following a connection link shall open the target details and move keyboard focus to its first control or heading. Only granted links of installed mods count. A mod with none shall read `Works on its own: it uses no mod and no mod uses it.` Traces: F-APP-9, J9.
- **APP-F-67** When the participant disables or removes a mod that an enabled mod calls or receives events from, the app shall first ask `Disable <name>?` or `Remove <name>?`, naming enabled callers with `These mods use it: <names>.` and enabled subscribers with `These mods extend it: <names>.` Only granted links count. Empty groups are omitted; when both apply, uses comes first, and a mod in both groups appears in both. A mod that only extends it shall not be called a user. Disabling or removing the main mod shall also ask, and say what then opens at start. The button that confirms a removal that deletes the mod's data shall say so; `Cancel` is the default and Escape cancels. Traces: F-APP-9, F-INST-5.
- **APP-F-68** The library shall list the main mod first, then the mods with a problem, then those with an update, then the rest by name. It shall filter its entries by words in the name or the publisher, and by published or local. When nothing matches, it shall say so and link to Find. Traces: F-APP-8.
- **APP-F-69** Updates shall list, in this order: the offers that need the participant (a new grant, a withdrawal, a compromised channel), the channels that could not be read, and the updates that installed without a question since the app started, newest first. Traces: F-INST-7, F-INST-8, F-APP-8.
- **APP-F-76** The app views shall follow the light or dark appearance of the system. Traces: F-APP-8.
- **APP-F-77** The details of a mod shall show each grant in plain words, naming each provider by its mod name, with a switch that takes effect at once; the consent view shall keep the exact words of every grant. Traces: F-APP-5, F-INST-5.
- **APP-F-78** When an install that the participant started in the manager adds a new mod, the manager shall show the library with that mod selected and say that it is installed. Traces: F-APP-8.
- **APP-F-79** The manager shall show that a check for updates is running until it ends, and show where a folder dragged over its window will go before it is dropped. Traces: F-APP-8, F-INST-7.

### Agents

- **APP-F-25** The participant shall be able to define a toolset: a name and a choice of calls of installed mods. The app shall name each call by its MCP tool name, derived by the app in Rust, including saved entries whose mod is disabled or unavailable. The participant can remove a saved toolset; its row and token selection disappear, while bound tokens remain as the host leaves them. Traces: F-AGENT-1, F-AGENT-3.
- **APP-F-26** When the participant asks for a token for a toolset and an agent handle, the app shall get one from the host and show it once, with the MCP address of the toolset. Traces: F-AGENT-2, W6.
- **APP-F-27** The app shall list the active tokens without their secret, and end a token when the participant asks. Traces: F-AGENT-2.

### Agent layer (step 11)

- **APP-F-80** The participant shall be able to sign in with their ChatGPT account in Settings. The app shall open the participant's browser at OpenAI, accept the answer only on a loopback address that it listens on for that one attempt, and keep the result on this device. Settings shall say `Signed in with ChatGPT` with the plan, or `Not signed in`. Traces: F-AGENT-5, J11.
- **APP-F-81** The participant shall be able to sign out in Settings. After that no mod shall be able to use the account, and the sessions that use it shall end. A sign-in that cannot be renewed shall show as signed out, with the cause. Traces: F-AGENT-5, F-AGENT-7.
- **APP-F-82** When a mod first asks to start agents in a folder, to use the participant's ChatGPT plan, or to let agents call a toolset, the consent view shall ask, naming the mod that began the call, in the words `Let <mod> have agents work in <folder> (<profile>, through <provider>)?`, `Let <mod> use your ChatGPT plan?` and `Let <mod>'s agents call: <functions>?`. The consent view shall never make the mod wait for the answer in a call; a denial shall change nothing and shall be asked again next time. Traces: F-AGENT-7, F-APP-4, J11.
- **APP-F-83** The details of a mod shall list the consents it holds, each in the words of its question, and the participant shall be able to withdraw each one. A withdrawal shall end what depends on it. Traces: F-AGENT-7, F-INST-5, J11.
- **APP-F-84** The grants to start Claude Code shall read in words in the consent view and in the details of a mod, `Can start Claude Code with your Claude account, to read and change files in folders you choose. Not checked by watane.` and `Can start Claude Code with your Claude account and let it run programs and use the web, inside Claude's sandbox. Not checked by watane.` The plan of every mod whose calls reach either grant shall say `Uses Claude Code through <provider>.` Traces: F-AGENT-5, F-INST-1, J11.
- **APP-F-85** The participant shall be able to choose the `claude` program in Settings. The app shall refuse a program older than the minimum version, with the version found and the version needed, and keep none until a good one is chosen. Traces: F-AGENT-5.
- **APP-F-86** On the page that the manager shows for a checked provider, the participant shall be able to enter a key and the origin of the provider's judge, see that a key is kept and for which origin (never the key), and remove it. The origin shall show next to the key before it is saved and shall never be filled in from the provider. Removing the key, or removing the provider, shall delete it. Traces: F-AGENT-9.
- **APP-F-87** No mod shall be able to read a sign-in, a key or the `claude` path: they stay in the app and reach the host in process only. Traces: F-AGENT-5, F-AGENT-9, F-RUN-4.

## Step 3: Publish

- **APP-F-28** At the first start, the app shall create the identity of the participant, with no network contact. Traces: F-APP-2, J3.
- **APP-F-29** The app shall show the identity as the profile name and the key fingerprint. The participant shall be able to set the profile name, after consent, and the web address of the site folder. The app shall show the site folder and the link to the own channel. Traces: F-PUB-1, F-PUB-3, J3.
- **APP-F-30** When the participant asks for a backup, the app shall ask for a passphrase twice, refuse a passphrase shorter than 12 characters or two that differ, and write the backup to a file the participant chooses. Traces: F-APP-6.
- **APP-F-31** When the participant restores a backup with its passphrase, the app shall replace the identity, and then ask for the link to the own channel or the statement that this identity never published. If the own channel already holds a statement, then the app shall refuse the restore and name the cause. Traces: F-APP-6, F-PUB-7.
- **APP-F-32** When a local program asks to publish a package, the app shall ask dist to publish it after consent, and answer with the release, the site folder and the link. Traces: F-PUB-2, F-PUB-3, F-AGENT-4, J3, W13.
- **APP-F-33** For a signature, the consent view shall show the kind of statement, the signer, and its details with readable names first and the full key or release reference after ` — `: for a release, the mod, the version, the package id and whether the source is included; for a withdrawal, the release and the reason; for a profile, the name. Traces: F-AGENT-4, F-PUB-4, F-PUB-5.
- **APP-F-34** When the participant enters a link, the app shall follow its channel. When the link names a release, the app shall fetch that release, plan its install with each provider fetched, and install it after consent. Traces: F-PUB-6, F-INST-6, F-INST-4, J3.
- **APP-F-35** The app shall show the publisher of each published mod by its profile name and key fingerprint, with "signature verified". Traces: F-APP-3, F-APP-5, J3.
- **APP-F-36** The app shall refresh each followed channel at start, every 10 minutes and on request. Traces: F-PUB-6, F-QUAL-1.
- **APP-F-37** If a refresh, a fetch or a verification fails, then the app shall show the channel or the release with the cause that dist names. If a channel is compromised, then the app shall show it as compromised. Followed-channel rows show the party label and `Read <ago>`, `Couldn't read: <cause>` or `Compromised: <cause>`, with no base URL or raw status token in the text. Following a channel or adding an index disables its button with `aria-busy="true"` and `Following…` or `Adding…` until it settles, then restores the button on success or error. Errors use plain Display text, with each cause once and no Debug or serde diagnostics. A failed index read says `Could not read <index>: <cause>` using its name, else its fingerprint. Mod refresh notices use the same channel name and cause text. A wrong-channel follow names both the served and expected parties, using stored profiles, `you` for the own identity, else fingerprints. Only the latest main-window action may set its error line; a fast retry clears an earlier failure and an older pending failure cannot return. Traces: F-PUB-7, F-QUAL-3, J3.

## Step 4: Find

- **APP-F-38** The participant shall be able to add an index from a link, and to use a followed channel as an index, see each chosen index with its name, fingerprint, state and the number of mods it lists, and drop it. The app shall add no index by itself. With only an empty own index, its row is hidden and the add-an-index hint shows alone; a live own listing or include, or another chosen index, shows the own row again. Traces: F-FIND-2, J4, W23.
- **APP-F-43** The participant shall be able to search the chosen indexes by words, by publisher and by interface. Each result shall show the mod's name, summary and version, its publisher by profile name and fingerprint with "signature verified" (or "compromised", APP-F-44), and the indexes that list it with their notes. An empty search reads `No mods found`. Duplicate direct/through provenance reads once as `listed by Ivy through Alice`; own-index provenance reads `listed by you`. Traces: F-FIND-3, F-FIND-4, F-APP-3, J4.
- **APP-F-44** The app shall show when each result's channel was last read, and mark a result whose channel could not be read since then. It shall show a result of a compromised channel as compromised, offer no install for it, and say in words why and what the participant can do. Traces: F-FIND-4, F-QUAL-3.
- **APP-F-45** When the participant installs a result, the app shall follow its publisher, fetch that release and show the install plan in the consent view. Traces: F-FIND-4, F-INST-1, F-INST-6, J4.
- **APP-F-46** The participant shall be able to list a mod of a followed channel in the own index with a note, include a followed channel as an index, and end each listing. Each one is a signature in the consent view. An already listed mod says `already listed`; an already included channel says `This channel is already in the index.`, without sequence numbers. Traces: F-FIND-1, F-AGENT-4, APP-F-33.
- **APP-F-39** When a refresh reports a new version that asks for no new grant, the app shall ask the host to update with no consent view. When a version asks for a new grant, the app shall show the plan in the consent view. A declined update reads `You declined the update to <version>.`; other failures read `Update to <version> failed: <cause>`. Compromise and withdrawal notices take priority. The offer names new grants as `Install <version> — asks for: <text>, <text>` even after a restart; an offer without new grants reads `Install <version>`. Traces: F-INST-7, J4.
- **APP-F-40** When a refresh reports a withdrawn installed version, the app shall tell the participant and offer the newest version that is not withdrawn. Own withdrawn releases read `Withdrawn: <reason>`; another publisher's release reads `Withdrawn by the publisher: <reason>`. Invalid withdrawal reasons use dist's cause: `invalid character count` reads `Give a reason of 1 to 280 characters.`; others read `This reason cannot be used: <cause>.` Traces: F-INST-8, J4.

## Step 5: Build on

- **APP-F-41** When a provider of a mod to install cannot be fetched, the app shall show the cause and the released mods of the chosen indexes and followed channels that fit in its place, verified and not compromised, and bind the one the participant picks only after the install consent shows that bind. Every party in picks, binds, grants, origins and index/search views uses its locally stored profile name even if not followed; the own identity reads `you`. With no fitting mod, or no pick, nothing is installed. Traces: F-BUILD-3, W26, J5.
- **APP-F-42** The app shall show the release a mod was made from as the publisher's claim, in the details, the install consent and the release signature, and list the mods made from an installed release as far as the chosen indexes list them. An installed derived mod shows `Installed` and no install button. Traces: F-BUILD-1, F-BUILD-4, W24.
- **APP-F-47** When the participant publishes a release that depends on a mod of another publisher, the app shall offer to list that mod in the own index, each listing with its own signature, and report which providers are listed. Traces: F-BUILD-2, W25, J5.
- **APP-F-48** When a local program asks for a release by its link, the app shall fetch and verify it and answer with the package, and shall install, grant, sign and follow nothing. A request with both/neither link and release (including both null) answers 400 `bad_request`, `give link or release`. Non-JSON or wrong-shaped bodies answer 400 `bad_request`, `give link or release: <serde cause>`, keeping the rejection body text. A wrong method on an app API route answers 405 `method_not_allowed` in the common JSON error envelope. Traces: F-BUILD-1, F-BUILD-2, F-AGENT-4.
- **APP-F-49** After it installs a release of a publisher that it does not follow, the app shall follow that publisher's channel. Traces: F-INST-7, F-INST-8, J5.

## Step 8: Own machines

"Own machines" are the participant's machines that `net` connects (spec
F-OWN). "The anchor" is the home that holds the identity; a "replica"
holds none (W33). A machine is named by its short key (8 characters).

- **APP-F-50** The app shall open its home through `node`. With own machines off, every function shall work as before. On a replica, the app shall show that this home holds no identity and is a full or light machine of the anchor, offer no publish, backup, restore or own listing, and answer a local publish request with `no_identity`. Traces: F-OWN-1, F-OWN-3, F-QUAL-1.
- **APP-F-51** Own machines shall be off until the participant turns them on in the main window, after the app showed what turning on does; the participant shall be able to turn them off. Traces: F-OWN-1, W32.
- **APP-F-52** On the anchor, the participant shall be able to ask for a pairing code for a full or a light machine. The app shall show the code, a copy button and the time it stays valid. Traces: F-OWN-2.
- **APP-F-53** When a machine presents a code, the consent view shall show the join question with the machine's short key and its role, and the anchor shall pair it only on approve; every other end is a no (W19). Traces: F-OWN-2.
- **APP-F-54** On a home whose identity signed no statement, the participant shall be able to join own machines with a code. The app shall first say that this home's unused identity, named by its fingerprint, will be deleted, with "cancel" as the default, and shall show this machine's short key while the anchor's answer is pending. A home whose identity signed a statement shall be refused, with the cause. Join shall be disabled immediately while a join starts and enabled again on failure. Before the first signature, the consent view shall warn that this home can no longer join its other machines. Traces: F-OWN-2, F-OWN-3.
- **APP-F-55** The app shall show the own machines: each one's short key, role, whether it is this machine or the anchor, for other machines whether it is online or when it was last seen, and when this machine last pulled from it. This machine's row shall say `this machine` without online or last-seen text. On the anchor, the participant shall be able to remove a machine after a question, change its role, and set the expiry of the list in days (0 for never). Traces: F-OWN-3, F-OWN-4, F-OWN-6.
- **APP-F-56** The participant shall be able to list, add and remove relays. A relay shall be added only after the consent view showed what a relay can see. Traces: F-OWN-7, W32.
- **APP-F-57** The participant shall be able to turn the DHT on and off. It shall be turned on only after the consent view showed what the DHT publishes. Traces: F-OWN-7, F-OWN-8, W32.
- **APP-F-58** The app shall show each refusal of a machine and each machine it cannot reach, with the short key and the cause. When no path exists without a relay or the DHT, the text shall say so. Traces: F-OWN-7, F-QUAL-3.
- **APP-F-59** On a light machine, a search result that came through another machine shall name that machine, and shall be verified like any other result. Traces: F-OWN-5, F-FIND-4.
- **APP-F-60** After a restore, the identity view shall show the heads of the own channel that own machines report, each with its machine and position, as claims to pick from. The app shall confirm the own channel only after the participant picks one, and shall offer no pick for a forked head. Traces: F-APP-6, W18.
- **APP-F-61** No mod and no local program shall reach any function of this step: no local API route and no mod window command turns machines on, pairs, removes, or changes a relay or the DHT. Traces: F-AGENT-4, F-OWN-7.

## Step 10: A mod as manager

- **APP-F-70** The grant to manage shall read in words in the consent view and in the details of a mod, like every other grant. Traces: F-INST-9, F-APP-5, J10.
- **APP-F-71** Each request of a managing mod shall show in the consent view, which shall name that mod, with its publisher or "local", as the one who asks. The app shall act on it only after the participant approves. Traces: F-INST-11, F-APP-4, F-AGENT-4, J10.
- **APP-F-72** An install or an update that a managing mod asked for shall show in the consent view also when it adds no grant. For a link of a channel that the app does not read yet, the app shall first ask the participant whether to read it, and shall contact no host for the request before the participant approves. Every request of a managing mod shall offer to deny it and stop that mod's asks, which withdraws its grant to manage. Traces: F-INST-11, F-OWN-7, J10.
- **APP-F-73** A request to remove a mod shall say whether its data is deleted, and name the mods that use it (APP-F-67). Traces: F-INST-5, F-INST-11, J10.
- **APP-F-74** When a managing mod disables a mod or closes its page, the app shall close that mod's window, and the details of a disabled mod shall say which mod turned it off, until the app restarts. Traces: F-INST-10, F-APP-9, J10.
- **APP-F-75** When the participant denies a request, closes the consent view or does not answer in 10 minutes, nothing shall change, and the managing mod shall learn that the request was denied. Traces: F-INST-11, W19.

## Acceptance scenarios

Each scenario is a test (TECHNICAL.md section 9 names the test).

**S1 Empty start** (APP-F-1, APP-F-3, APP-F-4). Given a new home, when the
app starts, then the library shows no mod entry, the header shows two mark
words, and `<home>/mods/` holds no mod.

**S2 Install two local mods** (APP-F-12, APP-F-19, APP-F-20). Given the
built folders `reading-list` and `recommendations` (which calls and
subscribes to `reading-list`), when the participant chooses both, then the
consent view lists `reading-list` before `recommendations`, both "local",
with the grant texts of TECHNICAL.md section 4; and when the participant
approves, then both appear in the library and in `<home>/mods/`.

**S2b Withhold a grant** (APP-F-19). Given the plan of S2, when the
participant unchecks the storage grant of `reading-list` and approves, then
the details of `reading-list` show that grant as not granted.

**S3 Deny** (APP-F-12, APP-F-18). Given an open install request, when the
participant closes the consent view, then the request gets "deny" and
`<home>/mods/` is unchanged.

**S4 Approve delay** (APP-F-16). Given a consent view that got the focus at
time t, when approve is clicked at t + 100 ms, then no answer goes out; when
it is clicked at t + 900 ms, then "approve" goes out.

**S5 Stays above** (APP-F-14). Given an open mod window, when a request
arrives, then the consent window is always-on-top, visible on all spaces and
focused, and the mod window is not always-on-top.

**S6 Hostile page** (APP-F-8, APP-F-9, APP-F-10). Given the installed
fixture mod `hostile`, when its page runs its attacks, then no app command
handler runs for a label `mod-*`, no plugin command succeeds, the test
listener gets no connection, no new window exists, the window URL stays on
its origin, and `<home>/mods/` is unchanged.

**S7 Fresh storage** (APP-F-10). Given a mod page that wrote localStorage,
when its window closes and opens again, then localStorage is empty.

**S8 Close on disable** (APP-F-11). Given an open mod window, when the
participant disables the mod, then the window no longer exists.

**S9 Withdraw a grant** (APP-F-23, J2). Given `recommendations` with its
call grant, when the participant withdraws it and adds a book in
`reading-list`, then the `recommendations` page shows `not granted` with the host's message (`no provider is linked`). With its provider disabled, opening the page shows the host's `disabled or unavailable`; after enabling the provider, the page reads again and clears its error. When the participant gives the grant back in the library, with no consent view, then the page
shows the book again.

**S10 Remove** (APP-F-24). Given `recommendations`, when the participant
removes it and chooses "delete data", then its folder in `<home>/mods/`
does not exist; after a new install of the same folder, its state is empty.

**S11 Token** (APP-F-25, APP-F-26, APP-F-27). Given a toolset `reader` with
the list tool of `reading-list`, when the participant asks for a token for
handle `claude`, then an MCP `tools/call` of that tool with that token
succeeds; and after the participant ends the token, the same call is
refused as unauthorized.

**S12 Identity** (APP-F-28, APP-F-30, APP-F-31). Given a new home with the
network disabled, when the app starts, then an identity exists. When the
participant saves a backup and restores it on a second new home, and states
that the identity never published, then both homes show the same
fingerprint. When the second home has published, then a restore is refused
with the cause.

**S13 Publish** (APP-F-32, APP-F-33). Given a packed mod, when a local
program sends its bytes in `POST /api/publish`, then the consent view shows
"release", the slug, the version and the package id; after approve, the
answer is 200 with the release, the site folder and the link; after deny,
the answer is 403 with code `refused` and the site folder is unchanged.

**S14 Install from a link** (APP-F-34, APP-F-35, J3). Given Alice's site
folder on a local static server and her link, when Bob enters the link,
then his consent view shows Alice's profile name, her fingerprint and
"signature verified"; after approve, the mod runs in Bob's app.

**S15 Tampered file** (APP-F-37, J3). Given Alice's site folder on a local
static server, when a byte of the package file changes before Bob enters
the link, then Bob's app shows a cause that names the hash mismatch and
installs nothing.


**S16 Stay up to date without an index** (APP-F-33, APP-F-36, APP-F-37,
APP-F-39, APP-F-40). Given Carol installed Alice's reading list from a link,
a refresh installs 0.2.0 with unchanged grants without opening consent and
keeps her books. A 0.3.0 update requesting network access shows the new
grant; declining leaves 0.2.0 and shows `You declined the update to 0.3.0.` until a successful
install. Alice sees her releases and withdraws 0.3.0 with reason `breaks
lists`, signing in consent. Carol's next refresh shows that reason and an
`Install 0.2.0` button. Installing it keeps data and asks no question.
A different signed statement at an already read position marks both the
channel and the installed mod as compromised.

**S17 Add an index** (APP-F-38, J4). Given Ivy's site folder, which lists
Alice's `reading-list`, and Alice's site folder, each on a local static
server, when the participant adds Ivy's link as an index, then the find
view lists Ivy with her name and "1 mod", and no consent view opened.

**S18 Search and install** (APP-F-43, APP-F-45, J4). Given S17, when the
participant searches `reading`, then one result shows `reading-list`, its
version, Alice's name and fingerprint with "signature verified", and
"listed by Ivy" with Ivy's note; when the participant installs it and
approves, then the mod runs and Alice is a followed channel.

**S19 Stale mark** (APP-F-44). Given S17, when Alice's server stops and the
participant refreshes, then the result still shows, with "could not check
since" and the time of the last read that succeeded.

**S20 Compromised result** (APP-F-44). Given S17, when Alice's channel forks,
then the result shows "compromised" and has no install button.

**S21 List in my index** (APP-F-46). Given the participant follows Alice,
when they list `reading-list` with the note `good for books`, then the
consent view shows "list", the mod `<Alice key>/reading-list`, Alice's
location and the note; after approve, the own site folder has the new
entry; after a later "unlist", another entry.

**S22 No index at first** (APP-F-38, W23). Given a new home, the find view
lists no index and says to add one from a link.

**S23 Pick a provider** (APP-F-41, J5). Given Eve follows Dave, whose
`stats` needs Carol's `ratings`, Carol's site is down, and Eve's index
lists Frank's `ratings` (same WIT), when Eve installs `stats`, then a pick
view names Carol's `ratings`, the cause, and one candidate, Frank's; after
she picks it, the install consent shows `stats` with the line
`Frank's ratings in place of Carol's ratings (your pick)`; after approve,
`stats` calls Frank's mod, its grant line reads `Call ... of ratings by
Frank (in place of ratings by Carol)`, and Frank is a followed channel. A `Deny` in the
pick view installs nothing.

**S24 No fit** (APP-F-41). Given S23 with Frank's `ratings` lacking an
interface that `stats` calls, when Eve installs `stats`, then no pick view
opens and the error names Carol's `ratings` and says that no mod fits.

**S25 Made from** (APP-F-42). Given Carol's `ratings` names Alice's
`reading-list@0.1.0` as its origin, when Eve installs it, then the consent
view and the details show `made from` Alice's mod with Alice's name and
`(the publisher says so)`; Alice's `reading-list` details list `ratings`
under the mods made from it.

**S26 Publish lists the provider** (APP-F-47). Given `stats` depends on
Carol's `ratings`, when Dave publishes `stats` with Carol's link in
`Watane-Provider-Links`, then the consent view asks to sign the release,
then to list `<Carol>/ratings` with the note `used by stats`; after both
approves the answer has `providers: [{provider: "<Carol>/ratings", listed:
true}]`. A `Deny` of the listing keeps the release and gives `listed:
false`.

**S27 Fetch route** (APP-F-48). Given Carol's site, when `POST /api/fetch`
names her `ratings` link, then the answer is the package bytes with the
headers of `Watane-Release` and `Watane-Package`, no consent view opened,
and Carol is not a followed channel. A link with no slug is 400; with an
`Origin` header, 403.

**S28 Off by default** (APP-F-50, APP-F-51). Given a new home, when the
app starts, then the machines view says own machines are off, no UDP
socket is open, and J3's publish works as before. When the participant
turns them on, the view first shows the text of `node` 7.

**S29 Pair a full machine** (APP-F-52, APP-F-53, APP-F-55). Given the
anchor's app with own machines on, when the participant asks for a full
code and `watane-node join` uses it, then the consent view shows the
node's short key, the same that the program printed, and "full"; after
approve, the machines view lists both. When a second machine uses a new
code and the participant closes the consent view, the list is unchanged.

**S30 Join as light** (APP-F-54, APP-F-50). Given a second app home whose
identity signed nothing, when the participant enters a light code, then
the question names that identity's fingerprint; after confirm and the
anchor's approve, the identity view says this home holds no identity,
`<home>/log/identity.key` is gone, and `POST /api/publish` answers 409
`no_identity`. A home that published is refused with the cause, and its
key is unchanged.

**S31 Manage machines** (APP-F-55). Given S30, when the participant
removes the light machine on the anchor and confirms, then the full
machine stops serving it; a role change and an expiry of 1 day show in the
machines view of every machine.

**S32 Relay and DHT** (APP-F-56, APP-F-57). When the participant adds a
relay, the consent view shows the relay text; a deny leaves the list
unchanged, an approve adds it. When the participant turns the DHT on, the
consent view shows the DHT text; after approve it is on; turning it off
asks nothing.

**S33 Problems named** (APP-F-58). Given a full machine that serves a
changed byte, the machines view shows `refused <short key>: <cause>`.
Given two machines with no path, it shows the cause that names a relay or
the DHT.

**S34 Results through a machine** (APP-F-59). Given S30 and the full
machine holding Ivy's index and Alice's channel, when the light machine
searches `reading`, then the result names the full machine's short key
and shows "signature verified"; installing it runs the mod.

**S35 Restore from machines** (APP-F-60). Given a restored identity on a
new home that paired the full machine with `replace`, then the identity
view lists the full machine's hint with its seq; a publish is refused
until the participant picks it; then the next release has the next seq.

**S36 No outside path** (APP-F-61). The hostile page's calls of every
command of stage G are refused; no local API route names machines,
relays or the DHT.

**S37 Start in the main mod** (APP-F-62, APP-F-63). Given a home with
`reading-list` and a fixture `desk` whose `mod.toml` has `main = true`,
installed in that order, when the app restarts, then the window
`mod-desk` opens and the main window does not. When `desk` is disabled
and the app restarts, the main window opens on the library.

**S38 Choose and unset** (APP-F-63). Given S37, when the participant
makes `reading-list` main, then `<home>/app/main` holds `reading-list\n`;
when they unset it, the file is gone; when they remove the main mod, the
file is gone.

**S39 Menu** (APP-F-64). Given S37 with only `mod-desk` open, when the
participant chooses `Mods…`, then the main window opens on the library;
when they close `mod-desk` and then the main window, the app quits.

**S40 Connections** (APP-F-66). Given `reading-list` and
`recommendations` (which calls and subscribes to it) installed, then the
details of `recommendations` show `Reading list` under `Uses` and under
`Adds to`, and those of `reading-list` show `Recommendations` under
`Used by` and `Extended by`. After the participant withdraws the
call grant, `Uses` and `Used by` are gone.

**S41 Warn before disable** (APP-F-67). Given S40, when the participant
disables `reading-list`, then the page names `Recommendations` and
`Cancel` has the focus; Cancel leaves both enabled.

**S42 Settings apart** (APP-F-65). The library, Find and Updates views
hold no control of identity, site, backup, agents or machines; Settings
holds them all.

**S49 The manager looks as drawn** (APP-F-3, APP-F-77, W37). Given
`desk` (main), `reading-list` with an offer that asks for a new grant, and
`recommendations` disabled, then the library shows `3 mods` and three tiles
in this order: `Desk` marked `Opens at start`, `Reading list` with the
badge `Update` and `Recommendations` with the badge `Off`; selecting `Recommendations`
shows its details beside the tiles with `Call books of Reading list (local)`
and a switch that is on; switching it off withdraws the grant at once.
Updates shows the offer under `Needs you` with `Review…`; `Not now` hides it.
A home with no mod shows `Install your first mod` with the three ways and no
Updates tab.

**S50 Light and dark** (APP-F-76). When the system turns dark, the
manager's page and tiles turn dark; back to light, they turn light.

**S51 Main through a local program** (APP-F-63). Given a running app with
no main mod, when a local program installs `desk` (`main = true`) and the
participant approves the consent view, which says that `desk` becomes the
start window, then `desk` is the main mod and the next start opens its
window. After the participant turns `Opens at start` off, an update of `desk`
leaves no main mod.

**S52 After install, while checking, while dropping** (APP-F-78,
APP-F-79). Given S49, when the participant installs `book-club` from
Find and approves the consent view, then the library shows with
`Book club` selected and `Book club is installed.` above the tiles. When
they choose `Check now`, the button reads `Checking…` and cannot be
chosen until the refresh ends, and then `Checked just now` shows. When
they drag a folder over the window, `Drop to install` covers it; when
the drag leaves, it is gone.

**S53 Disable the main mod** (APP-F-67). Given S49, when the participant
disables `desk`, then the panel `Disable Desk?` says `Until you enable it,
the manager opens at start.`; when they remove `reading-list` with
`Delete its data` chosen, the confirming button reads `Remove and delete
data`.

**S54 Sign in with ChatGPT** (APP-F-80). Given the app with a
stand-in for OpenAI's sign-in, when the participant chooses `Sign in with
ChatGPT` in Settings, then the browser stand-in is opened at the stand-in
issuer, the participant's answer arrives on the loopback address, and
Settings reads `Signed in with ChatGPT` with the plan.

**S57 Sign out** (APP-F-81). Given S54, when the participant chooses `Sign
out`, Settings reads `Not signed in` and `<home>/app/accounts/openai.json` is
gone.

**S58 A consent for an agent** (APP-F-82). Given a mod that asks to start
agents in a new folder, when it asks, the consent view reads `Let <mod>
have agents work in <folder> (<profile>, through <provider>)?` and the
mod's call has already returned; when the participant denies, nothing is
kept and the next ask shows the view again.

**S60 Consents in the details** (APP-F-83, APP-F-84). Given S58 approved,
when the participant opens the details of the mod, then the consent shows
in the words of its question, and so does the grant to start Claude Code
in its short words; when they withdraw the consent, it is gone from the
details and the mod's next ask shows the consent view again.

**S61 Choose `claude`** (APP-F-85). Given a stand-in `claude` that reports
a version below the minimum, when the participant chooses it in Settings,
then Settings says `Claude Code <found> is too old; watane needs <need>
or later.` and keeps no program; a stand-in at the minimum is kept and
shown by its path.

**S63 The judge key** (APP-F-86). Given a checked provider
installed, when the participant opens its details, enters a key and the
origin `https://judge.example`, then the origin shows next to the key
before saving; after saving, the page says `A key is kept for
https://judge.example` and never shows the key; a fresh start of the app
shows the same; `Remove key` removes it.

**S66 Remove the provider** (APP-F-86). Given S63 with a key kept, when the
participant removes the provider, its key file is gone, and a reinstall of the
same mod shows no key kept.
