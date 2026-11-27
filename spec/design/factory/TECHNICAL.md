# Factory mod · technical design (draft)

## 1. Parts and sources of truth

The draft distinguishes **planning** (plan, sketch and Alice's dispatch
decision) from **foreman** work (run records, session references, evidence and
review rounds). Both are pages of **one combined factory mod**, for now (§5,
Door H, 2026-11-26). The mod has one identity, so the host sees one origin
client for every session either page starts, and one consent scope: the
consents Alice grants it (host §19.7) hold for both pages. Profiles narrow
each session (§5, planning and foreman split).

The mod's source lives in one Git repository of its own under W29. Its
`mod.toml` `repository` names it, so the signed package carries the link,
and JF2 changes the factory through that repository. The repository,
slug and publisher are chosen when the repository is made, not in this
design.

### 1.1 Manual sketch snapshot (JF1)

Planning exports one versioned JSON object as copyable text. Alice copies it
and pastes it into Foreman, previews the parsed fields and explicitly imports
it. Planning keeps the exported snapshot. Foreman stores its own immutable
copy. Import creates an independently identified run. There is no live link
or shared mutable draft between the pages.

| Field | Value |
|---|---|
| `version` | Integer `1`. |
| `project` | Canonical GitHub repository URL, `https://github.com/<owner>/<repo>`, without credentials, query, fragment, trailing slash or `.git`. Compare owner and repo case-insensitively. |
| `slice` | Nonempty slice identifier from the plan, used with `project` and `plan_revision` to locate the sketch. |
| `plan_revision` | Full commit object ID in that repository at which Alice read `factory/PLAN.md`. Planning asks Alice to supply it. |
| `base` | Full commit object ID from which Alice prepares the checkout, independently of `plan_revision`. |
| `branch` | Valid Git short branch name for the slice. |
| `scope` | Nonempty text describing the allowed change and its limits. |
| `acceptance` | Ordered array of one or more nonempty observable criteria. Their one-based positions are the criterion IDs. |
| `decisions` | Array of objects with `status` (`settled` or `open`) and nonempty `text`. Empty when there are none. |

Object IDs are full hexadecimal Git IDs (40 or 64 digits, normalized to
lowercase), with one object format per run. Planning limits the exported
snapshot to 32 KiB (32,768 bytes) of UTF-8 JSON. Foreman applies the same
limit to the entire pasted input, including whitespace, before parsing.
Import rejects oversized input, unknown versions, duplicate or unknown keys,
missing fields and malformed values. It identifies the problem and creates
no run. The plan binding is the repository, plan
commit and slice identifier recorded with the exact imported fields. Alice
compares them with the committed plan before dispatch and confirms the
preview before import. The pages do not fetch Git data or establish that the
commit exists. To change an imported sketch, dispatch and import a new
snapshot and abandon the superseded run. A snapshot with an open decision
can be imported and inspected. The run stays at Dispatched and the
implementer start action is refused while any decision is open. Foreman
shows the open decisions and asks Alice to dispatch and import a new
snapshot with all decisions settled.

### 1.2 Run and evidence

The run is durable mod state, separate from live page connections. It has a
stable mod-generated run ID, the snapshot, the human-confirmed absolute
checkout path once selected, the active evidence revision and attempt,
and the following records. The foreman uses the snapshot's scope, criteria
and decisions as the build brief, together with the project, base and branch.
Alice confirms that her dedicated checkout matches those values before
starting an implementer. The path is local run data and is omitted from
both the snapshot and reviewer input.

| Record | Required data |
|---|---|
| Session attempt | Local attempt ID, role, stored profile name, expected backend, actual backend and kernel session ID when known, request keys for pending calls, last processed transcript cursor, session status and any wait or failure reason. Review attempts also carry the evidence revision and exact input they were started with. |
| PR observation | Repository (must match the snapshot), positive GitHub PR number, target branch, source branch (must match the snapshot and originate in the same repository), full head object ID and observation time. When Alice records the open PR, Foreman checks that the target and source branches differ. The repository, number and target branch are fixed for this run once recorded. |
| Check evidence | On a PR observation, Alice supplies the set of project-required checks, plus a result per check with name, pass/fail/pending, and a command or CI run reference and short result summary. Include specialist gates where required. A missing result is pending. If there are no required checks, Alice explicitly confirms an empty set. |
| Review evidence | On that same observation, the diff text Alice supplies, the submitted brief and checks, the reviewing session ID and vendor, the transcript entry range containing the response, and a verdict with findings. The verdict is approve or request changes. Each finding gives its location (or whole-slice scope) and reason. Alice confirms the transcription of the verdict and findings against the response. |
| Merge receipt | Same repository and PR number, the approved PR head, full resulting merge commit ID and observation time. Alice confirms the repository reports the PR merged at that head before recording the receipt. The approved head and resulting commit are separate fields, even when equal. |

Each confirmed PR observation starts an evidence revision, numbered
monotonically within the run. Changing the head, diff or check evidence
starts a new revision, preserving the earlier bundle and its reviews.
Re-entering the identical active bundle is a no-op. After returning to
building, Alice must confirm a fresh observation, with a new evidence
revision even if the head is unchanged. An explicit fresh review at
the same evidence revision starts a new attempt and supersedes the older
attempt as the decision input. Compare observations without their entry
times for duplicate detection. Stop an active reviewer when its evidence
is superseded. A late response stays attached to its
original revision and attempt. Alice records the observed head before
review and rechecks it in the repository immediately before merging.
Evidence is Alice's report of Git and repository state. A session response
or an old observation cannot itself change that state or authorize a merge.

Alice can edit the sketch in Planning before dispatch. Foreman creates a
run only when she confirms import.

| Run state | Entry and allowed next action |
|---|---|
| Dispatched | Planning has exported the snapshot. Foreman import creates the run in this state. Alice can select the checkout and start the implementer once every decision in the snapshot is settled. |
| Building | Alice starts or retries an implementer on the confirmed checkout. On a usable result she inspects the diff and advances to checking. A confirmed request for changes returns here for a new attempt. |
| Checking | Alice applies the diff in her trusted checkout, opens or updates the PR, and supplies head, diff and checks. With complete passing evidence she can start review. A failed check can be replaced with a fresh observation or returned to building. |
| Reviewing | Alice starts the independent review for the current evidence. A confirmed request for changes returns to building and clears the active evidence for readiness. A confirmed approval with matching passing checks makes the run ready for Alice. A changed observation goes back to checking. |
| Ready for Alice | The current evidence revision has passing required checks and an approval from its latest review attempt on the other vendor. A changed observation goes back to checking. A fresh review removes readiness while it runs. Alice can use a head-guarded merge and record the matching receipt. |
| Merged | Foreman has recorded the matching receipt. Alice can record this result in Planning. This state is terminal. |
| Abandoned | Alice reconciles any unresolved start and cancels any live session before abandoning the run. Keep the record and its evidence. This state is terminal. |

Waiting or blocked is a status on the current state, with the affected
action and reason. Consent or a tool answer can leave a session waiting.
A refusal, lost session, failed check or unusable review leaves the last
evidence visible and offers the relevant retry, return to building, or
abandon action. A running attempt can be stopped without abandoning the
run. An uncertain start keeps the original request key until reconciled.
Persist the pending operation and key before sending it, then store the
returned session ID and recover its transcript by index on reload (§2).
A pending operation or unusable response cannot make a run ready.

The foreman serializes updates to each run and checks the active evidence
revision and attempt before applying a response or recording a receipt.
It rejects stale page actions and preserves the latest record. Planning
can display the completed run and its receipt under the saved snapshot
after Alice explicitly selects that run and records the result there.
This is a local record association following the manual forward handoff.
`factory/PLAN.md` remains the authoritative plan during JF1. Alice reconciles
the result there by hand.

## 2. Use the agent layer as written

Contracts are [host §19](../../../host/TECHNICAL.md#19-the-agent-layer-step-11)
and agentic-harness [§2](https://github.com/civic-tech-pioneer/agentic-harness/blob/main/design/TECHNICAL.md#2-interfaces)
and [§9](https://github.com/civic-tech-pioneer/agentic-harness/blob/main/design/TECHNICAL.md#9-the-kernel-m2).
Host §19.1 owns the exact host WIT. The factory is a client of
`mods:agent@0.1.0/sessions` (§2.3). The session-owning foreman uses
`check` for profile and overlay refusals, then `start` or
`start-in-folder` as appropriate. Starting in an existing folder uses
the picked-workspace flow (§4.6), which takes an absolute path and
requires a Claude profile as written. The session's origin client, the
factory mod, must hold the consents. Tool asks are presented to Alice and answered
through the session owner's `answer` call (§9.6). The foreman uses `read` and `attach` with the
last transcript cursor (§9.4), `say` for more work and `interrupt` or
`cancel` to stop. It keeps the request key for the same logical start
or input across uncertain delivery. When a stream is gone, it recovers
using the session ID and the kernel transcript. A lost agent session
or an uncertain external result needs reconciliation before another run.

Profiles name the backend, reach and limits. Kernel §9.1 denies a mod
with no `access/<identity>` entry. The first-run `scratch` exception
applies only to `page`. The current §2.3 API writes neither that access
entry nor the role profiles in `profile/<name>`. A profile offer alone
does not provision either. Until harness F's factory-profile work
answers who provisions the profiles and access and with whose consent
(§5), the foreman cannot start a session under the written contracts.
For JF1, select an authorized Claude implementer and start it with
`start-in-folder`. Select an authorized OpenAI reviewer with no built-in
tools, toolsets, skills or delegates and use `start` without a folder.
Both are top-level sessions. Alice selects from the profiles available
to this mod. Factory-profile setup must establish the role reach above.
The current `profiles` response lists names, backends and models (§2.3),
so the page uses the provisioned role bindings and checks the actual
backend in the owner-scoped session list before accepting evidence. The review input is the frozen brief,
PR observation, diff and checks, with a request for a verdict and findings.
Supplemental excerpts Alice supplies are recorded with the review attempt
and refer to the same head. If serialized input exceeds the harness limit
(256 KiB per call, §9.3), leave review pending and ask Alice for a smaller
slice or evidence package she can stand behind. Preserve the submitted
input; never silently truncate it.
The mod checks that both review and check evidence
refer to the revision Alice is about to merge. A new head invalidates
readiness. Alice uses a merge action guarded by the reviewed head,
so a concurrent push requires a new decision. On GitHub she can use
`gh pr merge <number> --repo <owner>/<repo> --match-head-commit <head>`
with the repository's permitted merge strategy and approvals. If the
repository cannot perform a guarded merge, leave the run pending. Alice
waits for the repository to report the completed merge before recording a
receipt. A receipt that fails to match stays unresolved for reconciliation.
Resuming a session or replaying a stream does not repeat a repository
action.

The kernel and providers own the host calls under host §19 and
agentic-harness §2.1. The current unchecked Claude provider calls only
`claude-code.start` (harness §10). The host specifies
`claude-code-shell.start` and its sandbox, but a shell-capable provider
path and the provider mod's shell grant are open prerequisites (§5).
OpenAI uses account-bound `https-stream`. The factory does not call provider-only
sessions or hold account credentials. If later roles use project
toolsets, the kernel opens the named toolset under `agent-tools` with
the origin client's consent. Each tool exposes a bounded operation
under the host's available capability and grant. An instruction or a
toolset name cannot supply filesystem, git, signing, or GitHub authority
by itself. Overlays narrow authority per §9.2. Under §9.7 a child is
narrower than its parent.

## 3. Map from the current factory

The current process is recorded in [factory/README](../../../factory/README.md)
and [the shared pipeline](https://github.com/civic-tech-pioneer/watane-mods/tree/main/factory).

| Today | Moves to the factory mod | Authority and work that stay outside |
|---|---|---|
| `factory/PLAN.md`, deputy's project selection, slice sketches, vendor assignment and decisions | Planning page, selected sketch and dispatch record. Surface Deputy decisions, Waiting on the human, Human answers, Upkeep and Leftovers with their current purpose. Carry the distinct-vendor rule, work lane, scope and escalation to Alice. | Keep `PLAN.md` and `factory/DONE.md` as migration inputs. The authoritative plan and history transfer stay open (§5). Alice settles product, project and security decisions. |
| `factory/config.toml` `[agents]`, `projects.toml`, deputy handover | Project and role selection, and a handover view of active slices, outstanding decisions and their evidence. | Existing project and vendor configuration remains available to the legacy dispatch process. Config entries do not confer profile access or consent. |
| Foreman skill, `factory/tasks/`, `impl.sh`, implementer instructions and ledger | Foreman run, briefs, role profiles, resumable sessions and evidence. An implementer prepares a diff in a dedicated checkout under the chosen N5 profile. | Checkout and branch preparation and Git credentials stay with Alice in JF1. The implementer has only its consented folder and bounded tools, with no push or merge credentials. |
| `check.sh`, `check-main.sh`, `cloud-setup.sh`, project CI and shared check scripts | The foreman shows required checks and associates results with the head. A later toolset may read checks from one selected repository under a separate consent. | Build and test commands and CI stay in the project or its runner. JF1 takes results from Alice. A transcript saying a check passed is not CI evidence. |
| Review lead, validator, user-sim and adversary roles | Foreman dispatches the independent review session, presents its findings and records the verdict against the head. Later profiles can add specialist checks where a slice requires them. | The reviewer gets evidence and no build authority. Specialist execution can stay in the existing pipeline until equivalent scoped sessions exist. Alice dispatches a new round or resolves an escalation. |
| Claude routines, Codex tasks, `watch.sh`, `watch-cloud.sh`, shared PR watchers | The host pages show current work. Kernel transcripts and session IDs replace the agents' out-of-band reporting in JF1. Later, a consented read-only PR toolset and host timers may collect status while the host runs. | External tasks and routines remain an available manual handoff. Host timers do not promise cloud wakeups, dispatch on another machine, or execution while the host is closed. CI and GitHub keep their own duties. |
| Merge gate, `gate` ruleset and “deputy compares” | Show which checks, user-sim acceptance, adversary result and visual comparisons the project requires, and their evidence at the head. | Existing repository rules and required approvals stay in force. Alice performs the deputy's visual comparison where required. JF1's review does not substitute for specialist gates. |
| Shared pipeline's edit-here rule, `factory/bin/sync.sh` and `adopt.sh` | JF2 replaces the factory's self-change workflow for the factory mod: propose a change in its source repository (W29), review it and install a package. | Pipeline edits for legacy projects still start in the shared source and use sync/adopt. Updating a mod does not update their copied scripts. |
| Deputy merge and plan update, publisher, installer | Foreman surfaces the matching checks and review for Alice; planning records her result. The same flow can prepare updates to factory sources. | Alice pushes, opens the PR and approves each merge via repository controls. Publication, signing, install and new reach stay in the normal watane release and confirmation path. |

Today's “nothing runs on the Mac” rule keeps build and review teams off
the deputy's machine. JF1 deliberately runs the implementer from Alice's
host on a dedicated checkout. Existing CI and specialist execution can
stay on their current runners.

A later review toolset may read the diff and checks for the selected PR and
head. A review comment writer, if ever proposed, would be a separate
decision and consent. The first build uses no repository toolset. In all
cuts, PR text, tool output and agent messages are input data. They cannot
choose a repository, widen a profile or authorize an action.

## 4. Authority and self-change

W39 and the confirmation floor apply. Ordinary mod grants and
host §19.7 consents remain the source of authority. The host keys account and
workspace consents by origin client and provider (`via`). Toolset
consent covers the origin client and named functions, without a
provider key. The host rechecks authority at apply. A checked provider's judge
verdict is a tool guard. It has no standing as the human's merge
decision or as the other vendor's code review.

The working release steers work on a separate source checkout. Alice inspects and applies the resulting
diff from a trusted checkout before she uses repository credentials.
The factory never selects the installed mod tree or host state as
a workspace and never writes them through an agent. It supplies no
signing keys or GitHub merge credentials. The Claude shell option uses
the prescribed sandbox and protected-path rules. A separate checkout
limits workspace selection and writes, with reads governed by the
actual sandbox. JF1 omits web tools and needs no added shell network
reach. Dependencies that need fetching can prevent an agent build or
test run. Alice must prepare dependencies in advance, run the checks
herself, or select a slice compatible with those limits. If a required check or edit needs access outside
them, return it to Alice. A shell command allow rule is not a sandbox
boundary. A stopped or revoked session must not be silently replaced with
one that has wider authority.

The ordinary package identity and verification rules apply to a candidate.
A record of the source revision and proposed grants helps Alice review
it. Existing approval applies only where the host's ordinary update
rules still authorize it. Added reach follows the ordinary trusted grant
and consent prompts. A merge, a valid signature and a factory review each
establish different facts. None supplies approval for installation or new
reach. A candidate can be rejected before installation, or disabled and
removed afterward. Compatibility and reinstatement have to follow the
ordinary package and state rules for the chosen implementation.

## 5. Open decisions

S120 recorded these choices. The human decided Door H on 2026-11-26.
S146 settled the planning and foreman split to match. S150 settles the
manual sketch handoff for JF1. The Door C settlements are for the
human's veto. The table records what remains open.

| Door and decision | Options and consequences | Recommendation |
|---|---|---|
| H · Where the mods live | W29 gives each mod with source its own Git repository. Two separately released mods therefore need one repository per mod. New repositories are new projects, with separate releases and cross-project coordination. One combined factory mod can use a single repository, with coupled releases and a shared identity. Putting two source mods in one repository would require revisiting W29. Moving later may require migrating source links, package identities and consents. | **Decided 2026-11-26 by the human, "for now": one combined factory mod in one new repository, W29 kept.** The repository holds the mod's source (W29); its name, the slug and the publisher are chosen when it is made. Isolation of the foreman's authority comes from harness profile narrowing at stage F, not a second mod (next row). Splitting later would migrate source links, package identities and consents, and reopens this row and the next. |
| C · Planning and foreman split | A combined mod has separate pages under one identity and one consent scope. Separate mods can isolate planning from agent-facing permissions, at the cost of handoff and release coordination. Harness VISION and AGT-J5 describe a product-owner session delegating to a foreman; using that shape would apply parent/child profile narrowing and requires F. | **Settled with Door H (S146), for the human's veto: two pages, one mod.** *One identity:* both pages act as the factory mod; every session either page starts has the mod as origin client, and the kernel's `access/<identity>` entry and the role profiles are for that one identity. *One consent scope:* an account, workspace or toolset consent granted to the mod holds for both pages, and Alice's prompt names the mod, not the page. Neither page holds a consent the other lacks, so the page split is no authority boundary. *Narrower foreman authority:* each session runs under the role profile it starts with (§2). In JF1 all sessions are top-level, with no delegation: the implementer's profile reaches only its consented folder, the reviewer's no checkout or tools. *Delegation, from F, only as harness [§9.7](https://github.com/civic-tech-pioneer/agentic-harness/blob/main/design/TECHNICAL.md#97-delegation-and-counters) writes it:* a planning (product-owner) session may delegate to a foreman as its child (AGT-J5) only when it is a root session the factory mod's `access/<identity>` entry authorizes, on the OpenAI backend, and it names an OpenAI child profile on the same model that passes §9.7's profile comparison (narrower than the parent's). The child is admitted through that delegation and needs no `access/<identity>` entry of its own; any other session is a top-level start, authorized directly. *Open until F:* who writes the role profiles and the access entry and with whose consent (factory profiles row), the delegation counters, and whether the planning page runs an agent session of its own. |
| C · Sketch handoff | Both pages share one mod identity. A manual snapshot makes the revision binding explicit and leaves an internal automatic link possible later. | **Settled for JF1 (S150), for the human's veto:** manual export, paste, preview and import of the versioned snapshot in §1.1. The automatic link stays open. |
| C · Authoritative plan during migration | Keep Git's `PLAN.md` authoritative and treat the mod copy as a working view, with manual reconciliation. Or make the factory mod's planning page authoritative after an explicit cutover, which requires exporting or archiving a coherent plan for legacy deputies and preserving decision history. | Use the Git plan during JF1 (§1.2). Ask the human before any cutover. |
| C · Factory profiles at F | Human provisioning in the kernel would give Alice explicit control but needs a supported interface for writing role profiles and `access/<identity>`. Adoption from an offer could reduce setup, but still needs a way for Alice to authorize the specific profile for a specific mod identity. Automatic provisioning by the requesting mod would change the consent boundary. Door H changes this row: there is one identity, the factory mod's, so one `access/<identity>` entry authorizes every top-level session either page starts, and the roles differ only by their profiles. A delegated child is admitted under §9.7 instead (planning and foreman split row). | Settle the writer, consent, scope and withdrawal in harness F before JF1-2 can start a session. Keep absent entries denied. |
| C · N5, implementer shell | The Claude shell option needs a shell-capable provider path to `claude-code-shell.start`, the `claude-code-shell` grant on the provider mod, and a human decision on V27. V27 found that allowlisted names can resolve to local addresses (harness §§4.2, 14, 16). With shell network closed, Alice may have to prefetch dependencies or run checks herself. `codex app-server` would be a third native process kind and needs a host contract and implementation for account, protocol, grants, sandbox and recovery. A file-tools-only Claude implementer could use the current `claude-code` provider without waiting for an N5 shell choice or V27. It would still need factory profiles and the E/F sequence, and could edit files but could not run or iterate on tests itself. | Compare the harness's Claude shell recommendation with the smaller file-tools-only option when selecting the first real slice. Leave the provider path, provider grant and V27 prerequisites open. |
| C · E/F sequencing | Harness §17 orders E before F. [Harness PLAN, Done (S127)](https://github.com/civic-tech-pioneer/agentic-harness/blob/5082f2095c3167a6fc71d8bfba73973549b6fa18/factory/PLAN.md#done) records E complete ([PR #56](https://github.com/civic-tech-pioneer/agentic-harness/pull/56), merged 2026-11-26). | Keep that sequence. JF1 session work still waits for F's factory profiles. |
| C · Update and authority | Propose source changes, then have Alice review, merge, publish or select the release, and install through the ordinary path. This costs explicit actions and works with existing controls. A richer update proposal could use the normal manager request path where available, but needs a separate authority review. Unattended activation or widening would change the confirmation boundary. Door H changes this row: the pages are released together, so an update changes both, and one candidate's grants cover both pages. | Use ordinary updates with explicit human control. Defer any manager grant. Keep an old release available through normal package mechanisms where possible. |
| C · V7 statements and sequencing | Local notes and GitHub evidence suffice to exercise the draft journey but carry GitHub's trust and availability assumptions. Portable signed statements would let another person verify the slice intent and scope, reviewer verdict at a head, owner merge decision, and release provenance and requested authority against a chosen signer. This requires decisions about signers, visibility, subject identity, supersession and withdrawal. Requiring all statements first delays the real journey but avoids a migration from local evidence. | Agree which statements step 6 requires before building, and whether a supervised GitHub-backed first cut can precede them under VISION. Prefer explicit subject and revision identities and signatures made by people through approved controls. Until the human agrees, the V7 dependency stands. Never publish transcripts or credentials as statements by default. |

The V7 here is watane's vision goal. The harness's stage E probe also
called V7 measures judge latency and remains a separate prerequisite
for that stage. A V7 statement cannot authorize a merge or grant by replay.

## 6. Build order after this design

### JF1 build slices

Harness E's checked provider landed in S127. The outstanding agent-layer
gate for JF1 is F's factory-profile provisioning and authorization for
the factory identity (harness S151 sketches the design). STEP6-REPO is a
separate requirement for the first build slice. The human creates the
repository under W29 and names the publisher. Creating it does not make
the profiles available. This design can be completed before either lands.

| Slice | Deliverable and acceptance | Waits on |
|---|---|---|
| JF1-1 · Plan and hand off | In the factory repository, install the combined mod and use both pages to dispatch, preview, import and reload a real plan snapshot. Reject malformed input and keep the imported brief immutable. FUNCTIONAL JF1 outcome 1. | STEP6-REPO and the chosen mod identity. This row can build without F once the repository exists. |
| JF1-2 · Build and record | From the imported run, select the authorized Claude profile and dedicated checkout, launch and recover the implementer, surface consent and stop cases, and record Alice's PR, diff and check evidence. FUNCTIONAL JF1 outcomes 2–3 and the implementer failure cases in outcome 6. | JF1-1 and F's factory profiles. The planned Claude shell path additionally needs the supported shell provider and grant and the human's V27 disposition. Use the implementer path actually authorized for JF1 (§5). |
| JF1-3 · Review and close | Start the independent OpenAI review on the submitted evidence, handle change requests and stale heads, and bring a real PR through Alice's guarded merge to a recorded receipt. FUNCTIONAL JF1 outcomes 4–6. | JF1-2, F's authorized tool-free OpenAI profile, Alice's account and the applicable V7 sequencing decision (§5). Use a project whose required gates and head-guarded merge are available to Alice. |

N5's recorded human preference is Claude with the shell-and-web grant
for now (harness PLAN, Human answers, 2026-11-25). JF1 requests no web
tools or shell network reach. The existing provider contract still
requires the shell path to be delivered and the safety disposition in
§5 to be settled. A file-tools-only implementer remains an option for
the human if JF1 should proceed before those shell prerequisites. JF1
uses no delegation, repository toolset or checked-provider verdict.

### After JF1

1. **Checked roles and richer evidence.** A role that relies on checked
   Claude tools waits on E, including the judge account and the measured
   check budget. Add a scoped read-only evidence toolset only when its
   provider and consent can use the existing host contract. A judge
   failure leaves the checked action blocked. Neither an unchecked
   fallback nor the judge is part of the cross-vendor review.
2. **Delegated rounds.** F supplies delegation and counters. Use them for
   bounded work within a role where the parent's profile permits it,
   including a planning session delegating to a foreman as its child
   under the §9.7 conditions of §5's planning and foreman split row: an
   authorized OpenAI root session, a named OpenAI child profile on the
   same model that passes the profile comparison. The child needs no separate
   `access/<identity>` entry. Keep the independent review a separately selected top-level role on the
   other vendor. A later unattended dispatch decision would need its own
   scope and consent. Parallel work also depends on a worktree workspace
   design (harness FUNCTIONAL §5). JF1 uses one checkout prepared by Alice.
3. **Self-change · JF2.** Build on JF1 and the ordinary publication and
   installation paths, with the factory mod's own repository as the
   source under change. Requires the approved updates and V7 decisions
   from §5. It inherits JF1's E and F dependencies. Pass JF2 on a real
   factory candidate before calling step 6 complete.
