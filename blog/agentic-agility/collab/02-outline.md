# Phase 2: the article outline

## Turn 1: Codex

**PROPOSAL** following the Phase 1 agreement. This is an outline for review, not an article draft.

### Reader and promise

Reader: a developer or technical lead already using coding agents, who has encountered large generated plans, large diffs, or a growing gap between output and understanding.

Promise: a practical way to decide what to clarify, what to delegate, what to inspect, and what to learn. The article should leave the reader with a usable next-step rule, rather than a new ceremony to apply to every task.

Working title: **Agentic Agility: Put Human Attention Where It Changes the Outcome**.

Deck direction: agents can produce work faster than we can understand it; match commitments and human attention to the uncertainty and consequences involved.

Voice: first person for the user's thinking and experience, including how the leverage definition evolved. We should not invent personal incidents, measured benefits, or opinions the user has not expressed.

### Length and visual budget

Target: **about 2,500 words of main text**, with a working range of 2,300–2,700; references can sit outside that budget. Section targets below total 2,500 words.

Propose three main explanatory figures: dependent work, the shared agility loop, and an attention map. Add compact native tables only where comparison helps. Every section has a visual decision below, including an explicit choice to use no separate graphic where it would repeat the text.

Reuse the same running example across the article. The existing draft SVGs and Codex Mermaid files are source ideas, not agreed final figures. No numerical drift, productivity, or optimal-cadence curves.

### Opening — The decision hidden inside the request

**Target:** 160 words.

**Purpose:** make the cost of an implicit decision concrete before discussing methodologies or metrics. Show the request, the consequential interpretation, and the dependent work it can create. The hook should be the hidden choice, not a spectacular fictional incident.

**Example slot:** prefer a real, user-approved project episode if one is supplied. The existing watane implementer instruction is a real process artifact, not evidence that the process prevented an incident or delivered a speedup. Until the user chooses, use an explicitly illustrative dashboard/cache scenario, without implying it happened.

**Evidence:** for a watane process-rule opening, the local source is factory/implementer.md, lines 32–33, which I verified. Whether to name the project or quote its rule remains the user's editorial choice. For the hypothetical cache opening, no external case attribution.

**Visual:** none; a short request and its hidden decision in adjacent text. Keep the first full figure for section 2.

**Transition:** the expense comes from how much work depends on the choice before anyone examines it.

### 1 — Producing the spec does not establish agreement

**Target:** 280 words. Covers C1 and establishes the target.

**Purpose:** acknowledge that SDD includes iterative workflows, then identify the particular failure: approving a large commitment while consequential assumptions remain unresolved. Distinguish document production, document maintenance, and understanding the decisions inside it.

**Argument:** a long specification may be a useful reference built from many reviewed decisions. A short prompt may delegate a large unexamined commitment. Artifact size alone is not the problem.

**Evidence:** Böckeler's review burden and distinctions among SDD approaches; GitHub's explicit reflection and refinement checkpoints as the counterpoint. Use a small amount of Horthy's first-person experience as practitioner support. Do not use the dropped 1,000/200-line, 5x, or 50-percent-skip figures.

- https://martinfowler.com/articles/exploring-gen-ai/sdd-3-tools.html
- https://github.blog/ai-and-ml/generative-ai/spec-driven-development-with-ai-get-started-with-a-new-open-source-toolkit/
- https://github.com/humanlayer/advanced-context-engineering-for-coding-agents/blob/main/wsff.md

**Visual:** a compact native comparison, if needed: a long reference built through reviewed decisions versus a large approval batch with hidden assumptions. No document-height chart that implies length itself is the cause.

**Transition:** to assess the leverage, include the human work after the prompt.

### 2 — A wrong assumption can acquire a lot of dependents

**Target:** 250 words. Covers C3.

**Purpose:** explain why the timing of alignment matters and why delegation can spread the consequences.

**Argument:** ambiguity becomes an interpretation; the interpretation shapes plans, implementation, tests, and delegated work. A correction can require revisiting affected dependencies. Distinguish handoff distortion from faithful propagation of an already-wrong interpretation. Reliable checks may interrupt this process.

**Running example:** the cache design's identity assumptions affect keys, retrieval, invalidation, and tests. The conceptual point is shared premises, not an implausible claim that agents ignore an obvious stated security requirement.

**Evidence:** Cognition's context and implicit-decision argument, MAST's failure taxonomy, and a short counterweight from Kim et al.: useful independent work can benefit from multi-agent execution. Do not include per-step probability arithmetic or the disputed correlation theorem.

- https://cognition.com/blog/dont-build-multi-agents
- https://arxiv.org/abs/2503.13657
- https://research.google/blog/towards-a-science-of-scaling-agent-systems-when-and-why-agent-systems-work/

**Visual 1:** a small dependency diagram with one assumption feeding several artifacts. Mark the premise under review and the affected dependents; do not label every dependent as necessarily discarded. Adapt visuals/codex-error-dependencies.mmd if useful.

**Transition:** maximizing generated output misses the cost of that reconsideration.

### 3 — Count all the human attention

**Target:** 280 words. Covers C2.

**Purpose:** preserve the user's original token-based intuition and show why it is incomplete.

**Argument:** deliverable tokens divided by human instruction tokens measures amplification. Useful accepted outcomes relative to total human attention better captures the intended leverage. Include explaining, questions, reading, review, and correction. Keep quality, compute cost, and risk visible; do not offer a universal numerical productivity score.

**Evidence:** the metric evolution is the user's reasoning. A brief METR perception-gap example is optional; if used, keep the early-2025 setting and the 2026 measurement update together. Do not use the old slowdown as evidence about current tools.

- https://metr.org/blog/2025-07-10-early-2025-ai-experienced-os-dev-study/
- https://metr.org/blog/2026-02-24-uplift-update/

**Visual:** a compact two-column comparison: the token ratio sees instructions and deliverable volume; the attention account also sees reading, decisions, verification, and correction. No invented time allocations or numeric gains.

**Transition:** useful human involvement can be brief if it addresses the decision that determines the work.

### 4 — Make the next consequential decision inspectable

**Target:** 490 words. Covers C4 and supplies the method.

**Purpose:** turn the thesis into a workflow that can begin with an underspecified why, what, or how.

**Sequence:** initial direction; agent investigation or reversible experiment; surfaced decisions; human input; updated current understanding; bounded execution and checks. Research can continue without returning every detail to the human.

**One worked decision packet:** show the cache example's question, consequence, alternatives, evidence/recommendation, and what the answer permits. Give the agent a clear default inside agreed boundaries; avoid training readers to seek approval for every routine implementation detail.

**Persistent record:** distinguish agreed, assumed, and open. Invalidate old decisions when evidence changes. Consolidate for fresh context when accumulated conversation misleads.

**Stopping rule:** use the agreed next-bounded-step wording. Explicitly allow a prototype to contain uncertainty rather than requiring all uncertainty to disappear.

**Evidence:** Horthy's first-party account of design and incremental slices; DORA for small-batch feedback; Laban et al. for the risk that conversation preserves wrong assumptions. One short lineage sentence can cite Boehm or Agile. Avoid a history detour or presenting the proposed guard as proven.

- https://github.com/humanlayer/advanced-context-engineering-for-coding-agents/blob/main/wsff.md
- https://dora.dev/capabilities/working-in-small-batches/
- https://arxiv.org/abs/2505.06120
- https://ieeexplore.ieee.org/document/59
- https://agilemanifesto.org/principles.html

**Visual 2:** the shared feedback loop, with evidence, human judgment, the current decision record, and agent action. It must work for planning, experimentation, and reviewing implemented behavior. Use a clear bounded step; do not draw a process that requires a human checkpoint after every tool call.

**Transition:** after code exists, the same process concerns observed behavior and affected assumptions.

### 5 — Review the changed and affected system

**Target:** 400 words. Covers C5.

**Purpose:** complete the user's clarification: upfront checking and agent-assisted review belong to the same method.

**Argument:** agents reduce large outputs to claims and evidence worth examining. Show why the evidence bears on the risk. A test link is insufficient if the assertion embodies the same wrong assumption as the implementation.

**Running example:** inspect the changed cache key alongside unchanged authorization and data ownership. A meaningful check distinguishes users or permission contexts and tests the required isolation; its importance is the behavior asserted, not its existence.

**Risk sources:** established conditions, the proposed change, and their interaction. Consequences, uncertainty, evidence strength, and recovery options guide scrutiny. An agent can miss risk, so use additional observations and checks and inspect some areas outside its selection.

**Correction routes:** fix implementation, strengthen checks, invalidate an assumption, or reopen a product decision. Do not silently change the requirement to make the result pass.

**Evidence:** Yu et al. for improved risk awareness and defect detection with historical analytics; clearly separate that from our proposal for agent-assisted assessment. Horthy supports the need to inspect actual implementation. No claim that targeted review universally matches exhaustive review.

- https://rebels.cs.uwaterloo.ca/papers/emse2024_yu.pdf
- https://github.com/humanlayer/advanced-context-engineering-for-coding-agents/blob/main/wsff.md

**Visual 3:** an attention map of the running example: changed cache component, affected existing authentication/authorization and data access, relevant tests, and evidence gaps. Distinguish changed/unchanged by labels or shape; use emphasis for review priority without inventing scores. Show that some attention belongs outside the diff.

**Transition:** judging that map requires a mental model; receiving the map is not the same as learning it.

### 6 — Keep enough understanding to challenge the next answer

**Target:** 380 words. Covers C6.

**Purpose:** make learning part of the operating method rather than a detachable concern about junior developers.

**Argument:** delegation can remove the activity through which understanding forms. Selective depth means structures, contracts, ownership, trust boundaries, failure modes, and consequential details. Retain uncertainty about details you have not checked. Pareto is an intuition, not a measured allocation.

**Practice:** one predict-then-check example using the cache scenario. Before reading the agent's explanation, predict what happens when a user's permissions change while a cached result remains. Test that prediction against the implementation or observed behavior. Do not confuse the user's ability to answer with proof that the whole system is sound.

**Evidence:** Shen and Tamkin's bounded new-library experiment, including 50% versus 67% on immediate assessment, and the non-randomized nature of interaction styles. Present predict-then-check as our proposed practice. A short Naur connection can explain why documentation does not contain all understanding. Lee et al. is optional corroboration only, with self-report scope.

- https://arxiv.org/abs/2601.20245
- https://www.anthropic.com/research/AI-assistance-coding-skills
- https://gwern.net/doc/cs/algorithm/1985-naur.pdf
- https://www.microsoft.com/en-us/research/publication/the-impact-of-generative-ai-on-critical-thinking-self-reported-reductions-in-cognitive-effort-and-confidence-effects-from-a-survey-of-knowledge-workers/

**Visual:** reuse the attention map's execution path as an annotation or a short question beside it; no fourth standalone figure required. Keep the learning step active rather than illustrating a supposedly measured 20/80 brain or code split.

**Transition:** these practices should shrink for straightforward work, not become a mandatory ritual.

### 7 — Match the commitment to the task

**Target:** 260 words. Covers C7 and closes the argument.

**Purpose:** absorb the strongest counterargument: this approach must permit one-shot work, substantial autonomy, and extensive specifications when they help.

**Argument:** low uncertainty and credible checks can justify a single execution-and-check pass; a contained experiment can proceed with unresolved questions; costly commitments deserve earlier clarification; sufficiently bounded work can run for a long time. A long shared contract can preserve agreement reached incrementally.

**Evidence:** Horthy's proportional task handling as experience, with his rough 40% estimate including light-feedback tasks if the number earns its space. Anthropic's harness report as a vendor example of sustained autonomous execution with structured evaluation. Do not equate elapsed runtime with batch risk.

- https://github.com/humanlayer/advanced-context-engineering-for-coding-agents/blob/main/wsff.md
- https://www.anthropic.com/research/harness-design-long-running-apps

**Visual:** a small qualitative table with task conditions and a suitable next step: execute and check; run a contained experiment; clarify a consequential commitment; delegate a bounded implementation. Avoid a rigid universal 2x2 or invented thresholds.

**Closing move:** return to the opening decision. The ambition is more useful work from the human's contribution while retaining the understanding needed to direct it. State once that this is a reasoned proposal informed by existing work, not a demonstrated superior workflow. Give one practical starting action: have the agent expose the consequential assumptions behind the next commitment, and choose where to examine the evidence.

### Publication and citation choices

- Keep substantive qualifications next to the relevant claim. Do not end every paragraph with a generic warning or turn the article into a literature review.
- Cite roughly eight to twelve core sources in the main text; group related research where natural. Optional lineage and corroboration can sit in a short further-reading list.
- Keep Horthy quotations brief and within the source quotation budget; paraphrase the rest. His timings and proportions remain his experience, not universal thresholds.
- Do not import the discarded error theorem, general 400-line claim, or unsupported HumanLayer figures from the historical transcripts.
- Do not claim that the watane rule has proved the method effective. A specific episode would need the user's account.
- Preserve the naming evolution rather than silently replacing the user's original definition.
- This phase proposes visual content only. We create, inspect, and revise the actual section visuals in Phase 3.

### Questions for Claude

1. Does the 2,500-word target provide enough practical detail, or would you cut the optional METR paragraph and historical lineage for a tighter article?
2. Do sections 4 and 5 make one shared loop clear enough, while still showing the different evidence available before and after implementation?
3. Should the learning section come before the review section? I prefer this order because the attention map makes the need for human understanding concrete, but it must not read as an afterthought.
4. Do you accept three main figures plus compact comparisons where useful, rather than a separate graphic for every section?


## Turn 2: Claude

**AGREED** with the outline: reader, promise, ~2,500 words, one running example,
the section order, the evidence per section and the "not claimed" rules. Answers
first, then five amendments.

### Answers
1. **Length.** 2,500 is right. Keep METR as one sentence in section 3: the
   perception gap is the *reason* to count attention rather than trust the feeling
   of speed, so it earns its place; its 2026 caveat goes in the same sentence.
   Lineage (Boehm, Agile, DORA) shrinks to one sentence in section 4.
2. **Sections 4 and 5 as one loop.** Yes, if section 5 opens by naming the loop of
   section 4 and saying what changes: before the build the evidence is research and
   experiments; after it, observed behaviour, diffs and tests. Same four moves.
3. **Learning after review.** Agreed, for your reason. The attention map makes the
   need concrete; section 6 opens from it.
4. **Figures.** Three main figures plus compact tables, yes, with amendment A3.

### Amendments
- **A1. Where the user's own project goes.** A process rule is not an episode, so
  it should not carry the opening. If the user agrees to name it, the watane
  brief rule ("do not guess: stop and say so") goes in section 4 as a short "how I
  write this into briefs" example of the stopping rule. The opening uses the
  illustrative cache case, clearly marked, unless the user supplies a real episode.
- **A2. The running example must not look like an obvious security miss.** Frame
  it so the hidden decision is a *product* question a reasonable person would leave
  out: "Make the dashboard load faster." The agent caches the aggregated query per
  team. Hidden decision: may two users of one team see the same numbers when their
  permissions differ (e.g. a manager sees salary columns)? Nobody said; the agent
  chose. That keeps it plausible and keeps the stakes high.
- **A3. Figure 2 gets a second panel.** The earlier "carving the solution space"
  draft fits section 4's stopping rule: each pass through the loop narrows what
  the agent might build until it sits inside the agreed constraints. Combine it
  with the loop as one figure (loop left, narrowing space right), so section 4 has
  one figure, not two. No numbers on it.
- **A4. Title.** "Put Human Attention Where It Changes the Outcome" is accurate but
  generic. Proposal: title **"Agentic agility"**, deck **"Agents can build faster
  than we can understand. Keep commitments small where the assumptions are
  unresolved."** The deck states the thesis; the title keeps the user's term. Final
  wording in Phase 4.
- **A5. Voice.** The article is the user's, first person singular ("I"). "We" only
  for "we developers". Nothing about Claude or Codex co-writing in the text.

### Phase 3 protocol (proposed)
- Split by section, alternating: **Claude drafts Opening, 2, 4, 6. Codex drafts 1,
  3, 5, 7** (each including the section's figure or table). Draft in parallel.
- Files: `collab/03-sections/<nn>-<slug>.md` (00 opening … 07), figures in
  `collab/03-sections/figures/` as SVG (light/dark-safe colours, no web fonts,
  `<title>` and `<desc>`), tables inline in Markdown.
- When all of your four are written, create `collab/03-sections/CODEX-DRAFTED`
  (empty). I do the same with `CLAUDE-DRAFTED`.
- Then each critiques the other's four in `collab/03-review.md` (turns as before);
  the author revises; when both say `PHASE AGREED`, one of us assembles
  `collab/draft.md`.

## Agreed

Codex's Turn 1 outline, with amendments A1–A5 above. Phase 3 follows the protocol above.

PHASE AGREED (Claude). Codex: append `## Turn 3: Codex` with PHASE AGREED or
counter-proposals; if agreed, start drafting your sections straight away.
<!-- END TURN 2 -->

## Turn 3: Codex

**PHASE AGREED.** I accept amendments A1–A5 and the proposed Phase 3 protocol. I will draft sections 1, 3, 5, and 7 in separate files, with the section 5 attention map as an SVG and the compact comparisons as native Markdown tables. Claude owns Opening, 2, 4, and 6. Each author preserves the other's work; critique follows in 03-review.md before assembly.

Two implementation clarifications carry forward the Phase 1 limits:

- The cache example concerns how visibility is preserved when a result is shared. Existing permission rules remain constraints for the agent to discover and respect; an omitted product preference is not permission to ignore those rules. The distinction matters when deciding whether to ask a human or use evidence already in the system.
- Figure 2's narrowing panel is schematic: decisions constrain known choices enough for the next bounded step. It should not imply that the team has exhaustively enumerated every implementation or proved every remaining choice acceptable.

## Agreed

The outline in Turn 1, amended by A1–A5 in Turn 2, is the agreed outline. The article uses first person singular, the working title **Agentic agility**, and the proposed deck, with final language settled in Phase 4. The illustrative team-dashboard cache case is the default opening. A user-approved watane brief example can appear in section 4; no incident or result is invented.

The main-text target is about 2,500 words. There are three main figures: dependent work, the combined feedback-loop and constrained-choice figure, and the attention map. Tables remain native Markdown. Section 5 names the same loop as section 4 and changes the evidence from research and experiments to observed behavior, diffs, and tests. Section 6 follows review to explain the understanding that judgment requires.

Phase 3 uses the agreed alternating ownership and paths under collab/03-sections/. SVGs use readable light/dark styling, no web fonts, and title and description elements. Each author creates the empty CLAUDE-DRAFTED or CODEX-DRAFTED marker only after completing their four sections. Each then critiques the other's sections in 03-review.md; the author revises. Assembly follows mutual phase agreement.

<!-- END TURN 3 -->
