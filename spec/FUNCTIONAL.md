# Functional specification: watane

What watane does, without technology. Each area serves one step of the
order of work in `VISION.md`. `TECHNICAL.md` says how.

## 1. Terms

| Term | Meaning |
|---|---|
| mod | The unit that a person makes, installs and shares. It has a page, a logic part, or both. |
| page | The part of a mod that the person sees and uses. |
| logic part | The isolated part of a mod that computes and holds its state. |
| interface | A named, versioned set of calls that a mod offers to other mods. |
| event | A message that a mod sends to the mods that subscribe to it. |
| grant | One authority that a mod asks for and the participant approves. |
| host | The program that runs the installed mods together. |
| app | The desktop application around the host. The participant gives every approval there. |
| maker tools | The watane commands with which a maker and a coding agent create, check, test, preview and publish mods. |
| coding agent | An agent of the maker's choice, outside watane, that writes the code of mods. |
| preview | A host that runs a mod for its maker, with no identity and no data of the participant. |
| toolset | A named list of calls of installed mods that the participant gives to an agent. |
| participant | A person with an identity in watane. |
| identity | A key pair that signs the statements of the participant. |
| channel | The public, ordered, append-only record of the signed statements of one participant. |
| package | One immutable version of a mod, as a file. |
| release | A statement in a channel that publishes one package. |
| index | A channel that lists mods, also of other participants, and that a participant chose to search. |

## 2. Requirements

### 2.1 Make (step 1)

- **F-MAKE-1** The maker tools create a complete mod from a template: a page, a logic part, its interface and its tests.
- **F-MAKE-2** The maker tools check a mod against the watane constraints, build it and run its tests.
- **F-MAKE-3** The tests include contract tests, which use a fake for each interface that the mod imports.
- **F-MAKE-4** The maker tools run a mod in a preview. The preview runs the mod in the same host as an installed mod, with exactly the grants that the mod declares.
- **F-MAKE-5** Each command of the maker tools gives a result that a coding agent can read and act on.
- **F-MAKE-6** The maker tools hold no key and need no account.

### 2.2 Run together (step 2)

- **F-RUN-1** The host accepts as a mod only a package with at least one part, and with at most one page and one logic part.
- **F-RUN-2** The host isolates each logic part from the device, from the host and from other mods. A logic part has only the grants that the participant approved.
- **F-RUN-3** The host limits the time and the memory of each call and stops a call that exceeds a limit.
- **F-RUN-4** The host isolates each page from the host and from other pages. A page calls only the logic part of its own mod.
- **F-RUN-5** A mod calls an interface of another mod only if it declared the call and the participant granted it.
- **F-RUN-6** The host delivers an event only to the mods that the participant granted the subscription. The sender does not choose or see its subscribers.
- **F-RUN-7** The grant kinds are: private storage, network access to named hosts, calls to a named interface of a named provider, and subscriptions to the events of a named provider.
- **F-RUN-8** Each mod has private state. A failed call changes no state.

### 2.3 Install (step 2)

- **F-INST-1** Before an install, watane shows a plan: the mod, its publisher or "local", the grants it asks for, and the provider mods it needs.
- **F-INST-2** watane installs a mod only after the participant approves the plan, and only with the approved grants. The participant can withhold each grant.
- **F-INST-3** A mod that was made on this device installs without a signature. watane marks it as local.
- **F-INST-4** watane installs the provider mods of a mod first.
- **F-INST-5** The participant can open, disable, enable and remove each mod, and withdraw each grant at any time. A removal asks whether to delete the data of the mod.
- **F-INST-6** watane installs a package of another participant only after it verified the signature of the release and the digest of the package. (step 3)
- **F-INST-7** watane installs an update without a question only if the update asks for no new grant.
- **F-INST-8** When a publisher withdraws an installed version, watane tells the participant and offers the newest version that is not withdrawn. (step 4)

### 2.4 App (step 2)

- **F-APP-1** The app starts empty. It shows only the launcher until the participant adds a mod.
- **F-APP-2** At the first start, the app creates the identity of the participant, with no contact to a server. (step 3)
- **F-APP-3** The app shows the page of each mod in its own view, with the name of the mod and its publisher.
- **F-APP-4** Each approval happens in a consent view of the app, which no mod can cover or imitate.
- **F-APP-5** The app shows, for each mod, its version, its publisher, its grants and its origin.
- **F-APP-6** The participant can save the identity to a backup file and restore it. After a restore, watane signs nothing until the participant has given the link to the own channel, or has stated that the identity never published. (step 3)

### 2.5 Agents (steps 1 and 2)

- **F-AGENT-1** The participant defines toolsets. An agent reaches only the calls of its toolset.
- **F-AGENT-2** An agent uses a toolset only with a token. The host gives a token only when the participant asks for one, and the participant can end it.
- **F-AGENT-3** Each tool has a name that identifies its mod, so no mod can hide or replace a tool of another mod.
- **F-AGENT-4** **The confirmation floor.** No agent, tool or command installs a mod, approves a grant, or signs a statement without the confirmation of the participant in the app.

### 2.6 Publish (step 3)

- **F-PUB-1** Each participant has one channel.
- **F-PUB-2** When the participant publishes a mod, watane adds a release with the name, the version and the digest of the package. A published version never changes.
- **F-PUB-3** watane writes the channel and the packages as plain files. The maker puts them on any static web host. That host is not trusted.
- **F-PUB-4** Where the maker agrees, the package includes the source of the mod.
- **F-PUB-5** The participant can withdraw a version, with a reason.
- **F-PUB-6** The participant can follow a channel from a link. watane keeps a followed channel up to date and verifies each statement.
- **F-PUB-7** If two validly signed statements of one channel cannot both be in it (two at the same position, or two that follow the same statement), watane marks the channel as compromised and stops updates from it.

### 2.7 Find (step 4)

- **F-FIND-1** An index lists releases of its owner and mods of other participants, and it can include other indexes. Includes count one level deep.
- **F-FIND-2** The participant chooses the indexes that watane reads, each from a link. watane has no index of its own.
- **F-FIND-3** The participant searches the chosen indexes by text, by publisher, and by the interfaces that a mod offers or uses.
- **F-FIND-4** watane shows a result only after it verified the release against the channel of its publisher. It hides a withdrawn version, and it marks a result whose withdrawals it could not check.

### 2.8 Build on (step 5)

- **F-BUILD-1** The maker tools make a new mod from the source of a published mod. The new mod records the release it came from, and watane shows this origin.
- **F-BUILD-2** The maker tools add a dependency on an interface of a published mod, and make that interface available to the coding agent.
- **F-BUILD-3** When no mod of the named provider is available, watane shows other mods whose interface fits, and binds one only after the participant picks it.
- **F-BUILD-4** watane shows the mods that derive from a mod, as far as the chosen indexes list them.

### 2.9 Quality

- **F-QUAL-1** Every function works offline, except reading a channel or an index from its web host.
- **F-QUAL-2** watane runs on macOS first and on Linux second.
- **F-QUAL-3** watane never hides a failed verification. It names the cause.

## 3. Journeys

One journey for each step. A step is done when its journey passes as a test
on the real programs.

### J1: Make a mod

Alice asks her coding agent for a reading list. The agent creates the mod
with the maker tools and writes the code. A contract test fails; the agent
reads the result, fixes the code, and the tests pass. The agent starts a
preview. Alice tries the page, asks for one change, and sees it in the
preview. The agent reads the list through a toolset of the preview.

### J2: Use mods together

Alice starts the app for the first time. It shows an empty launcher. She
installs her reading list and a recommendations mod
that calls the reading list and subscribes to its events. The plan shows
both as local, with their grants, and the reading list first. Alice
approves. She adds a book in one mod, and the other mod reacts. After a
restart, the data is still there. She withdraws the call grant, and the
recommendations mod reports the missing grant. She removes a mod.

### J3: Publish and install from a link

Alice's app holds her identity, which it created with no server. Alice
publishes the reading list. The app asks, and she confirms. She puts
the written files on a static web host and shares the link with Bob. Bob
follows the link. His plan shows Alice as the publisher, with a verified
signature. He installs the mod and uses it. When a file on the web host is
changed, Bob's watane refuses it and names the cause.

### J4: Find a mod

Ivy shares the link of her index, which lists Alice's mod. Carol adds it
as an index. She describes her need in a few words, and the search shows
the mod, verified, with Ivy's note. She installs it. Alice releases a new
version with no new grant, and Carol's watane updates it without a
question. Alice withdraws that version, and Carol's watane tells her. A
second statement at a used position marks Alice's channel as compromised.

### J5: Build on a mod

Carol's coding agent makes a ratings mod from the source of Alice's mod.
Carol publishes it, and its origin is visible. Dave's agent adds a
dependency on the interface of Carol's mod; his contract tests use a fake
of it. Eve installs Dave's mod, and watane installs Carol's mod first. Where
Carol's mod is not available, watane shows another mod whose interface
fits, and binds it after Eve picks it.

## 4. Later

These are not in the first version (see `VISION.md`):

- audits, auditors and trust policies;
- a tool grant for each agent, a confirmation setting for each tool, and a
  record of tool calls;
- direct transfer between devices, and helper services;
- the factory, and statements between participants;
- recovery of a lost identity without a backup.
