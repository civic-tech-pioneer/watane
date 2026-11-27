# Factory mod · functional design (draft)

## 1. What Alice controls

The planning page and the foreman page belong to one factory mod. Alice
installs, updates and grants the mod, not each page: each install,
update or consent covers both pages
([TECHNICAL §5](TECHNICAL.md#5-open-decisions)). An update that asks for
added reach still waits for her approval of that reach (JF2).

**FF1 · Plan and dispatch.** The planning page shows the project, plan, slice sketch, outcomes and open
decisions. An agent may propose a sketch or a change in direction. Alice
chooses what to dispatch. Each run identifies the repository, base
revision, slice branch, role, backend, and current revision when one exists.

**FF2 · Build and review.** The foreman page shows the build brief, session progress, checks and review
rounds. It gives Alice the work and the evidence needed to decide. A review
names the exact revision, the vendor, the verdict and each finding. A
new revision makes the previous verdict historical. Alice can ask for
another round or abandon the slice. An unavailable session, denied consent,
failed check or unresolved review leaves the slice visibly blocked or
waiting, with its last known evidence.

**FF3 · Merge approval.** Alice uses the repository's merge controls for the first journey. The
foreman shows when a revision is ready for her decision. Each merge takes
an explicit action by Alice against that revision. Package publication,
installation and changes in reach use their own controls.

**FF4 · Update the factory.** Alice can review and install a candidate
through the usual release and installation controls.

## 2. Journeys

### JF1 · Take one real slice through the factory

Covers FF1–FF3.

Use a real repository with a small slice, a committed plan, and a
head-guarded merge action. The following outcomes can be checked in the
installed app. The snapshot and record rules are in
[TECHNICAL §1](TECHNICAL.md#1-parts-and-sources-of-truth).

1. **Dispatch and recover the brief.** Planning shows the project, plan
   revision, slice, base, branch, scope, acceptance and decisions. Alice
   checks them against the committed plan, dispatches and copies the
   snapshot. In Foreman she pastes it, sees those same fields in a
   preview, and confirms import. A malformed or unsupported snapshot
   shows a field or format error and creates no run. On reload the
   exported snapshot and the imported run are still available. Editing
   the planning draft leaves the imported brief as it was.
2. **Build in the chosen checkout.** Foreman shows the brief, authorized
   implementer profile, dedicated checkout and current attempt. Alice
   confirms the checkout against the project, base and branch, starts a
   Claude implementer and answers the host's folder prompt. She sees
   session progress and the transcript. She can answer tool asks and
   stop the session from Foreman. Reloading during work restores the run
   and transcript without starting a second session. On a usable result
   Alice inspects the diff and advances to checking.
3. **Record project checks.** Alice applies the diff in her trusted
   checkout, opens or updates the PR and runs the required checks in her
   repository tools. Foreman accepts her PR reference, exact head, diff
   and check evidence and shows them together. A missing or failed check
   leaves the run at checking with the reason visible. Alice can supply
   fresh results or return to building. Required project gates still apply.
4. **Review the exact head.** Alice starts an OpenAI reviewer from the
   passing evidence. Foreman shows the evidence revision, head and actual
   reviewing vendor. The reviewer receives the brief, diff and checks;
   Alice can supply further same-head excerpts from Foreman. The
   session has no checkout or tools. Alice reads the reply and confirms
   its verdict and findings. If changes are requested she can start
   another build attempt. A changed head puts the run back at checking
   and keeps the prior verdict visible at its original head. Fresh
   checks and review are needed for the new head.
5. **Record Alice's merge.** Foreman shows ready for Alice only with
   passing checks and a confirmed approval for the current evidence
   revision. Alice rechecks the head in her repository tools and uses
   an explicit merge action guarded by that head. If another push
   wins the race, the merge is refused and she records the new head.
   Accepting the review or closing the page leaves the PR open. Alice
   records both the approved head and the resulting merge revision from
   the merged PR. In Planning she selects that completed run and records
   the result so the slice shows done with its receipt. Git's plan is
   updated by hand.
6. **Recover or stop.** With a missing account, unavailable profile,
   denied folder consent, lost session or unusable review, Foreman shows
   the affected step, reason and last known evidence. Alice can retry
   after resolving the cause, stop an active session or abandon the run.
   After reload, an unresolved start is reconciled before a new attempt
   can start. A stale review reply or stale page action cannot restore
   readiness after the evidence changes. Abandoning preserves the history.

Alice carries repository evidence by hand. The review is limited to that
evidence and the reviewer runs neither the project gate nor user-sim.

### JF2 · Change the factory from the factory

Covers FF4.

1. Alice points the same process at the factory mod's source repository.
   The running release continues to serve the work while the implementer
   changes a separate candidate checkout. Review and checks bind to the candidate
   revision, including the proposed changes in authority.
2. Alice approves the merge and chooses whether to publish or obtain the
   corresponding package. For a GitHub publication she uses
   `watane-dev publish --push` under W30. She inspects the candidate's publisher,
   version, provenance and requested grants, and installs it through
   ordinary watane controls. She can decline and keep the existing
   version. A proposal asking for broader reach stays pending until Alice
   separately approves that reach in the relevant trusted controls.
3. After activation, Alice can see the running release and its source.
   The previous review, merge and grant decisions remain attributable to
   their original revisions or releases. If compatibility prevents a
   safe update, Alice can keep the old release while deciding how to
   transfer its records.
