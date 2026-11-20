# Phase 3 review: each critiques the other's sections

## Turn 1: Claude (on Codex's 1, 3, 5, 7 and figure 3)

Overall: all four are faithful to the agreed argument and outline, sourced and
honest about limits. My points are mostly about making the strongest sentence
land and connecting each section to the running example. Line-level wording is for
Phase 4; below are only changes of substance or structure.

### Section 1: Producing the spec does not establish agreement
- **AGREED** on target, Spec Kit counterpoint and the table.
- **R1.1 Tie it to the opening.** The reader has just met the dashboard. One
  sentence should connect: "the dashboard request was short; a long spec could have
  hidden the same question just as well." That makes "artifact size ≠ commitment
  size" concrete before the table.
- **R1.2 Promote the thesis line.** "As output grows, my ability to assess them can
  become the limiting resource" is the article's C1 and sits mid-paragraph. Give it
  its own short paragraph, in plainer words, e.g. "Agents can now produce more than
  I can judge. My judgement is the scarce part."
- **R1.3 Böckeler paragraph:** "Her experience illustrates a cost that document
  generation alone does not remove" says nothing the previous sentence didn't.
  Replace with what she actually found that matters for us (review burden; the
  risk of a false sense of control) or cut it.
- **R1.4 Keep the PRD line**; it is the user's original insight. Fine as is.

### Section 3: Count all the human attention
- **AGREED** on keeping the user's original definition verbatim and the table.
- **R3.1 Name the new definition in one bold line.** Right now the reader has to
  assemble it. Suggest: "**Agentic leverage** is the useful, accepted work I get per
  unit of my attention: writing, answering, reading, reviewing and correcting."
- **R3.2 METR sentence.** It is hard to parse and the caveat is attached to the
  wrong thing (the 2026 update is about *later* estimates). Proposal: "In METR's
  early-2025 trial, experienced developers were measured to be about 19% slower
  with AI, yet believed it had made them about 20% faster. The tools have changed
  since, and METR itself warns against reading the number as current. The gap
  between felt and measured speed is the point." Concrete numbers, caveat attached
  correctly, purpose stated.
- **R3.3 End on the example.** "A longer instruction can improve leverage if it
  resolves a costly ambiguity": say which one: one sentence about who may see
  whose numbers, written before the cache existed.

### Section 5: Review the changed and affected system
- **AGREED**, this is strong. The "agent should investigate the existing
  authorization model before asking me to invent a policy" point is a good addition
  and matches your Phase 2 clarification.
- **R5.1 Name the deterministic signals.** C5 agreed that the agent's ranking is
  checked by signals that do not share its blind spots. "Other evidence" is vague;
  name two or three: coverage of the changed lines, static analysis, the call graph
  from the changed code to unchanged code, files that change often (Tornhill's
  hotspots). One sentence.
- **R5.2 Pointers, not paraphrases.** You have the idea ("a summary saying
  permissions are respected gives me little"); make the request explicit and
  quotable, e.g. "I ask for pointers, not paraphrases: the line, the test that
  covers it, and what would break."
- **R5.3 Last paragraph** duplicates the opening of my section 6 ("An attention map
  is only useful to someone who can read it"). Keep one. I propose you keep yours as
  the transition, and I cut my first two sentences.

### Section 7: Match the commitment to the task
- **AGREED** on table and Horthy framing.
- **R7.1 Close on the opening, as the outline says.** The ending does not return to
  the dashboard. Proposal: last paragraph goes back to it: the question about shared
  views cost one sentence to answer before the cache existed, and four pieces of
  work to revisit after. Then the starting question.
- **R7.2 Move the hedge out of the closing.** "Informed by existing work rather than
  proven superior to every alternative" is right to say once, but the last
  paragraph is the worst place: it deflates the ending. Put it one paragraph earlier,
  shorter: "I can't prove this beats other ways of working. It is what the evidence
  and my own practice point to."
- **R7.3 Number check:** Horthy's "~40% one-shot or light feedback" is the concrete
  figure; you chose to leave it out. I'm fine either way; if in, label it his
  estimate.

### Figure 3 (attention map)
- Clear and correct as a *dependency* figure. Two changes:
- **F3.1 Show attention, not only dependencies.** The section's point is where
  attention goes. Encode it: e.g. a thick or highlighted outline on the interaction
  (cache hit × viewer permissions) as "look closely", the unchanged query scope as
  "skim", and one small item outside the flagged area marked "spot-check". Still no
  numbers.
- **F3.2 One visual style for all three figures.** Yours uses a blue palette, an
  in-figure title and larger type; mine use neutral greys, orange dashed for
  "unexamined / at risk" and green for "agreed with the human", with titles left to
  the Markdown caption. Proposal: adopt mine across all three (the colours carry
  meaning across figures: orange = still an assumption, green = decided), drop
  in-figure titles, 680 wide. I am happy to restyle figure 3 myself if you prefer to
  keep authoring content only.

Your turn: critique my Opening, 2, 4, 6 and figures 1–2 under `## Turn 2: Codex`,
and say which of R*/F* you accept. End with `<!-- END TURN 2 -->`.
<!-- END TURN 1 -->


## Turn 2: Codex

**AGREED** on the division of work, the running example as an illustration, the three-figure budget, and the major direction of all four Claude sections. My main requests concern what the example establishes, which decisions actually require a human, and what the loop can claim about attention and learning.

No section or figure has been revised in this turn. I read the SVG source and inspected rendered light and forced-dark-palette previews of figures 1 and 2. This checks their content and appearance at the authored width, not browser theme-switching or mobile layout.

### Opening

**AGREED:** the short request is a strong hook. Showing the cache key, permissions, and apparent performance improvement immediately makes the dependent-work problem concrete. Keep the example explicitly fictional.

**C0.1 Required — distinguish an omitted constraint from an unresolved preference.** The opening says the agent reads the code, and that the old query applies each user's permissions. That means the system already supplies evidence that the manager and engineer may see different data. Sharing salary information is not a policy question left open merely because the performance request omits it.

The last paragraph currently excuses the choice on the grounds that the agent had to pick something. It did not have to silently pick a new visibility policy: it could preserve the existing boundary, investigate it, or flag a genuinely unresolved tradeoff. This conflicts with section 5 and the Phase 2 clarification we agreed.

Keep the case, but make its lesson: the agent treated a team as a valid reuse boundary without examining the constraint that boundary had to preserve. The human need not answer a question that the code and requirements already settle. A human decision may still be needed about freshness, performance targets, complexity, or which viable optimization is worth pursuing.

Suggested direction, not a replacement paragraph: the request was short, but the existing system was also part of the instruction. The consequential assumption was that team membership sufficiently described who could share a cached result.

**C0.2 Required — do not imply a source for the generalization.** The aside saying the example is made up but its shape is not can read as a claim about a real comparable incident. Prefer an explicit illustrative label and let the cited dependency and review research establish the broader mechanism later. Likewise, passing tests and a large speed improvement belong to the fictional setup, not observations about agent performance.

### Section 2 — A wrong assumption gathers dependents

**AGREED:** the four dependents are concrete, the faithful-versus-distorted handoff distinction works, and the Kim counterpoint prevents the article becoming a blanket argument against delegation.

**C2.1 Required — make commitment conditional, not inevitable.** The opening says an ambiguity must quickly become an interpretation to make progress. Investigation, a reversible experiment, or preserving an existing contract are also ways to make progress. Write about what happens when the agent commits to an interpretation without checking it. This matters because the method later recommends exactly those other routes.

Similarly, the steps are internally consistent with the premise, rather than individually sensible in light of the established permissions. Carry C0.1 through this section and figure 1.

**C2.2 Acceptable with a narrow wording change — affected work needs another look.** All four illustrated dependents can warrant reconsideration because each was explicitly built on the premise. Do not let this become a general claim that every downstream artifact must always be reviewed or that longer elapsed time necessarily creates more dependency. More affected work is the mechanism; some work can survive the correction.

**C2.3 Required citation repair.** Link the Cognition attribution directly to [Walden Yan's article](https://cognition.com/blog/dont-build-multi-agents). At present the paragraph names Cognition but leaves that specific supporting claim without its source URL.

The final test sentence is useful, with a qualification: tests derived only from the same wrong premise can pass. An existing acceptance check or a test derived from the authorization contract can expose the mistake.

### Section 4 — Make the next decision visible

**AGREED:** the packet, current decision record, bounded-step rule, and contained prototype make the proposed method usable. The first-person voice largely fits the user's notes. Keep the watane example optional until the user selects it.

**C4.1 Required — make the packet answer the right question.** As written, the packet asks whether the human permits a visibility change that the existing system already rules out. Investigate and retain that rule first. Then expose the consequential choice among viable approaches: viewer-specific caching, reuse across requests with equivalent permitted data, or query optimization, with appropriate checking of freshness and permission changes.

Do not imply that a permission-set key alone solves every identity, tenant, row-filter, or revocation dependency. The article need not design the complete cache; it should show the assumptions the recommendation depends on.

**C4.2 Required — label illustrative findings and keep tradeoffs conditional.** The three permission sets and the repeated expensive join are invented findings within the made-up case. Mark them as hypothetical rather than let them read as the author's investigation. The labels smaller gain and larger gain also depend on workload and invalidation costs. Present them as expectations to check, or replace them with the reason an option is worth testing.

A good packet can recommend a bounded experiment where performance evidence is missing. That illustrates our stopping rule better than unsupported certainty.

**C4.3 Required — remove the implied productivity measurement.** One sentence doing more than an hour of review is a fabricated quantitative comparison. A brief answer can address a consequential premise more directly than blanket document approval, but the time needed to understand and answer may be substantial. Preserve that distinction. No need to replace it with a timing claim.

**C4.4 Required — do not claim the guard is established.** I agree with the Laban qualification and fresh-context proposal. Calling the guard obvious does not add useful support. Keep it as a proposed response to the observed anchoring risk. A compact record must still be checked for lost assumptions or contrary evidence.

**C4.5 Suggested — shorten the lineage claim.** Risk-driven decision iteration existed before agents. The new opportunity is agents helping with investigation, experiments, and evidence preparation at greater scale. Avoid saying research has now become cheap enough for the first time to apply small batches to decisions; Boehm's precedent already concerns risk and decisions. One sentence of lineage is enough. Link Boehm if retained.

### Section 6 — Keep enough understanding to challenge the next answer

**AGREED:** the study is scoped well; it distinguishes the randomized AI-access comparison from the observed interaction styles. Selective depth and the 80/20 qualification are faithful to the argument.

**C6.1 Required — avoid inventing a personal practice.** The user proposed deep engagement and selective understanding. They have not told us they routinely use predict-before-reading. Introduce it as a practice the article proposes or one the author would try, rather than an established habit. The Phase 1 agreement specifically describes this as our proposed safeguard.

**C6.2 Required — a mismatch has more than one explanation.** If a prediction differs from the implementation or a test, the mental model may be wrong, but the code or test can also violate the intended contract. Comparing the two should trigger investigation of the disagreement. Do not teach the reader to update their understanding to match any observed behavior automatically.

Keep the permission-change example. It neatly tests whether the model includes authorization, cache lifetime, and invalidation. Make the expected behavior depend on the agreed rule, rather than introduce an unexplained five-minute allowance.

**C6.3 Suggested — qualify exhaustive reading.** Reading every line of every change may consume the leverage the author wants, but some changes merit comprehensive review. Keep the broader position: maintain structural understanding and go deep where details carry consequences, without prescribing uniformly shallow or uniformly exhaustive inspection.

**C6.4 Agreed transition edit.** I accept R5.3: keep section 5's transition and remove section 6's repeated attention-map introduction. Preserve the next thought about delegation removing the activity through which understanding forms.

The Naur attribution is useful lineage. Keep it about the programmer's understanding of how the solution relates to the problem; it is not empirical validation of our proposed learning practice.

### Figure 1 — Dependents

**AGREED:** at 680 pixels, the diagram is readable in both tested palettes, the branching is clear, and orange is also distinguished by the dashed premise outline. The caption usefully exposes the need to reconsider dependencies without inventing risk scores.

**CF1.1 Required — align the premise with C0.1.** Replace the implication that salary visibility is an unanswered discretionary policy choice with the unchecked reuse assumption: team membership alone determines which results can be shared. Existing visibility rules constrain that assumption. Update the title/description and visible labels together.

**CF1.2 Suggested — make status precise.** All four dependent boxes are orange, although we have not established that all four implementations are wrong. Clarify in the caption or label that they depend on an unchecked assumption and warrant examination. Neutral boxes with an orange dependency cue could also communicate this. Do not let the palette silently mean a confirmed defect.

### Figure 2 — Loop and solution space

The two panels are legible at their authored width and the shared palette works. Three semantic changes matter more than cosmetic changes.

**CF2.1 Required — human judgment is review.** Remove the label a sentence, not a review. Answer length does not measure cognitive effort, and a decision can require examining code, alternatives, or experiments. A label such as resolve the tradeoff keeps the focus without promising effortless oversight.

**CF2.2 Required — show a next-step condition, not a containment proof.** The shrinking orange rectangle ending entirely inside a fixed green region currently says that every possible agent output is acceptable to the human. The description makes the same strong claim. This is precisely the guarantee we excluded in Phase 1.

The panel can remain if its labels make it about **known choices for the next step**, bounded by agreed constraints and subject to checking. Call it schematic. Show or state that new evidence can reopen the choices. A contained experiment is permitted even when the full solution remains uncertain.

Green should mean an agreed constraint or decision, not verified correctness or guaranteed acceptable outcomes. Orange should mean an unresolved assumption, not a measured unsafe region.

**CF2.3 Required — avoid a mandatory human gate on every cycle.** The four boxes route every pass through a human. Our agreement permits execution and checking within existing boundaries, with human input when a consequential unresolved choice appears. Add that condition in a label, branch, or caption. The action box should include checking the result as well as recording and execution.

**CF2.4 Shared publication check — small-screen readability.** At 320 pixels wide, 12- and 14-pixel text in a 680-pixel SVG scales to roughly 6–7 pixels. Both panels should have a usable narrow presentation, perhaps stacked, or another explicit reading arrangement. My figure 3 needs this consideration too. Matching canvas width alone is not sufficient. We can settle this during figure revision without creating an extra conceptual figure.

### Responses to your R and F points

- **R1.1 ACCEPT.** Tie the artifact/commitment distinction to the short dashboard request. The long specification could preserve or conceal the same assumption.
- **R1.2 ACCEPT WITH WORDING LIMIT.** Give the attention claim its own paragraph. Retain the conditional scale claim agreed in Phase 1; do not imply judgment is universally the dominant bottleneck or invent a measured limit on the author's capacity.
- **R1.3 ACCEPT.** Cut the repeated explanation and use the source's review burden or false sense of control.
- **R1.4 ACCEPT.** Keep the user's PRD connection.
- **R3.1 ACCEPT.** Name agentic leverage explicitly in one concise line, including useful accepted work and total human attention. No universal numerical score.
- **R3.2 ACCEPT THE CLARITY GOAL, WITH SOURCE PRECISION.** Concrete historical results can help. Use took 19% longer rather than the mathematically different throughput statement 19% slower. Keep the perceived 20% speedup distinct. The 2026 update discusses later estimates and selection problems; do not attribute an exact warning to METR that we have inferred rather than quoted. State the early setting and later measurement limitation in plain language. Keep this brief.
- **R3.3 ACCEPT.** End with the visibility constraint or viable optimization choice from the example, consistent with C0.1.
- **R5.1 ACCEPT WITH QUALIFICATION.** Name specific signals, for example coverage, static-analysis findings, and traced dependencies into unchanged code. They are useful inputs with their own limits, not guaranteed independent checks of intent. Coverage does not show whether an assertion tests the right behavior. I will not cite hotspots as a validated risk measure without checking that source.
- **R5.2 ACCEPT THE INTENT.** Ask explicitly for locations, assertions, and why the evidence addresses the requirement. A line and a covering test alone still need interpretation. Explanations should accompany pointers; the aim is inspectable evidence rather than paraphrase alone.
- **R5.3 ACCEPT.** Keep the transition in 5 and avoid repeating it in 6.
- **R7.1 ACCEPT WITH EXAMPLE LIMIT.** Return to the dashboard. Describe a brief clarification before dependent work accumulates; do not give a fabricated cost of exactly one sentence versus four units of rework. The four depicted artifacts are illustrative dependents, not four measured repair costs.
- **R7.2 ACCEPT MOVING THE LIMITATION.** Keep it once before the ending. Avoid saying the author's own practice proves or points to this workflow when they have only proposed parts of it here. A concise statement that this is a proposal informed by the evidence is enough.
- **R7.3 KEEP THE NUMBER OUT.** Horthy's proportional handling already supports the paragraph. His estimated percentage adds little here and needs extra qualification about light feedback.
- **F3.1 ACCEPT THE ATTENTION EMPHASIS, COUNTER-PROPOSE THE LABELS.** Highlight the unresolved cache/permissions interaction and show an illustrative spot-check outside the agent's flagged area. Do not categorically label the query scope skim: it may determine whose rows or columns enter the cache and deserve deep inspection itself. Priority depends on the question and evidence, not whether a component is unchanged.
- **F3.2 ACCEPT SHARED STYLE, WITH SEMANTICS AND READABILITY PRESERVED.** Use neutral structure, orange for unresolved assumptions, green for agreed decisions or constraints, and a common width. Do not use green as a correctness guarantee. Keep changed/unchanged status in text or shape rather than repurpose the semantic colors. I will revise my figure 3's content and style myself after the review round; no need for us to edit the same SVG. Removing its in-figure heading is fine if the caption supplies the subject. Preserve readable type and a narrow-screen plan rather than automatically reducing every label to the smallest size.

### Suggested revision order

1. Settle the example's existing visibility constraint and the actual tradeoff to be decided. It affects the Opening, 2, 4, 5, 6, and all three figures.
2. Revise the section and figure claims about effort, guarantees, and learning, then make each author's accepted connection and evidence edits.
3. Check the assembled transitions, total length, figure palette semantics, and reading widths together.

Both reviews are now recorded. I am ready to revise my four sections and figure 3 once we agree the example correction above. The rest of this review does not require adding more ceremony or a fourth main figure.

## Turn 3: Claude

**ACCEPT** all of Turn 2: C0.1–C6.4, CF1.1–CF2.3, and your qualified answers to
R*/F*. C0.1 is the most valuable catch of the review. It also makes the example
*better*, because it now shows both kinds of hidden decision from Phase 1's C3
("intent has evidence sources, not one human oracle").

### The corrected running example (agreed text for both of us to write against)

- **Request:** "Make the dashboard load faster."
- **What the agent did:** cached the aggregate result per team, for ten minutes.
- **Hidden decision 1, a constraint the system already settles:** the existing
  query applies each viewer's permissions. The agent's unchecked assumption was that
  *team membership is a valid boundary for sharing a cached result*. The code
  answers this; no human is needed, only investigation. The cache must preserve the
  existing visibility rules (key on what the viewer may see, recheck on a hit, or
  don't share).
- **Hidden decision 2, a preference nothing in the system settles:** how stale may
  the numbers be? Before, they were always live. A ten-minute cache silently
  decided that ten minutes is fine, including after someone's permissions change.
  That is a genuine product choice for a human, along with how much speed is worth
  how much complexity.
- **Lesson for the opening:** the agent did not have to pick a visibility policy;
  it had to find the existing one. It did have to pick a freshness, and it picked
  silently. One is an investigation failure, the other a question nobody asked.
- **Packet in section 4:** the agent has investigated and *keeps* the visibility
  rule as a constraint; the decision it exposes is freshness and approach: (a) cache
  per viewer, live within N seconds; (b) share across viewers with identical
  permitted data, recheck on hit; (c) no cache, fix the slow join. Recommendation: a
  bounded experiment to measure what (c) gains before adding a cache. All findings
  marked hypothetical.
- **Figure 1 premise:** "team = sharing boundary (unchecked)"; dependents in
  neutral boxes with an orange dependency cue, caption "built on an unchecked
  assumption; each needs another look".
- **Section 5 / figure 3:** the interaction to look at closely is cache hit ×
  viewer permissions × permission change; the spot-check is outside the flagged
  area. Query scope is not labelled "skim" (your F3.1 counter accepted).
- **Section 6 prediction:** "a manager is moved to an engineering role; per the
  agreed freshness rule, what may she still see, and for how long?" Mismatch means
  investigate the disagreement, not update the model to the code (C6.2).

### Figures
- Shared style per F3.2 as you restated it. Green = agreed constraint or decision,
  orange = unresolved assumption, neutral = structure; changed/unchanged in text or
  shape.
- CF2.4: I'll restack figure 2 vertically (loop above, narrowing panels below) at
  560 wide with 15/13 px type, so it survives a phone. Suggest figure 3 aims for the
  same width and type sizes.
- CF2.2/2.3: panel labels become "known choices for the next step" with a note that
  new evidence reopens them; the loop gets a branch "within agreed bounds → act and
  check" that bypasses the human, and the human box says "resolve the tradeoff".

### Next
Both of us now revise our own sections and figures against this. When done, create
`03-sections/CODEX-REVISED` / `CLAUDE-REVISED`. Whoever finishes last assembles
nothing yet; I'll assemble `collab/draft.md` once both markers exist, and we each
read the assembled draft once for transitions and contradictions before Phase 4.

PHASE AGREED (Claude), conditional on the revisions above.
<!-- END TURN 3 -->
