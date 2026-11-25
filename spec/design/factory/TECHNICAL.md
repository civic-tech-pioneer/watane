# Factory mods · technical design (draft)

## 1. Parts and sources of truth

Two mod responsibilities are needed. The **planning mod** owns the editable
plan, the sketch and Alice's dispatch decision. The **foreman mod** owns
run records, session references, evidence and review rounds, and shows
their connection to the slice. A reviewed sketch is handed over as a
snapshot. For JF1 Alice may carry that snapshot between the two pages.
This needs no new link contract or shared storage. A later mod-to-mod
link can carry the same snapshot over existing call links after its
contract is agreed.

The sketch identifies the project, base, branch, scope, acceptance and
decisions. The foreman associates the snapshot with the session IDs and
records evidence against repository, PR and head revision. The record
moves through sketch, dispatched, building, checking, reviewing, ready
for Alice, and merged or abandoned. Changes requested go back to building.
A blocked step keeps its evidence and reason. Git and the repository
service own commits, check status, PR state and merge results. In JF1
Alice confirms these readings and copies them in. A mod's record of a
merge is a report of her action.

## 2. Use the agent layer as written

Contracts are [host §19](../../../host/TECHNICAL.md#19-the-agent-layer-step-11)
and agentic-harness [§2](https://github.com/civic-tech-pioneer/agentic-harness/blob/main/design/TECHNICAL.md#2-interfaces)
and [§9](https://github.com/civic-tech-pioneer/agentic-harness/blob/main/design/TECHNICAL.md#9-the-kernel-m2).
Host §19.1 owns the exact host WIT. The factory is a client of
`mods:agent@0.1.0/sessions` (§2.3). The session-owning foreman uses
`check` for profile and overlay refusals, then `start` or
`start-in-folder` as appropriate. Starting in an existing folder uses
the picked-workspace flow (§4.6), which takes an absolute path and
requires a Claude profile as written. The session's origin client
must hold the consents. Tool asks are presented to Alice and answered
through the session owner's `answer` call (§9.6). The foreman uses `read` and `attach` with the
last transcript cursor (§9.4), `say` for more work and `interrupt` or
`cancel` to stop. It keeps the request key for the same logical start
or input across uncertain delivery. When a stream is gone, it recovers
using the session ID and the kernel transcript. A lost agent session
or an uncertain external result needs reconciliation before another run.

Profiles name the backend, reach, and limits. Profiles for the two roles
are separately selected for the foreman's caller identity per §9.1–2.
The other vendor is checked against the actual session backend. JF1
passes the review material as input to a top-level session and gives it
no checkout or tools. Choose a slice whose evidence fits the kernel's input limits (§9.3).
The mod checks that both review and check evidence
refer to the revision Alice is about to merge. A new head invalidates
readiness. Alice uses a merge action guarded by the reviewed head,
so a concurrent push requires a new decision. Resuming a session or replaying a stream does not repeat a
repository action.

The kernel and providers own the host calls under host §19 and
agentic-harness §2.1. For a Claude shell profile that means
`claude-code-shell.start` and its sandbox; for OpenAI it means the
account-bound `https-stream`. The factory does not call provider-only
sessions or hold account credentials. If later roles use project
toolsets, the kernel opens the named toolset under `agent-tools` with
the origin client's consent. Each tool exposes a bounded operation
under the host's available capability and grant. An instruction or a
toolset name cannot supply filesystem, git, signing, or GitHub authority
by itself. Overlays narrow authority per §9.2, and delegation, once used,
narrows the parent under §9.7.

## 3. Map from the current factory

The current process is recorded in [factory/README](../../../factory/README.md)
and [the shared pipeline](https://github.com/civic-tech-pioneer/watane-mods/tree/main/factory).

| Today | Moves to mods | Authority and work that stay outside |
|---|---|---|
| `factory/PLAN.md`, deputy's project selection, slice sketches, vendor assignment and decisions | Planning page and plan state. Import or copy the selected sketch and keep a reference to the source. Carry the distinct-vendor rule, work lane, scope and escalation to Alice. | The repository plan remains a usable bootstrap and handoff during migration. Alice settles product, project and security decisions. |
| Foreman skill, `factory/tasks/`, `impl.sh`, implementer instructions and ledger | Foreman run, briefs, role profiles, resumable sessions and evidence. An implementer prepares a diff in a dedicated checkout under the chosen N5 profile. | Checkout and branch preparation and Git credentials stay with Alice in JF1. The implementer has only its consented folder and bounded tools, with no push or merge credentials. |
| `check.sh`, `check-main.sh`, `cloud-setup.sh`, project CI and shared check scripts | The foreman shows required checks and associates results with the head. A later toolset may read checks from one selected repository under a separate consent. | Build and test commands and CI stay in the project or its runner. JF1 takes results from Alice. A transcript saying a check passed is not CI evidence. |
| Review lead, validator, user-sim and adversary roles | Foreman dispatches the independent review session, presents its findings and records the verdict against the head. Later profiles can add specialist checks where a slice requires them. | The reviewer gets evidence and no build authority. Specialist execution can stay in the existing pipeline until equivalent scoped sessions exist. Alice dispatches a new round or resolves an escalation. |
| Claude routines, Codex tasks, `watch.sh`, `watch-cloud.sh`, shared PR watchers | The host pages show current work. Kernel transcripts and session IDs replace the agents' out-of-band reporting in JF1. Later, a consented read-only PR toolset and host timers may collect status while the host runs. | External tasks and routines remain an available manual handoff. Host timers do not promise cloud wakeups, dispatch on another machine, or execution while the host is closed. CI and GitHub keep their own duties. |
| Deputy merge and plan update, publisher, installer | Foreman surfaces the matching checks and review for Alice; planning records her result. The same flow can prepare updates to factory sources. | Alice pushes, opens the PR and approves each merge via repository controls. Publication, signing, install and new reach stay in the normal watane release and confirmation path. |

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
reach. Select an initial slice compatible
with those constraints. If a required check or edit needs access outside
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

Each row is for the human. The recommendations keep existing interfaces
in use. S120 records these choices without settling them.

| Door and decision | Options and consequences | Recommendation |
|---|---|---|
| H · Where the mods live | Use an existing mod project with the owner's agreement. This reuses its release process but couples ownership and release cadence. A dedicated repository would give the factory an independent release boundary and require creating and governing a new project. Moving later costs package and repository reference migration and may require fresh identity-linked consents. | Start in an existing mod project if the owner accepts it. Decide the home before assigning permanent publisher/slug identities or publishing anything. Keep design ownership here until then. |
| C · N5, implementer shell | Use the existing Claude shell grant with its sandbox, folder scope and launch constraints. Review that build with OpenAI. Alternatively, adopt `codex app-server`, a third native process kind, which needs a host design for its account, protocol, grants, sandbox and recovery. A later switch changes role profiles and reopens the boundary work. | Follow agentic-harness §17's recommendation for the Claude shell grant at F, subject to a real scoped checkout proving sufficient and the human resolving harness V27. V27 found that allowlisted names can resolve to local addresses (harness §§4.2, 14, 16). Keep shell network closed for JF1 and bring any required network reach back to the human. If the human chooses Codex, the first build waits for that backend contract and implementation. |
| C · Update and authority | Propose source changes, then have Alice review, merge, publish or select the release, and install through the ordinary path. This costs explicit actions and works with existing controls. A richer update proposal could use the normal manager request path where available, but needs a separate authority review. Unattended activation or widening would change the confirmation boundary. | Use ordinary updates with explicit human control. Defer any manager grant. Keep an old release available through normal package mechanisms where possible. |
| C · V7 statements and sequencing | Local notes and GitHub evidence suffice to exercise the draft journey but carry GitHub's trust and availability assumptions. Portable signed statements would let another person verify the slice intent and scope, reviewer verdict at a head, owner merge decision, and release provenance and requested authority against a chosen signer. This requires decisions about signers, visibility, subject identity, supersession and withdrawal. Requiring all statements first delays the real journey but avoids a migration from local evidence. | Agree which statements step 6 requires before building, and whether a supervised GitHub-backed first cut can precede them under VISION. Prefer explicit subject and revision identities and signatures made by people through approved controls. Until the human agrees, the V7 dependency stands. Never publish transcripts or credentials as statements by default. |

The V7 here is watane's vision goal. The harness's stage E probe also
called V7 measures judge latency and remains a separate prerequisite
for that stage. A V7 statement cannot authorize a merge or grant by replay.

## 6. Build order after this design

1. **First build slice · JF1.** After the human resolves the needed home,
   N5 and V7 sequencing, take a small real change through two pages,
   two top-level sessions on different vendors, and one deliberate
   human merge. Alice carries Git evidence, runs checks and opens the
   PR. This cut uses the earlier real account, folder consent and
   both-vendor session work (harness C and D, host §19). It waits on
   stage F for N5 and the factory profiles. It uses no delegation,
   repository toolset or checked-provider verdict itself. Harness §17
   still governs the order of its own stages.
2. **Checked roles and richer evidence.** A role that relies on checked
   Claude tools waits on E, including the judge account and the measured
   check budget. Add a scoped read-only evidence toolset only when its
   provider and consent can use the existing host contract. A judge
   failure leaves the checked action blocked. Neither an unchecked
   fallback nor the judge is part of the cross-vendor review.
3. **Delegated rounds.** F supplies delegation and counters. Use them for
   bounded work within a role where the parent's profile permits it. Keep
   the independent review a separately selected top-level role on the
   other vendor. A later unattended dispatch decision would need its own
   scope and consent.
4. **Self-change · JF2.** Build on JF1 and the ordinary publication and
   installation paths. Requires the approved updates and V7 decisions
   from §5. It inherits the F dependency for factory profiles. If an
   adopted release workflow requires checked tools, that part also
   waits on E. Pass JF2 on a real factory candidate before calling step 6
   complete.
