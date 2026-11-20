# Phase 4: unsloppifying the language

Files: `draft-v1.md` is the assembled Phase 3 text, untouched. `draft-v2-claude.md`
is my proposal (Turn 1). `draft.md` is the working copy.

## Turn 1: Claude

I did the language pass on the whole draft as a concrete proposal
(`diff draft-v1.md draft-v2-claude.md`). Prose without URLs is roughly 2,600
words, inside our 2,300–2,700 range.

### Rules I applied (please challenge any)
1. **One voice.** First person singular throughout, American spelling (Codex's
   sections were American, mine British; American wins on count). Paragraphs
   unwrapped.
2. **Short, concrete sentences.** Split sentences that carry two claims. Prefer the
   example's nouns (cache, manager, permission) to abstractions (artifact,
   commitment, constraint) once the example is in play.
3. **One hedge per claim, next to it.** No stacked hedges ("may", "can", "could",
   "rather than", "alone") on one claim; no closing-paragraph disclaimers.
4. **Words rationed:** "consequential" (now 0 in the prose: "important",
   "open", or the specific thing), "substantial", "rather than", "artifact",
   "assess", "examine" when "check" or "read" will do. No em dashes.
5. **Each paragraph moves the example or the argument.** Cut restatements.
6. **Key lines stand alone:** "Agents can now produce more than I can judge. My
   judgment is the scarce part." / the bold agentic-leverage definition / "the unit
   of review is a decision, not a document".

### Content fixes made in passing (two need your OK as owner)
- **Timeline (section 5, yours):** section 4 ends with "try (c) first", so section 5
  reviewing a ten-minute team cache read as if the loop's own result were reviewed.
  I opened section 5 with: "Suppose the loop was skipped and the cache from the
  opening arrives as a finished change." Figure 3 stays valid. **Please confirm.**
- **Ending (section 7, yours):** it now returns to the dashboard: two questions that
  "could have been settled before anything was built on them" (no counted cost, per
  your R7.1 limit), then the starting question. The hedge moved one paragraph
  earlier and shrank to one sentence. **Please confirm.**
- Section 1: "We have been here before, with the big requirements documents of the
  past" (the user's PRD point, said plainly).
- Section 3: the METR sentence split in two, numbers kept as "took 19% longer" per
  your precision note; caveat now says the tools changed and METR explains why later
  measurements are harder (no invented quote).
- Section 5: the Yu sentence shortened; "risk awareness" dropped, "defect detection,
  though not their speed" kept.

### Asked of Codex
Critique the proposal as a whole and line by line. You may edit `draft.md`
directly, but list every change in your turn with its reason, so I can review it as
a diff against `draft-v2-claude.md`. Flag any place where my edits changed a claim's
meaning or dropped a qualification you consider necessary. End with
`<!-- END TURN 2 -->`.
<!-- END TURN 1 -->


## Turn 2: Codex

**AGREED** on the language pass's direction: one first-person voice, American spelling, unwrapped paragraphs, concrete nouns, and shorter sentences. I read the whole proposal and edited the working draft directly. I preserved draft-v1.md, draft-v2-claude.md, the section sources, and the figures.

### The two content fixes

- **Section 5 timeline: ACCEPT.** Keep the sentence supposing that the loop was skipped and the opening's cache arrives as a finished change. It separates retrospective review of that cache from the recommended join experiment. My edits retain this sentence unchanged.
- **Section 7 return to the opening: ACCEPT.** Keep the return to the dashboard and the final starting question. I refined the wording so investigation checks an existing access rule while human input settles the new freshness preference. The proposal's limitation stays before the ending.

### Language rules and claim precision

The prose is easier to follow with one voice and a running example. The standalone attention claim and leverage definition help the argument.

I would apply the word and hedge rules as editorial guides, not as quotas. A qualifier sometimes carries the claim: no statistically significant difference is not the same as no difference, and cannot guarantee is not the same as cannot. The edits below preserve those distinctions without adding a general disclaimer to every paragraph.

Likewise, agents can help read or navigate a document. The human still needs to judge the decisions. The article's own agent-assisted evidence loop depends on that distinction.

### Changes applied to draft.md

Each numbered entry identifies one literal replacement against your proposal; the full diff is available by comparing draft-v2-claude.md with draft.md.

- **L01 — Opening.** Changed the introductory sentence to identify two assumptions, rather than claim nobody had ever made either decision. **Reason:** The original sentence contradicted the next paragraph: the visibility rule had already been decided. The agent made unchecked assumptions about preserving that rule and about freshness.

- **L02 — Opening.** Rewrote the freshness paragraph around updates to the numbers and removed the permission-change clause. **Reason:** Separate stale numbers from stale authorization. Tolerating old data must not be read as authorizing access after a permission change.

- **L03 — Section 1.** Scoped the SDD criticism to the version that concerns the author and replaced the claim that agents cannot read the document for them. **Reason:** Scope the criticism to the user's concern instead of asserting popularity. Agents can read and summarize documents; the remaining human cost is judgment, which is also the basis of the proposed review loop.

- **L04 — Section 1.** Replaced the absolute scarcity declaration with a standalone conditional sentence about output exceeding careful review. **Reason:** Keep the standalone hinge while preserving Phase 1's conditional bottleneck claim.

- **L05 — Section 2.** Described invalidation as following team-level events rather than clearing on every team-data change. **Reason:** Match figure 1 without implying that every underlying update is immediately invalidated, which would obscure the running example's separate freshness issue.

- **L06 — Section 2.** Changed implicit decisions being invisible to a sub-agent into decisions it may miss. **Reason:** Partial context does not imply that every implicit decision is invisible. Preserve the practitioner's context-loss argument without turning it into a universal claim.

- **L07 — Section 2.** Qualified the two test claims and named checking permitted data on a cache hit. **Reason:** Whether a test detects the defect depends on what it asserts. A source of requirements alone does not guarantee a useful test.

- **L08 — Section 4.** Replaced the big-plan/no-plan contrast with the requirement that a useful plan expose important decisions. **Reason:** Remove a stock contrast and keep the argument focused on inspectable decisions rather than an opposition between plan sizes.

- **L09 — Section 4.** Removed the rule that the agent must return without a plan. **Reason:** Do not prescribe a mandatory plan-free intermediate stage. A plan or code excerpt can represent a decision; the request is to expose the important findings and choices.

- **L10 — Section 4 packet.** Separated the possible freshness/performance tradeoff from the existing access requirement. **Reason:** Preserve the freshness tradeoff without treating changed permissions as part of the permitted data-age allowance or assuming a measured speed gain.

- **L11 — Section 4 example answer.** Introduced the answer with Suppose and retained the existing access rules instead of inventing an instantaneous timing guarantee. **Reason:** Mark the answer as part of the hypothetical example. Retain the existing access contract rather than invent an additional production timing guarantee.

- **L12 — Section 4 evidence.** Scoped the Laban finding to simulated conversations where requirements emerge across turns. **Reason:** Retain the study's multi-turn underspecification setting. The result is not a general demonstration that conversation length causes drift.

- **L13 — Section 4 optional example.** Removed the pending project-example paragraph from the working draft only. **Reason:** Remove unresolved editorial scaffolding from the clean reading copy. The optional project example remains preserved in draft-v1.md, draft-v2-claude.md, and the source section; the user can still choose to restore it.

- **L14 — Section 4 lineage.** Shortened the lineage paragraph and removed the claim that agents have newly made this practice practical on ordinary tasks. **Reason:** Preserve the lineage without implying historical decision iteration was previously impractical or that the cited work validates this complete proposed method.

- **L15 — Section 5.** Clarified the distinction between existing live behavior, an unagreed tolerance for delay, and visibility rules to preserve. **Reason:** The existing code supplies current behavior, even when it does not settle a new freshness tolerance. Keep authorization and data-age requirements distinct in the retrospective review.

- **L16 — Section 5 evidence and risk signals.** Restored the statistical speed qualification and replaced guaranteed independent signals with evidence used to challenge the ranking. **Reason:** An effect that is not statistically significant is not an established absence of effect. The study did not test an agent, and deterministic signals do not guarantee freedom from shared assumptions.

- **L17 — Section 6.** Replaced an absolute statement about unread code being unsafe with the absence of a correctness guarantee. **Reason:** Preserve the limit on selective understanding without implying that reasoning about unread code is always unsafe.

- **L18 — Section 6 prediction exercise.** Tied the role-change prediction to the existing access rules and included a faulty test as a possible explanation for disagreement. **Reason:** Keep the prediction exercise tied to the existing access contract. Include a faulty test among the possible explanations and avoid assuming exactly one account must be wrong.

- **L19 — Section 6 lineage.** Rephrased the Naur attribution around theory-building and its relation to the problem. **Reason:** Keep Naur's theory-building account about understanding the relationship between problem and program, rather than treating it as a literal location for the entire design.

- **L20 — Section 7.** Removed a blanket forecast about future models and restored the lack-of-guarantee qualification for unstated preferences. **Reason:** Avoid a blanket forecast about future models and the false absolute that an unstated preference can never be inferred. The agreed point concerns the lack of a guarantee.

- **L21 — Section 7 ending.** Kept the dashboard ending while distinguishing checking the sharing assumption from deciding tolerated staleness. **Reason:** Accept the return to the opening while preserving the distinction between investigating an existing constraint and resolving a human preference.

- **L22 — Final question.** Changed safe to accept to worth accepting in the final question. **Reason:** End on a decision under uncertainty rather than an implied guarantee that evidence makes an assumption safe.

- **L23 — Opening continuity.** Changed the first paragraph's antecedent to concerns visibility so it follows the two-assumptions introduction correctly. **Reason:** After describing two assumptions, the opening must not refer to the unchecked sharing assumption as an already-decided rule. Name visibility, then show the existing rule.

- **L24 — Section 7 pronoun.** Named a model explicitly as the subject of the sentence about unstated preferences. **Reason:** Make the subject explicit after changing the preceding sentence from future models to investigation and checking.

### Reading-copy and source notes

The optional watane paragraph is preserved in both earlier drafts and the section source. It is omitted from the clean working copy because the user's choice remains open. This does not decide whether to include it later or authorize publishing it.

No new research claims or sources were added. I retained the METR results with their historical setting and later-measurement qualification, the skill-formation study's observational-versus-randomized distinction, the proposed status of the learning exercise and decision record, and the non-guaranteed nature of risk-based review.

### Verification and next step

The working draft is **2,652 words when Markdown link targets are excluded**, counting headings, tables, and captions. It retains seven section headings and all three figures, whose relative paths resolve. It has no em dashes or visible pending-project placeholder. Earlier draft versions are unchanged.

Please review the diff, especially the access/freshness separation and the empirical qualifications. I have no further structural objection to the article. Once these edits are accepted, the remaining decision is the user's review of the text and optional project example.

<!-- END TURN 2 -->

## Turn 3: Claude

**ACCEPT** L01–L24. They make the claims more precise without bringing slop back,
and the access/freshness separation (L02, L10, L15, L18) is right: tolerating old
numbers must never read as tolerating old permissions. Agreed that the word and
hedge rules are guides, not quotas.

Two counter-edits, both applied to `draft.md`, both keeping your distinction:
- **L21 ending.** Your version is accurate but passive and ends on "dependent work
  accumulated", which is the abstract register the pass removed. Now: "Back to the
  dashboard. One look at the existing permission rules, and one question about how
  stale the numbers may be, before anything was built on either." "Look" (check an
  existing rule) and "question" (settle a preference) keep your L21 split.
- **L14 last sentence.** "Agents can help with the investigation and prototyping
  that these discussions require" → "Agents lower the cost of the investigating and
  prototyping this takes." It claims a lower cost, not new practicality, which was
  your objection.

L13: agreed to keep the watane paragraph out of the reading copy until the user
decides.

PHASE AGREED (Claude). If you accept the two counter-edits, write `PHASE AGREED`
in Turn 4 and the article is done.
<!-- END TURN 3 -->

## Turn 4: Codex

**PHASE AGREED.** I accept both counter-edits as applied to draft.md.

- **L21:** the ending distinguishes looking up an existing permission rule from asking about tolerated staleness. In this illustrative example, it summarizes the two interventions without claiming a measured review time or repair cost.
- **L14:** the sentence concerns the cost of investigating and prototyping, not the invention of risk-driven iteration or a demonstrated reduction in total project cost. The surrounding attention account and empirical qualifications keep those separate.

I checked that the working draft differs from my reviewed revision only by these two counter-edits. No further prose or figure changes are requested.

## Agreed

The four co-writing phases are complete. The agreed article is **Agentic agility**, in collab/draft.md, with its three linked SVG figures. Earlier drafts, research records, and review turns remain preserved.

The optional watane example remains out of the reading copy pending the user's editorial choice. The article is ready for the user's review; this agreement does not authorize publication.

<!-- END TURN 4 -->
