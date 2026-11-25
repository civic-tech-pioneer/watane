# Factory mods · functional design (draft)

## 1. What Alice controls

The planning mod shows the project, plan, slice sketch, outcomes and open
decisions. An agent may propose a sketch or a change in direction. Alice
chooses what to dispatch. Each run identifies the repository, base
revision, slice branch, role, backend, and current revision when one exists.

The foreman mod shows the build brief, session progress, checks and review
rounds. It gives Alice the work and the evidence needed to decide. A review
names the exact revision, the vendor, the verdict and each finding. A
new revision makes the previous verdict historical. Alice can ask for
another round or abandon the slice. An unavailable session, denied consent,
failed check or unresolved review leaves the slice visibly blocked or
waiting, with its last known evidence.

Alice uses the repository's merge controls for the first journey. The
foreman shows when a revision is ready for her decision. Each merge takes
an explicit action by Alice against that revision. Package publication,
installation and changes in reach use their own controls.

## 2. Journeys

### JF1 · Take one real slice through the mods

1. Alice enters one real repository and a small slice in the planning mod.
   She reviews the sketch and dispatches it. She prepares a dedicated
   checkout of the specified base and branch and accepts only the
   implementer's required reach when the host asks.
2. From the foreman mod she starts an implementer on that checkout. The
   transcript records its result. She can reload the page and recover the
   run and transcript. She inspects the actual diff, applies it in her trusted checkout, runs the project's
   checks, and opens a PR in her repository tools. The foreman records the
   PR, exact head, and check results Alice supplies.
3. She starts a review in the foreman mod with the other vendor and
   supplies the brief, diff and check evidence for that head. The reviewer
   returns a verdict with findings. Its session has no checkout write
   reach or repository tools. If it needs more context, Alice adds the
   relevant excerpt or leaves the review pending.
4. A change request returns to the build. Alice supplies evidence for the
   new head and the foreman asks for fresh review. Once the checks and
   review match the current head, Alice inspects it and explicitly merges
   in her repository tools. The planning page records the merge revision
   and marks the slice done. Closing the page or accepting the review alone
   leaves the PR open.
5. With a missing account or denied folder consent, the affected agent
   cannot proceed. Alice sees which step stopped and can cancel. If the
   repository head changes after review, the old verdict stays visible
   with its old head and the new head needs its own checks and review.

### JF2 · Change the factory from the factory

1. Alice points the same process at the factory mods' source. The running
   release continues to serve the work while the implementer changes a
   separate candidate checkout. Review and checks bind to the candidate
   revision, including the proposed changes in authority.
2. Alice approves the merge and chooses whether to publish or obtain the
   corresponding package. She inspects the candidate's publisher,
   version, provenance and requested grants, and installs it through
   ordinary watane controls. She can decline and keep the existing
   version. A proposal asking for broader reach stays pending until Alice
   separately approves that reach in the relevant trusted controls.
3. After activation, Alice can see the running release and its source.
   The previous review, merge and grant decisions remain attributable to
   their original revisions or releases. If compatibility prevents a
   safe update, Alice can keep the old release while deciding how to
   transfer its records.
