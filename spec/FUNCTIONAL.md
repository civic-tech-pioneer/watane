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
| collect | A question that a mod asks all the mods that subscribe to it at once. It gets back their answers in one list. (step 9) |
| grant | One authority that a mod asks for and the participant approves. |
| host | The program that runs the installed mods together. |
| app | The desktop application around the host. The participant gives every approval there. |
| main mod | The mod that the app opens at start. The participant chooses it among the installed mods with a page. |
| manager | The view where the participant sees, finds, updates and controls the installed mods. The app has one; from step 10 a mod can be another. |
| maker tools | The watane commands with which a maker and a coding agent create, check, test, preview and publish mods. |
| coding agent | An agent of the maker's choice, outside watane, that writes the code of mods. |
| preview | A host that runs a mod for its maker, with no identity and no data of the participant. |
| toolset | A named list of calls of installed mods that the participant gives to an agent. |
| client | The mod, or its page, that starts an agent session and reads it. It is the session's owner. (step 11) |
| profile | A named, complete setting of an agent: instructions, model, tools, permissions, skills, subagents, limits and output format. The participant makes it. (step 11) |
| agent session | A conversation between a mod and an agent, with its turns and its stored transcript. It lives until the mod that started it deletes it. (step 11) |
| kernel | The mod `mods:agent`: it keeps the profiles, the sessions and their transcripts, the permissions and the delegation of agents. (step 11) |
| provider | The mod that holds the Claude Code grant and starts the participant's `claude` program for the kernel: unchecked, or checked by a judge. (step 11) |
| judge | A service that the participant chose, which reads an agent step and says whether it is safe. (step 11) |
| workspace | A folder in which agents may read and change files: one that the participant picked for a client mod, or the scratch folder of one session. (step 11) |
| origin client | The mod, or its page, at the start of a chain of calls that ends in an agent. A consent names it. (step 11) |
| participant | A person with an identity in watane. |
| identity | A key pair that signs the statements of the participant. |
| channel | The public, ordered, append-only record of the signed statements of one participant. |
| package | One immutable version of a mod, as a file. |
| release | A statement in a channel that publishes one package. |
| mod repository | A public git repository with the source of one mod. The mod names it. |
| index | A channel that lists mods, also of other participants, and that a participant chose to search. |
| own machine | A machine of the participant, paired with the home that holds the identity. It holds no identity of its own. (step 8) |
| full machine | An own machine that keeps everything that any own machine holds. (step 8) |
| light machine | An own machine that keeps only what it uses, and asks a full machine for the rest. (step 8) |

## 2. Requirements

### 2.1 Make (step 1)

- **F-MAKE-1** The maker tools create a complete mod from a template: a page, a logic part, its interface and its tests.
- **F-MAKE-2** The maker tools check a mod against the watane constraints, build it and run its tests.
- **F-MAKE-3** The tests include contract tests, which use a fake for each interface that the mod imports.
- **F-MAKE-4** The maker tools run a mod in a preview. The preview runs the mod in the same host as an installed mod, with exactly the grants that the mod declares, except the Claude Code grants and the participant's accounts: the maker tools offer a fake `claude` and a scripted OpenAI stream instead (step 11).
- **F-MAKE-5** Each command of the maker tools gives a result that a coding agent can read and act on.
- **F-MAKE-6** The maker tools hold no key and need no account.

### 2.2 Run together (step 2)

- **F-RUN-1** The host accepts as a mod only a package with at least one part, and with at most one page and one logic part.
- **F-RUN-2** The host isolates each logic part from the device, from the host and from other mods. A logic part has only the grants that the participant approved.
- **F-RUN-3** The host limits the time and the memory of each call and stops a call that exceeds a limit.
- **F-RUN-4** The host isolates each page from the host and from other pages. A page calls only the logic part of its own mod.
- **F-RUN-5** A mod calls an interface of another mod only if it declared the call and the participant granted it.
- **F-RUN-6** The host delivers an event only to the mods that the participant granted the subscription. The sender does not choose or see its subscribers.
- **F-RUN-7** The grant kinds are: private storage, network access to named hosts, calls to a named interface of a named provider, subscriptions to the events and collects of a named provider, (step 10) the management of mods, and (step 11) starting Claude Code, with or without programs and the web.
- **F-RUN-8** Each mod has private state. A failed call changes no state.
- **F-RUN-9** A mod can collect: the host asks each mod that the participant granted the subscription and gives the asking mod all their answers in one list. The asking mod does not choose or see who answered. An answer that fails is left out and does not fail the collect, unless it uses up the time or the fuel of the whole call. (step 9)

### 2.3 Install (step 2)

- **F-INST-1** Before an install, watane shows a plan: the mod, its publisher or "local", the grants it asks for, and the provider mods it needs.
- **F-INST-2** watane installs a mod only after the participant approves the plan, and only with the approved grants. The participant can withhold each grant.
- **F-INST-3** A mod that was made on this device installs without a signature. watane marks it as local.
- **F-INST-4** watane installs the provider mods of a mod first.
- **F-INST-5** The participant can open, disable, enable and remove each mod, and withdraw each grant at any time. A removal asks whether to delete the data of the mod.
- **F-INST-6** watane installs a package of another participant only after it verified the signature of the release and the digest of the package. (step 3)
- **F-INST-7** watane installs an update without a question only if the update asks for no new grant.
- **F-INST-8** When a publisher withdraws an installed version, watane tells the participant and offers the newest version that is not withdrawn. (step 4)
- **F-INST-9** A mod manages mods only with the grant to manage them. With it, the mod sees the installed mods, their grants and how they connect. (step 10)
- **F-INST-10** A managing mod can make things do less on its own: disable a mod, withdraw a grant, close a page. (step 10)
- **F-INST-11** A managing mod only asks for anything more: an install, an update, a grant, a pick of a provider, or a removal that deletes data. The participant decides each request in the consent view, as if they had started it in the app. A managing mod never signs, never answers a consent and never reaches the identity. (step 10)

### 2.4 App (step 2)

- **F-APP-1** The app starts empty. At start it opens the main mod; with no main mod, it opens the manager.
- **F-APP-2** At the first start, the app creates the identity of the participant, with no contact to a server. (step 3)
- **F-APP-3** The app shows the page of each mod in its own view, with the name of the mod and its publisher.
- **F-APP-4** Each approval happens in a consent view of the app, which no mod can cover or imitate.
- **F-APP-5** The app shows, for each mod, its version, its publisher, its grants and its origin.
- **F-APP-7** The participant can make any installed mod with a page the main mod, and can unset it. When none is set, a mod that declares itself a main view becomes the main mod when the participant installs it. The manager is always one menu command away.
- **F-APP-8** The manager holds only the mods: the installed ones and how they connect, finding new ones, and their updates. The identity, the agents and the own machines are settings of the app, outside the manager.
- **F-APP-9** The manager shows, for each mod, the mods it calls, the mods it receives events from or answers, and the mods that use it. Before the participant disables or removes a mod that other mods use, the manager names them.
- **F-APP-6** The participant can save the identity to a backup file and restore it. After a restore, watane signs nothing until the participant has given the link to the own channel, or has stated that the identity never published. (step 3) From step 8, a full own machine may also give the newest own statement; the participant still confirms it.

### 2.5 Agents (steps 1, 2 and 11)

- **F-AGENT-1** The participant defines toolsets. An agent reaches only the calls of its toolset.
- **F-AGENT-2** An agent uses a toolset only with a token. The host gives a token only when the participant asks for one, or when the participant consents to a toolset for a client mod (an agent's handle; step 11, agentic-harness 6 and 8), and the participant can end it.
- **F-AGENT-3** Each tool has a name that identifies its mod, so no mod can hide or replace a tool of another mod.
- **F-AGENT-4** **The confirmation floor.** No agent, tool or command installs a mod, approves a grant, or signs a statement without the confirmation of the participant in the app.
- **F-AGENT-5** With the participant's consent, a mod can run agent sessions on the participant's own Claude or ChatGPT subscription. watane holds no key and runs no service for it. (agent layer)
- **F-AGENT-6** An agent session is private to the client that started it. The client may publish what it chooses.
- **F-AGENT-7** The participant decides, for each client mod, the folders, the plan and the toolsets its agents may use, sees them, and withdraws each one.
- **F-AGENT-8** The participant defines the profiles of agents. A client can only narrow a profile.
- **F-AGENT-9** The participant can pick a provider that checks each agent step with a judge of their choice. A check that cannot be done denies.

### 2.6 Publish (step 3)

- **F-PUB-1** Each participant has one channel.
- **F-PUB-2** When the participant publishes a mod, watane adds a release with the name, the version and the digest of the package. A published version never changes.
- **F-PUB-3** watane writes the channel and the packages as plain files. The maker puts them on any static web host. That host is not trusted.
- **F-PUB-4** Where the maker agrees, the package includes the source of the mod, and the mod has its own repository with that source, tagged for each release. (repository: step 6)
- **F-PUB-5** The participant can withdraw a version, with a reason.
- **F-PUB-6** The participant can follow a channel from a link. watane keeps a followed channel up to date and verifies each statement.
- **F-PUB-7** If two validly signed statements of one channel cannot both be in it (two at the same position, or two that follow the same statement), watane marks the channel as compromised and stops updates from it.
- **F-PUB-8** The maker tools push the site to a GitHub repository of the maker and each mod to its own GitHub repository, with the maker's own git login. The link reads the site from the repository's raw URL by default; GitHub Pages or any other static host also works. (step 6)

### 2.7 Find (step 4)

- **F-FIND-1** An index lists releases of its owner and mods of other participants, and it can include other indexes. Includes count one level deep.
- **F-FIND-2** The participant chooses the indexes that watane reads, each from a link. watane has no index of its own.
- **F-FIND-3** The participant searches the chosen indexes by text, by publisher, and by the interfaces that a mod offers or uses.
- **F-FIND-4** watane shows a result only after it verified the release against the channel of its publisher. It hides a withdrawn version, and it marks a result whose withdrawals it could not check.

### 2.8 Build on (step 5)

- **F-BUILD-1** The maker tools make a new mod from the source of a published mod. Where the mod has a repository, the new mod is a fork of it (step 6). The new mod records the release it came from, and watane shows this origin.
- **F-BUILD-2** The maker tools add a dependency on an interface of a published mod, and make that interface available to the coding agent.
- **F-BUILD-3** When no mod of the named provider is available, watane shows other mods whose interface fits, and binds one only after the participant picks it.
- **F-BUILD-4** watane shows the mods that derive from a mod, as far as the chosen indexes list them.

### 2.9 Own machines (step 8)

- **F-OWN-1** watane connects to other machines only after the participant turned this on. With it off, every other function works as before.
- **F-OWN-2** The participant pairs a machine with a one-time code from the home that holds the identity. The home shows the new machine's short key, and pairs it only after the participant confirms. A code is valid for 10 minutes and for one use.
- **F-OWN-3** A paired machine holds no identity and signs no statement. Each machine is full or light, as the participant chose.
- **F-OWN-4** Own machines keep each other current directly: a new statement or package on one reaches the others that keep it, with no static host between them. A machine that was offline catches up when it comes back.
- **F-OWN-5** A light machine searches and installs through a full machine, and verifies every result itself (F-FIND-4).
- **F-OWN-6** The participant can remove a machine. The other machines stop trusting it once they learn of the removal. The participant can also set the list of machines to expire after a number of days; by default it never expires.
- **F-OWN-7** No endpoint that someone else runs is used unless the participant chose it: not a relay, not the DHT. Where the machines cannot reach each other without one, watane says so.
- **F-OWN-8** watane publishes nothing that ties the participant to a machine or an address, unless the participant turned on a function that names it, such as the DHT.

### 2.10 Get watane (step 7)

- **F-GET-1** One command that a person pastes in a terminal puts the programs `watane`, `watane-dev` and `watane-node` and the desktop app on a Mac, in the person's own folders, with no root, no account and no checkout.
- **F-GET-2** The install verifies the download against the checksums of the same release before it writes a file, and installs nothing when they differ.
- **F-GET-3** The install offers to install the Rust toolchain that the maker tools need, with a question that the person answers; it installs it only after a yes. With no terminal to ask on, it installs nothing and prints the commands. The maker tools themselves install nothing (DEV-F-13).
- **F-GET-4** The same command upgrades an installed watane. One command removes it and keeps the participant's home unless the participant types a word that deletes it.
- **F-GET-5** The install tells the person how to open the app although it is not signed by an Apple developer, and sends nothing about the person or the machine.

### 2.11 Quality

- **F-QUAL-1** Every function works offline, except reading a channel or an index from its web host, and reaching another machine (step 8).
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

Alice starts the app for the first time. It shows an empty manager. She
installs her reading list and a recommendations mod
that calls the reading list and subscribes to its events. The plan shows
both as local, with their grants, and the reading list first. Alice
approves. She adds a book in one mod, and the other mod reacts. After a
restart, the data is still there. She withdraws the call grant, and the
recommendations mod reports the missing grant. She removes a mod.

### J3: Publish and install from a link

Alice's app holds her identity, which it created with no server. Alice
publishes the reading list. The app asks, and she confirms. watane-dev
pushes the site to her GitHub repository and the source to the reading
list's own repository. She shares the link with Bob. Bob
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

Carol's coding agent forks the repository of Alice's mod and makes a
ratings mod from it.
Carol publishes it, and its origin is visible. Dave's agent adds a
dependency on the interface of Carol's mod; his contract tests use a fake
of it. Eve installs Dave's mod, and watane installs Carol's mod first. Where
Carol's mod is not available, watane shows another mod whose interface
fits, and binds it after Eve picks it.

### J7: Get watane

Alice has a Mac and has never installed Rust. She pastes one line from the
watane release page into a terminal. It downloads the release, checks it
against the release's checksums, and puts `watane`, `watane-dev` and
`watane-node` in `~/.local/bin` and `watane.app` in `~/Applications`. It
asks whether to install the Rust toolchain for the maker tools, and she
answers yes. It tells her how to open the unsigned app if macOS asks, and, if her Mac lacks the macOS command line tools, the one command that installs them. She
opens the app, and her coding agent runs `watane-dev new reading-list`
straight after. A week later she pastes the line again and watane is
upgraded, with her home as it was. A corrupted download installs nothing.
An uninstall removes the programs and the app, and asks before it would
delete her home.

### J8: Own machines

Alice turns on her own machines in the app. She runs `watane-node` on a
home server on her network and pairs it as a full machine with a code;
the app shows the server's short key, and she confirms. A second use of
the code is refused. She publishes a release on her laptop; the server
has it within seconds, with no static host. She pairs a second laptop as
a light machine; it finds and installs Ivy's listed mod through the
server, and verifies it. The server serves a changed byte, and the
second laptop refuses it and names the server. Alice removes the second
laptop; the server stops serving it. Alice restores her identity on a new
laptop, pairs the server again, and the new laptop learns her newest
statement from it; she confirms it before she publishes. No relay and no
DHT was used.

### J9: Extend a shell

Alice installs a shell mod and two mods that add commands to it. The plan
shows, for each command mod, the grant to answer the shell. The shell
collects and shows both commands; it does not learn which mod gave which.
Alice clicks one, and the mod that gave it acts. One command mod fails
when asked; the shell still shows the other command. Alice withdraws one
mod's grant, and its command is gone at the next collect. A command
mod that also calls the shell is refused at the plan, which names the
cycle.

### J10: Manage from a shell

Alice installs Desk, a shell that declares itself a main view and asks to
manage her mods. The plan shows that grant in words. At the next start the
app opens Desk, not the manager. Desk lists her mods with their grants and
connections. From Desk, Alice turns Weather off, and it stops at once.
From Desk she asks to install Book club, which her index lists: the
consent view shows the plan and that Desk asked, and Desk cannot approve
it. Alice approves, and Desk sees Book club installed. Desk asks to remove
Notes with its data, and the consent view asks Alice. Alice withdraws
Desk's grant to manage; Desk still runs, and its next request fails. "Mods…" in the app menu opens the
manager of the app at any time.

### J11: Run agents from a mod

Alice installs Notes and the agent kernel. The plan of Notes says it uses
agents; the plan of the Claude Code provider says it starts Claude Code with
her account, unchecked. In the kernel's page she makes a profile "Tidy":
Sonnet, file tools, no shell, her own instructions, and lets Notes use it.
She asks Notes to tidy her notes folder. The app asks whether Notes may have
agents work in that folder with "Tidy" through the provider; she agrees. The
agent's text streams into Notes as it works. The agent asks to delete a file;
Notes shows the ask; Alice denies it, and the agent goes on without it.
She quits watane mid-turn and starts it again. Notes shows the session as
not running, with everything up to the last finished message. She sends
"continue", and the agent resumes where Claude left off. In the manager she
withdraws the folder consent, and Notes' next session there asks again.

## 4. Later

These are not in the first version (see `VISION.md`):

- audits, auditors and trust policies;
- a tool grant for each agent, a confirmation setting for each tool, and a
  record of tool calls;
- direct transfer between friends and strangers, and helper services (`PEERS.md` 10);
- the factory, and statements between participants;
- recovery of a lost identity without a backup;
- finding mods through the search of a code host, such as a GitHub topic.
