# app: functional specification

What the app does, free of implementation. "The app" is the desktop
application of FUNCTIONAL.md section 1. Ids `APP-F-<n>`. Each `Traces:`
line cites spec/FUNCTIONAL.md. Step 1 needs nothing from the app (J1 uses a
preview). Steps 4 and 5 are sketched only, so that no design blocks them.

## Step 2: Run together

### Start and launcher

- **APP-F-1** When the app starts, the app shall open the home of the participant, run its host in the app process, and serve the host on the loopback interface. Traces: F-APP-1, J2.
- **APP-F-2** If the app cannot open the home, then the app shall show the cause, run no mod, and accept no request. A busy loopback port keeps the main window open with `Port P is in use, maybe by another watane app. Start this one with --port <n> or WATANE_PORT=<n> to use another port, or --port 0 for any free port.` The same line goes to stderr once. Traces: F-QUAL-3.
- **APP-F-3** The app shall show a launcher with one entry for each installed mod. When no mod is installed, the launcher shall show no entry. Traces: F-APP-1, J2.
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
- **APP-F-19** For an install plan, the consent view shall show each mod in the order of install, with its name, version, publisher (with "signature verified") or "local", the version it replaces if any, each grant in words with each new grant marked, and the provider mods it uses. The heading is `Install <name>` for one new mod, `Update <name> to <version>` for one replacement, or `Install <n> mods` for several. The participant shall be able to withhold each grant; the app shall then approve only the other grants. Traces: F-INST-1, F-INST-2, F-INST-3, F-INST-4, F-INST-7, J2.

### Manage mods

- **APP-F-20** When the participant chooses one or more mod folders, the app shall ask the host for a plan of all of them and install the plan after consent. Traces: F-INST-1, F-INST-2, F-INST-4, J2.
- **APP-F-21** If a plan needs a provider that is not installed and not supplied, then the app shall name each missing provider and install nothing. Traces: F-INST-4, F-QUAL-3.
- **APP-F-22** The app shall show, for each mod, its name and installed version next to each other outside Details, publisher or "local", grants in words, page origin and package id. Traces: F-APP-5.
- **APP-F-23** The participant shall be able to open, disable, enable and remove each mod, withdraw each grant, and give back a withdrawn or withheld grant. Traces: F-INST-5, J2.
- **APP-F-24** When the participant removes a mod, the app shall ask whether to delete its data, with "keep data" as the default. Traces: F-INST-5.

### Agents

- **APP-F-25** The participant shall be able to define a toolset: a name and a choice of calls of installed mods. The app shall name each call by the tool name that the host gives it. Traces: F-AGENT-1, F-AGENT-3.
- **APP-F-26** When the participant asks for a token for a toolset and an agent handle, the app shall get one from the host and show it once, with the MCP address of the toolset. Traces: F-AGENT-2, W6.
- **APP-F-27** The app shall list the active tokens without their secret, and end a token when the participant asks. Traces: F-AGENT-2.

## Step 3: Publish

- **APP-F-28** At the first start, the app shall create the identity of the participant, with no network contact. Traces: F-APP-2, J3.
- **APP-F-29** The app shall show the identity as the profile name and the key fingerprint. The participant shall be able to set the profile name, after consent, and the web address of the site folder. The app shall show the site folder and the link to the own channel. Traces: F-PUB-1, F-PUB-3, J3.
- **APP-F-30** When the participant asks for a backup, the app shall ask for a passphrase twice, refuse a passphrase shorter than 12 characters or two that differ, and write the backup to a file the participant chooses. Traces: F-APP-6.
- **APP-F-31** When the participant restores a backup with its passphrase, the app shall replace the identity, and then ask for the link to the own channel or the statement that this identity never published. If the own channel already holds a statement, then the app shall refuse the restore and name the cause. Traces: F-APP-6, F-PUB-7.
- **APP-F-32** When a local program asks to publish a package, the app shall ask dist to publish it after consent, and answer with the release, the site folder and the link. Traces: F-PUB-2, F-PUB-3, F-AGENT-4, J3, W13.
- **APP-F-33** For a signature, the consent view shall show the kind of statement, the signer, and its details: for a release, the mod, the version, the package id and whether the source is included; for a withdrawal, the release and the reason; for a profile, the name. Traces: F-AGENT-4, F-PUB-4, F-PUB-5.
- **APP-F-34** When the participant enters a link, the app shall follow its channel. When the link names a release, the app shall fetch that release, plan its install with each provider fetched, and install it after consent. Traces: F-PUB-6, F-INST-6, F-INST-4, J3.
- **APP-F-35** The app shall show the publisher of each published mod by its profile name and key fingerprint, with "signature verified". Traces: F-APP-3, F-APP-5, J3.
- **APP-F-36** The app shall refresh each followed channel at start, every 10 minutes and on request. Traces: F-PUB-6, F-QUAL-1.
- **APP-F-37** If a refresh, a fetch or a verification fails, then the app shall show the channel or the release with the cause that dist names. If a channel is compromised, then the app shall show it as compromised. Traces: F-PUB-7, F-QUAL-3, J3.

## Step 4: Find

- **APP-F-38** The participant shall be able to add an index from a link, and to use a followed channel as an index, see each chosen index with its name, fingerprint, state and the number of mods it lists, and drop it. The app shall add no index by itself. Traces: F-FIND-2, J4, W23.
- **APP-F-43** The participant shall be able to search the chosen indexes by words, by publisher and by interface. Each result shall show the mod's name, summary and version, its publisher by profile name and fingerprint with "signature verified" (or "compromised", APP-F-44), and the indexes that list it with their notes. Traces: F-FIND-3, F-FIND-4, F-APP-3, J4.
- **APP-F-44** The app shall show when each result's channel was last read, and mark a result whose channel could not be read since then. It shall show a result of a compromised channel as compromised and offer no install for it. Traces: F-FIND-4, F-QUAL-3.
- **APP-F-45** When the participant installs a result, the app shall follow its publisher, fetch that release and show the install plan in the consent view. Traces: F-FIND-4, F-INST-1, F-INST-6, J4.
- **APP-F-46** The participant shall be able to list a mod of a followed channel in the own index with a note, include a followed channel as an index, and end each listing. Each one is a signature in the consent view. Traces: F-FIND-1, F-AGENT-4, APP-F-33.
- **APP-F-39** When a refresh reports a new version that asks for no new grant, the app shall ask the host to update with no consent view. When a version asks for a new grant, the app shall show the plan in the consent view. A declined update reads `You declined the update to <version>.`; other failures read `Update to <version> failed: <cause>`. Compromise and withdrawal notices take priority. The offer names new grants as `Install <version> — asks for: <text>, <text>` even after a restart; an offer without new grants reads `Install <version>`. Traces: F-INST-7, J4.
- **APP-F-40** When a refresh reports a withdrawn installed version, the app shall tell the participant and offer the newest version that is not withdrawn. Own withdrawn releases read `Withdrawn: <reason>`; another publisher's release reads `Withdrawn by the publisher: <reason>`. Invalid withdrawal reasons use dist's cause: `invalid character count` reads `Give a reason of 1 to 280 characters.`; others read `This reason cannot be used: <cause>.` Traces: F-INST-8, J4.

## Step 5: Build on

- **APP-F-41** When a provider of a mod to install cannot be fetched, the app shall show the cause and the released mods of the chosen indexes and followed channels that fit in its place, verified and not compromised, and bind the one the participant picks only after the install consent shows that bind. With no fitting mod, or no pick, nothing is installed. Traces: F-BUILD-3, W26, J5.
- **APP-F-42** The app shall show the release a mod was made from as the publisher's claim, in the details, the install consent and the release signature, and list the mods made from an installed release as far as the chosen indexes list them. Traces: F-BUILD-1, F-BUILD-4, W24.
- **APP-F-47** When the participant publishes a release that depends on a mod of another publisher, the app shall offer to list that mod in the own index, each listing with its own signature, and report which providers are listed. Traces: F-BUILD-2, W25, J5.
- **APP-F-48** When a local program asks for a release by its link, the app shall fetch and verify it, follow its channel, and answer with the package, and shall install, grant and sign nothing. Traces: F-BUILD-1, F-BUILD-2, F-AGENT-4.
- **APP-F-49** After it installs a release of a publisher that it does not follow, the app shall follow that publisher's channel. Traces: F-INST-7, F-INST-8, J5.

## Acceptance scenarios

Each scenario is a test (TECHNICAL.md section 9 names the test).

**S1 Empty start** (APP-F-1, APP-F-3, APP-F-4). Given a new home, when the
app starts, then the launcher shows no mod entry, the header shows two mark
words, and `<home>/mods/` holds no mod.

**S2 Install two local mods** (APP-F-12, APP-F-19, APP-F-20). Given the
built folders `reading-list` and `recommendations` (which calls and
subscribes to `reading-list`), when the participant chooses both, then the
consent view lists `reading-list` before `recommendations`, both "local",
with the grant texts of TECHNICAL.md section 4; and when the participant
approves, then both appear in the launcher and in `<home>/mods/`.

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
`reading-list`, then the `recommendations` page shows its "not granted"
text. When the participant gives the grant back in the launcher, with no consent view, then the page
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
`stats` calls Frank's mod and Frank is a followed channel. A `Deny` in the
pick view installs nothing.

**S24 No fit** (APP-F-41). Given S23 with Frank's `ratings` lacking an
interface that `stats` calls, when Eve installs `stats`, then no pick view
opens and the error names Carol's `ratings` and says that no mod fits.

**S25 Made from** (APP-F-42). Given Carol's `ratings` names Alice's
`reading-list@0.1.0` as its origin, when Eve installs it, then the consent
view and the details show `made from` Alice's mod with Alice's name and
`(the publisher says so)`; Alice's `reading-list` details list `ratings`
under the mods made from it.

**S26 Publish lists the provider** (APP-F-47). Given Dave follows Carol,
when he publishes `stats`, then the consent view asks to sign the release,
then to list `<Carol>/ratings` with the note `used by stats`; after both
approves the answer has `providers: [{provider: "<Carol>/ratings", listed:
true}]`. A `Deny` of the listing keeps the release and gives `listed:
false`.

**S27 Fetch route** (APP-F-48). Given Carol's site, when `POST /api/fetch`
names her `ratings` link, then the answer is the package bytes with the
headers of `Watane-Release` and `Watane-Package`, no consent view opened,
and Carol is a followed channel. A link with no slug is 400; with an
`Origin` header, 403.
