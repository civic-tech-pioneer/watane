# dist: functional specification

What `dist` does, free of implementation. "dist" in a requirement is the
subsystem. "The participant" is the person who owns the home. "The app"
is the product that answers each confirmation. Ids of `spec/` are cited in
`Traces:` lines only.

## 1. Terms of this subsystem

| Term | Meaning |
|---|---|
| statement | One signed entry of a channel, of kind `profile`, `release` or `withdraw`, and from step 4 `list`, `include` or `unlist` (W15). |
| listing | A `list` statement (one mod of a channel) or an `include` statement (another index) that no `unlist` ended. (step 4) |
| chosen index | A channel whose listings the participant chose to search. The own channel is always one. (step 4) |
| read set | The channels that dist reads for one chosen index: the index, the channels it lists or includes, and the channels that an included index lists. (step 4) |
| site folder | The folder of plain files that holds the own channel and its packages, ready for a static host. |
| site URL | The address where the maker serves the site folder. |
| link | Text that names a channel by its key and its site URL, and optionally one mod in it by slug. |
| followed channel | A channel of another participant that dist reads and keeps up to date. |
| refresh | One read of the followed channels from their web hosts. |
| compromised | The state of a channel in which two valid statements contradict each other. |

## 2. Requirements

### 2.1 Identity (step 3)

- **DIST-F-1** When dist opens a home with no identity, dist shall create the identity on the device, with no network contact.
  Traces: F-APP-2, J3.
- **DIST-F-2** When the participant asks for a backup, dist shall give the identity as a file protected by a passphrase.
  Traces: F-APP-6.
- **DIST-F-3** When the participant restores a backup while the own channel has no statement, dist shall replace the identity, and shall sign nothing until the own channel is confirmed: read and verified from its link, or stated by the participant as never published.
  Traces: F-APP-6, F-PUB-7.
- **DIST-F-4** If the link given to confirm the own channel names another key, or its channel cannot be read and verified, then dist shall keep the own channel unconfirmed and name the cause.
  Traces: F-APP-6, F-QUAL-3.

### 2.2 Statements and the confirmation floor (step 3)

- **DIST-F-5** dist shall sign a statement only after the app approved exactly that statement.
  Traces: F-AGENT-4, J3.
- **DIST-F-6** If the app refuses a statement, then dist shall sign nothing and change no file.
  Traces: F-AGENT-4.
- **DIST-F-7** When dist asks for a confirmation, dist shall show the kind of the statement, the key of the own channel, and every field of the body that it will sign. For a release, this includes the size of the package and whether it includes the source.
  Traces: F-AGENT-4, F-APP-4, F-PUB-4.
- **DIST-F-8** When the participant sets a name, dist shall append a `profile` statement with that name.
  Traces: F-APP-3, F-INST-1.
- **DIST-F-9** If a name or a reason is empty, too long, or holds a control or direction character, then dist shall refuse it before it asks for a confirmation.
  Traces: F-APP-4, F-QUAL-3.
- **DIST-F-10** If the own channel is compromised or not confirmed after a restore, then dist shall refuse every new statement before it asks for a confirmation.
  Traces: F-PUB-7, F-APP-6.

### 2.3 Publish (step 3)

- **DIST-F-11** When the participant publishes a package, dist shall append a `release` statement with the release body of that package, which holds its name, version and package id.
  Traces: F-PUB-1, F-PUB-2, J3.
- **DIST-F-12** If the own channel already holds a release with the same slug and version, then dist shall refuse the publish, also when that release is withdrawn.
  Traces: F-PUB-2.
- **DIST-F-13** If a provider of the package has no release, then dist shall refuse the publish and name the provider.
  Traces: F-PUB-2, F-INST-4.
- **DIST-F-14** If a package is larger than 64 MiB, then dist shall refuse the publish.
  Traces: F-PUB-2, F-RUN-3.
- **DIST-F-15** When the participant withdraws a version of the own channel with a reason, dist shall append a `withdraw` statement that names that release and the reason.
  Traces: F-PUB-5, J4.
- **DIST-F-16** If the named release is not a release of the own channel, or is already withdrawn, then dist shall refuse the withdrawal.
  Traces: F-PUB-5.

### 2.4 Site folder and link (step 3)

- **DIST-F-17** When dist appends a statement to the own channel, dist shall write the own channel and its packages as plain files into the site folder.
  Traces: F-PUB-3, J3.
- **DIST-F-18** When the participant names another folder, dist shall write the same files into it and leave every other file in it unchanged.
  Traces: F-PUB-3.
- **DIST-F-19** The site folder shall hold only the statements of the own channel and the packages of its releases that are not withdrawn.
  Traces: F-PUB-3, F-PUB-5, F-RUN-2.
- **DIST-F-20** If a folder already holds a channel of another key, or another statement at a position of the own channel, then dist shall refuse to write and name the file.
  Traces: F-PUB-7, F-QUAL-3.
- **DIST-F-21** When the participant has set the site URL, dist shall give a link to the own channel, and a link to one mod of it.
  Traces: F-PUB-6, J3.

### 2.5 Follow and refresh (step 3)

- **DIST-F-22** When the participant follows a link, dist shall read the channel from its web host, verify each statement against the key in the link, keep the verified statements, and add the channel to the followed channels.
  Traces: F-PUB-6, J3.
- **DIST-F-23** If the first read of a followed link fails, then dist shall not add the channel, and shall name the cause.
  Traces: F-PUB-6, F-QUAL-3.
- **DIST-F-24** If a file from a web host fails verification, belongs to another key, or holds another position than its name, then dist shall keep nothing of that file and name the cause.
  Traces: F-INST-6, F-QUAL-3, J3.
- **DIST-F-25** When the participant asks for a refresh, dist shall read each followed channel that is not compromised, and report for each channel its new names, new releases, new withdrawals, unreadable statements and failures.
  Traces: F-PUB-6, F-INST-7, F-INST-8, J4.
- **DIST-F-26** If two valid statements claim one position of a channel, or a statement does not follow the one before it, then dist shall mark the channel compromised, keep both statements, report it, and read and fetch nothing more from that channel.
  Traces: F-PUB-7, J4.
- **DIST-F-27** If a web host serves an older state of a channel than dist holds, then dist shall keep what it holds and report the channel as behind.
  Traces: F-PUB-6, F-QUAL-3.
- **DIST-F-28** dist shall record, for each followed channel, the time of the last read that succeeded.
  Traces: F-FIND-4, F-INST-8.
- **DIST-F-29** When the participant unfollows a channel, dist shall stop reading it.
  Traces: F-PUB-6.
- **DIST-F-30** If a link names the own channel, then dist shall refuse to follow it.
  Traces: F-PUB-1.

### 2.6 Fetch (step 3)

- **DIST-F-31** When the caller asks for the package of a release, dist shall give it only after it verified that the release is a statement of a channel that is not compromised, that the release is not withdrawn, and that the BLAKE3 hash of the bytes equals the package id of the release.
  Traces: F-INST-6, J3.
- **DIST-F-32** If the hash of a package differs from the release, or its slug or version differs from the release, then dist shall refuse it, keep none of its bytes, and name the cause.
  Traces: F-INST-6, F-QUAL-3, J3.
- **DIST-F-33** When the participant opens a link that names a mod, dist shall follow the channel, refresh it, and fetch the newest release of that mod that is not withdrawn.
  Traces: F-INST-1, F-PUB-6, J3.
- **DIST-F-34** dist shall keep each verified package on the device, so that a later fetch of the same release needs no network.
  Traces: F-QUAL-1.

### 2.7 Limits and failures (step 3)

- **DIST-F-35** dist shall limit the size and the time of each read from a web host, and stop a read that exceeds a limit.
  Traces: F-RUN-3, F-QUAL-3.
- **DIST-F-36** dist shall do every function except a read from a web host with no network.
  Traces: F-QUAL-1.
- **DIST-F-37** When dist refuses or fails, dist shall name the cause, the file or address involved, and the statement involved.
  Traces: F-QUAL-3.

### 2.8 Find (step 4)

- **DIST-F-40** When the participant chooses an index from a link, dist shall read it as a channel with the same verification, and then read every channel of its read set.
  Traces: F-FIND-1, F-FIND-2, J4.
- **DIST-F-41** When the participant searches, dist shall search the releases of the own channel, the followed channels and the read sets of the chosen indexes by text, publisher and interface, and give a release of another participant only from the verified channel of its publisher.
  Traces: F-FIND-3, F-FIND-4, J4.
- **DIST-F-42** dist shall give, for each mod, only its newest version that is not withdrawn, and shall mark a result whose channel it could not read since the time of the last read that succeeded.
  Traces: F-FIND-4, F-INST-8.
- **DIST-F-43** dist shall read includes one level deep: the listings of an included index count, its own includes do not.
  Traces: F-FIND-1.
- **DIST-F-44** dist shall read at most a fixed number of channels because of listings, and report how many it left out.
  Traces: F-RUN-3, F-QUAL-3.
- **DIST-F-45** dist shall search with no network, over what it read before.
  Traces: F-QUAL-1.
- **DIST-F-46** When the participant lists a mod or includes an index, dist shall append a `list` or `include` statement with its link and a note; when the participant ends a listing, an `unlist` statement. Each asks for a confirmation (DIST-F-5).
  Traces: F-FIND-1, F-AGENT-4.
- **DIST-F-47** If a listing names the own channel, names a mod with no release that is not withdrawn, repeats a live listing, or its note breaks the text rules, then dist shall refuse it before it asks for a confirmation.
  Traces: F-FIND-1, F-QUAL-3.
- **DIST-F-48** A listing shall never decide the version, the bytes or the withdrawals of a mod: they come only from the channel of its publisher.
  Traces: F-FIND-4, F-INST-6.

## 3. Acceptance scenarios

Each scenario is one test. "Alice" and "Bob" are two homes on one device.
"The host" is a local HTTP server that serves Alice's site folder.

### Step 3

- **A1 Identity offline.** Given an empty home and no network, when dist
  opens it, then an identity exists and a second open gives the same key.
  (DIST-F-1, DIST-F-36)
- **A2 Refused publish.** Given an app that refuses, when Alice publishes a
  package, then the publish fails as refused, the own channel has no new
  statement, and the site folder is unchanged. (DIST-F-5, DIST-F-6)
- **A3 What is shown is what is signed.** Given an app that records each
  request and approves, when Alice publishes, then the release in the
  request equals the release decoded from the new statement, and the
  request names Alice's key, the size and the source flag. (DIST-F-7,
  DIST-F-11)
- **A4 Same version twice.** Given Alice published `reading-list` 1.0.0,
  when she publishes another package with the same slug and version, then
  the publish fails as "version exists" before any confirmation.
  (DIST-F-12)
- **A5 Unreleased provider.** Given a package whose provider has no release,
  when Alice publishes it, then the publish fails and names the provider.
  (DIST-F-13)
- **A6 Site folder.** Given Alice published two releases and withdrew one,
  when dist writes the site folder, then the folder holds exactly `head`,
  `entries/0` to `entries/2`, and the one package that is not withdrawn,
  and each file has the expected bytes. (DIST-F-17, DIST-F-19)
- **A7 Export keeps foreign files.** Given a folder with `index.html` and
  `CNAME`, when Alice exports into it, then both files are unchanged.
  (DIST-F-18)
- **A8 Export conflict.** Given a folder that holds Carol's channel, when
  Alice exports into it, then the export fails and names `head`.
  (DIST-F-20)
- **A9 Link round trip.** Given Alice set the site URL, when she asks for a
  link of `reading-list`, then the link text parses back to her key, the
  URL and the slug. (DIST-F-21)
- **A10 Follow and fetch from a link (J3, dist half).** Given the host
  serves Alice's site folder, when Bob opens Alice's link of
  `reading-list`, then Bob follows Alice, and he gets the package whose
  bytes equal Alice's package, with the release statement that Alice's
  channel holds. (DIST-F-22, DIST-F-31, DIST-F-33)
- **A11 Changed package.** Given Bob follows Alice and one byte of the
  package file on the host changed, when Bob fetches the release, then the
  fetch fails as a hash mismatch that names the URL, and Bob's package
  folder holds no file for it. (DIST-F-32)
- **A12 Changed statement.** Given one byte of `entries/1` on the host
  changed, when Bob follows the link, then the follow fails with an
  invalid signature at `entries/1`, and Bob holds no entry above seq 0.
  (DIST-F-24)
- **A13 Another key.** Given the host serves Carol's channel at Alice's
  URL, when Bob follows Alice's link, then the follow fails as "wrong
  channel" and Bob follows nothing. (DIST-F-24, DIST-F-23)
- **A14 Refresh reports.** Given Bob follows Alice, when Alice publishes
  1.1.0, withdraws 1.0.0 and copies her folder to the host, and Bob
  refreshes, then the report for Alice holds one new release (1.1.0) and
  one withdrawal (1.0.0, with Alice's reason), in this order.
  (DIST-F-25)
- **A15 Fork (J4 step).** Given Bob follows Alice at seq 2, when the host
  serves a channel with Alice's key and another statement at seq 1, and Bob
  refreshes, then Alice's channel is compromised, the report says so, a
  later fetch of any of Alice's releases fails as compromised, and a
  further refresh makes no request to the host. (DIST-F-26)
- **A16 Behind.** Given Bob holds Alice's channel at seq 2, when the host
  serves the state at seq 1, then the refresh reports "behind" and Bob
  still holds seq 2. (DIST-F-27)
- **A17 Limits.** Given the host sends a package one byte above the limit,
  or stops sending for longer than the timeout, when Bob fetches, then the
  fetch fails as "too large" or "timeout" and names the URL. (DIST-F-35)
- **A18 Offline refetch.** Given Bob fetched a release once, when the host
  stops, then a second fetch of that release succeeds. (DIST-F-34,
  DIST-F-36)
- **A19 Restore.** Given Alice's backup and her link, when a new home
  restores the backup, then a publish fails as "own head unconfirmed"
  before the app is asked. When the home confirms the own channel with
  the link and Alice publishes there, then the new statement has the next
  seq of her published channel, and Bob's refresh stores it with no fork.
  A link of another key leaves the channel unconfirmed. (DIST-F-3,
  DIST-F-4, DIST-F-26)
- **A20 Bad name.** Given the name `"Al\u{202E}ice"`, when Alice sets it,
  then dist refuses it before the app is asked. (DIST-F-9)

### Step 4

"Ivy" owns an index, "Carol" searches. Each home's site folder is served by
its own local host.

- **A21 Choose an index.** Given Ivy lists Alice's `reading-list`, when
  Carol chooses Ivy's link, then Carol holds Ivy's and Alice's verified
  channels, `follows.json` marks Ivy as an index and Alice as listed (not
  followed). (DIST-F-40)
- **A22 Search finds a verified mod.** Given A21, when Carol searches
  `reading`, then one hit is Alice's `reading-list` at its newest version,
  with Alice's profile name and `Via::Index` of Ivy with Ivy's note.
  (DIST-F-41)
- **A23 A listing cannot vouch.** Given Ivy lists Alice's key with a base
  that serves Mallory's channel, when Carol chooses Ivy, then the read of
  that base fails as "wrong channel" and no search gives a hit of it.
  (DIST-F-41, DIST-F-48)
- **A24 Withdrawn versions are hidden.** Given A21 and Alice withdraws her
  newest version, when Carol refreshes and searches, then the hit is the
  version before it; when Alice withdraws every version, there is no hit.
  (DIST-F-42)
- **A25 Freshness mark.** Given A21 and Alice's host stops, when Carol
  refreshes and searches, then the hit is still there, with
  `unchecked_since` equal to the time of the last read that succeeded.
  (DIST-F-42)
- **A26 One level of includes.** Given Ivy includes Jay, Jay lists Ken's
  mod and includes Lou, when Carol chooses Ivy, then the search gives Jay's
  mods and Ken's mod, nothing of Lou, and Lou's host gets no request.
  (DIST-F-43)
- **A27 Unlist.** Given A21 and Ivy unlists Alice's mod, when Carol
  refreshes, then no hit of Alice remains and her listed record is gone; a
  followed Alice stays. (DIST-F-40, DIST-F-46)
- **A28 A compromised index lists nothing.** Given A21, when Ivy's channel
  forks, then Carol's search gives no hit through Ivy's listings and Alice
  is no longer read. (DIST-F-26, DIST-F-40)
- **A29 Filters.** Given hits that offer, call and subscribe to
  `mods:reading-list/books@0.1.0`, when Carol searches by that interface
  with and without the version, and by Alice's key, then exactly the
  matching hits come back. (DIST-F-41)
- **A30 Own listings.** Given an app that refuses, when Ivy lists a mod,
  then nothing is signed. A list of her own channel, of a slug with no
  release, or of a mod already listed fails before the app is asked.
  (DIST-F-46, DIST-F-47)
- **A31 Cap.** Given Ivy lists 513 channels and `max_listed` is 512, when
  Carol chooses Ivy, then 512 channels are read and `skipped` is 1.
  (DIST-F-44)
- **A32 Offline search.** Given A21, when every host stops, then the
  search gives the same hits. (DIST-F-45)
- **A33 J4, dist half.** Alice publishes; Ivy lists it; Carol chooses Ivy,
  searches, follows the hit's link and fetches the release; Alice
  publishes 0.2.0 and Carol's refresh reports it; Alice withdraws 0.2.0
  and the refresh reports the withdrawal; a second statement at a used
  position marks Alice compromised and the hit carries that status.
  (DIST-F-40 to DIST-F-42, DIST-F-25, DIST-F-26)
