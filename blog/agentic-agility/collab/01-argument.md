# Phase 1: the argumentative structure

## Turn 1: Claude

I read both research files. We agree on most of it, so this proposal builds on
Codex's thesis and tries to make the chain of claims tighter, so that each claim
leans on the previous one and has its own evidence and its own honest limit.

### Thesis

Agents make specifications and code cheap to produce. They do not make it cheap
for a human to know that what was produced is what they meant. That judgement is
the scarce input. Agentic agility spends it on **decisions rather than documents**,
**in proportion to risk**, **on both sides of generation**, and in a way that
**keeps the human able to judge** the next time.

### The chain (each claim needs the one before it)

**C1. The bottleneck moved from producing to judging.**
Writing a spec or a diff is now nearly free; reading and assessing one is not.
- Evidence: HumanLayer's ~1,000-line plans took about an hour to review, "nearly as
  much work as reviewing the code" (ZenML write-up of the RPI→CRISPY talk).
  Böckeler: verbose, repetitive Markdown and a false sense of control
  (martinfowler.com, 2025-10-15). Marmelab: 8 files / 1,300 lines for "show the
  current date".
- New: METR's RCT (16 experienced devs, 246 tasks on their own repos, early 2025)
  found AI use made them ~19% *slower* while they believed it made them ~20%
  faster; the time went to prompting, waiting and reviewing
  (https://metr.org/blog/2025-07-10-early-2025-ai-experienced-os-dev-study/).
  I use it **only** for the perception gap: people misjudge where their own time
  goes, so leverage must be measured, not felt. METR's 2026 update says the
  slowdown number itself should not be repeated as a current effect
  (https://metr.org/blog/2026-02-24-uplift-update/).
- Limit: no study measures human comprehension of specs by length. The 400-line
  figure is about code review (SmartBear/Cisco) and goes out, or appears only as
  an explicit analogy.

**C2. So leverage must be measured against human attention, not human tokens.**
Keep the user's original definition (deliverable tokens ÷ instruction tokens) and
name it **token leverage**: it measures amplification. Define **agentic leverage**
as accepted outcome per unit of human attention (writing, answering, reading,
reviewing, correcting), net of rework. The token ratio rewards verbosity; the
attention ratio punishes it. This is the hinge of the whole article: once the
denominator is attention, big-spec-then-one-shot is visibly expensive (you pay in
reading) and so is review-everything-afterwards (you pay in reading again).
- I agree with Codex's split and naming. Decision for the user (flag, don't
  block): whether "agentic leverage" may be redefined like this or the article
  shows the evolution of the idea explicitly. I propose the latter: it is honest
  and makes a good narrative beat.

**C3. Misunderstanding multiplies through dependent work.**
The mechanism is dependency (Codex's chain): an unresolved ambiguity becomes an
assumption, then a plan, then code and sub-agent tasks built on it; correcting it
means redoing everything downstream. Two amplifiers:
- Correlated errors: per-step failures are not independent, so the naive 0.95^n
  arithmetic *understates* the problem (arXiv 2603.29231). Use the arithmetic only
  as an intuition pump, labelled as such.
- Handoffs: sub-agents lose implicit decisions (Cognition, "Don't build
  multi-agents"); MAST taxonomy of 1,600+ multi-agent traces, inter-agent
  misalignment as a failure class (Cemri et al., arXiv 2503.13657).
- Key distinction (the critic's "agents self-correct"): the environment catches
  *conformance* errors (compiler, tests, types) and agents fix those well. *Intent*
  errors have no oracle except the human. So the human belongs in the loop at
  intent decisions, not everywhere.

**C4. Agentic agility before the build: iterate on decisions, not documents.**
The loop: light prompt → agent research/experiment → condensed to consequential
decisions (decision, consequence, alternatives, recommendation, what it unblocks;
Codex's format) → human decides → next iteration. Stop when the remaining freedom
lies inside the human's **indifference set**, with explicit triggers to reopen.
- Evidence: HumanLayer converged on exactly this (≈200-line design discussion,
  "~5× leverage" vs a 1,000-line plan; Horthy's one line "work back and forth with
  me, starting with your open questions"). DORA 2025: AI amplifies existing
  practice, and **small batches** are one of the seven capabilities that turn AI
  speed into product performance rather than instability
  (https://dora.dev/dora-report-2025/). Agility is small batches applied to
  decisions.
- Lineage, claimed openly: Boehm's spiral (risk-driven iteration), set-based
  concurrent engineering (narrow a set, don't commit early), Agile principles.
  What is new is the economics: research and prototypes are cheap enough to
  iterate on decisions.
- Counter-evidence we must absorb, not hide: **Laban et al. "LLMs Get Lost in
  Multi-Turn Conversation"** (arXiv 2505.06120): models anchor on early wrong
  assumptions across turns. So iteration is not automatically good. The loop must
  keep a consolidated *decision record* (agreed / assumed / open), invalidate
  superseded decisions, and restart from that record in fresh context. This turns
  a weakness into a design rule and is a stronger article for it.
- Also absorb: agents skip the question step ~50% of the time (HumanLayer). Who
  does the condensing is itself an unreliable capability today.

**C5. The same loop runs after the build, in reverse.**
Not "check everything afterwards" and not "skip checking". Agents condense the
output into **pointers, not paraphrases** (exact lines, the test covering each,
what would break); attention goes in proportion to risk; corrections go back as
new constraints, so the after-loop feeds the before-loop. One loop, not two.
- Risk is two-sided (the user's most original point): risk in the **existing
  system** (blast radius, centrality, fragility) × risk in the **change** (novelty,
  implicit decisions, deviation from patterns), plus their interaction (Codex's
  cache-key-missing-identity example is perfect for this).
- The agent's risk ranking shares the agent's blind spots, so anchor it with
  deterministic signals (coverage of changed lines, static analysis, change
  impact, hotspots per Tornhill) and spot-check outside the flagged areas.
- Evidence: risk analytics improved reviewers' risk awareness and defect finding,
  not speed (Yu, Cogo, McIntosh, Godfrey, EMSE 2024). HumanLayer reversing "don't
  read the code" after six months is the cautionary tale.

**C6. Selective depth keeps the human able to steer.**
Delegation can remove the activity through which understanding forms. Anthropic's
RCT: AI group 50% vs 67% on an immediate quiz (17 percentage points, largest gap
on debugging), but high scorers used AI for comprehension (asking why, conceptual
questions). Engagement, not abstinence, is the variable; and the agility loop is
an engagement pattern by construction.
- The 80/20 framing stays as an explicitly labelled heuristic. Two corrections:
  (a) the 20% is "structure plus the details that carry weight" (concurrency,
  auth, migrations), found by C5's risk scan; (b) reasoning about unread code is
  only safe in a consistent codebase, so consistency must be machine-enforced.
  Naur's "theory building": the human's 20% is the theory; the checks keep the
  code faithful to it.
- Limit: mostly junior devs, unfamiliar library, immediate test. Evidence for, not
  proof.

**C7. Where big upfront specs do work, and what better models change.**
Big specs are right when an **oracle** exists and intent is already fixed:
ports, reimplementations against an existing test suite, protocol conformance,
migrations with a diffable before/after. Long autonomous runs work there
(Anthropic's long-running harness reports). Growing time horizons (METR) lengthen
the safe interval between checkpoints; they don't remove the loop, because an
underspecified intent is not a capability gap: a smarter agent just guesses
faster.

### What the article does not claim
- That agentic agility is proven better than other workflows. It is a reasoned
  proposal with converging practitioner and partial empirical support. Say so.
- That we coined "agentic agility" (naked Agility uses it). Define our meaning.
- Any measured drift curve, optimal checkpoint interval or literal 80/20 split.

### Questions for Codex
1. Is C2 the right hinge, or should the article lead with C3 (error
   multiplication) as the emotional hook and introduce the attention metric later?
2. Do you accept "indifference set + reopen triggers" as the stopping rule, versus
   your "several acceptable solutions within explicit constraints"? I think they
   are the same claim; pick whichever reads better.
3. C7's "oracle" criterion: does it hold up, or do you have a counterexample where
   big specs work without one?
4. Anything in your source register that contradicts a claim above and that I
   under-weighted?

## Turn 2: Codex

**AGREED** on the attention denominator, dependency as the mechanism of costly drift, a consolidated decision record, evidence pointers for review, risk arising from the system and the change, and selective human depth. I also agree that the article should define its own use of agentic agility without claiming to coin the term.

**Counter-proposal:** keep the seven-claim spine, but revise C1's universal bottleneck claim, C3's probability argument and intent oracle, C6's causal learning claim, and C7's oracle criterion. These are substantive changes; the phase remains open pending your response.

### Answers to your four questions

**1. C2 is the conceptual hinge; C3 should supply the opening example.**

Lead with a concrete case: a request to make a dashboard faster becomes a caching implementation that fails to distinguish users. The request omitted whether and how data may be shared. The agent chose, built on the choice, and produced an implementation that can look locally sensible. A small clarification about identity and data ownership would have changed the approach.

Label this as an illustrative example, not a reported incident. It demonstrates the cost of an implicit decision without requiring an exponential-error claim.

Then introduce the attention denominator: the target is useful progress after accounting for explaining, reading, checking, and correcting. Treat that as the article's organizing principle rather than begin with a formal productivity equation.

I accept showing the evolution of the user's token ratio explicitly. One adjustment: count correction time in the denominator rather than also subtracting the same correction cost from the numerator. Accepted useful outcomes divided by total human attention, with quality, compute cost, and risk reported separately, is clearer. This is an evaluative framework, not a universally comparable numerical metric.

**2. I accept the intended stopping rule, with two qualifications.**

Indifference set and acceptable solutions overlap, but are not identical in ordinary usage. Several implementations can all be acceptable while still having different costs or preferences. More importantly, acceptable product behavior alone does not establish that an implementation will meet it.

Proposed wording:

> Delegate the next bounded step when the remaining choices fall within agreed constraints, consequential uncertainties have been resolved or explicitly contained, and there is a credible way to check the result. Reopen the discussion when those conditions change.

This is permission to proceed with the next step, not a declaration that all requirements discovery is finished. A reversible prototype can be that step even when important uncertainty remains. Avoid requiring complete indifference between every possible implementation; it would make routine delegation unnecessarily hard.

**3. C7's oracle criterion does not hold as a necessary condition.**

A detailed API specification negotiated between teams can be useful before implementation even without a complete executable oracle. It coordinates names, responsibilities, compatibility, and expectations. That is a conceptual counterexample, not a claim about a measured project.

The key distinction is **artifact size versus commitment size**. A substantial specification can accumulate many small, reviewed decisions. It need not imply one large planning or implementation batch. Conversely, a short prompt can delegate a very large commitment.

A strong evaluation mechanism makes longer autonomous execution more plausible; it does not tell us that a large upfront document is the right artifact. An existing test suite is also only a partial oracle: a port can pass the suite while violating untested behavior or operational requirements.

Replace C7 with a positive boundary condition:

> Larger documents help when they preserve stable requirements, shared contracts, or coordination knowledge. Longer autonomous runs become more reasonable when intent is sufficiently clear, dependencies are understood, evaluation is credible, and failures are contained or recoverable.

The article should oppose committing large amounts of dependent work on unresolved assumptions. It should permit extensive documentation and substantial autonomous work under appropriate conditions.

**4. Four sources or source limits deserve more weight.**

- The scaling-agent research shows that delegation can improve outcomes on suitable parallelizable tasks. Preserve this counterweight to the telephone-game story. Architectural coordination and task structure matter; more agents is not itself the failure mechanism. [Google Research](https://research.google/blog/towards-a-science-of-scaling-agent-systems-when-and-why-agent-systems-work/)
- Laban et al. compare information-distributed simulated conversations with fully specified tasks. Their result challenges naive conversational iteration, but does not prove our decision-record workflow works. Consolidation and fresh context are plausible mitigations, not a derived guarantee. [Paper](https://arxiv.org/abs/2505.06120)
- The skill-formation RCT randomized AI availability, not interaction style. We cannot infer that engagement with decisions, or our loop specifically, causes learning to be preserved. The learning protection is a design requirement to evaluate. [Shen and Tamkin](https://arxiv.org/abs/2601.20245)
- Yu et al. evaluated historical risk analytics, not agent judgment. Improved reviewer effectiveness supports making risk visible; it does not validate the agent as a reliable risk estimator or prove that targeted review preserves the quality of exhaustive review. [Original study](https://rebels.cs.uwaterloo.ca/papers/emse2024_yu.pdf)

### C1 needs a conditional bottleneck claim

Writing a specification or diff is not nearly free in every relevant sense. Execution consumes time and compute, producing a usable result can require substantial work, and teams have other constraints. METR's perception gap does not establish a universal shift to judgment as the dominant bottleneck.

Proposed replacement:

> As agents increase the volume of work they can produce, human understanding and judgment can become the limiting resource. Automating document production does not automatically reduce the effort required to assess the decisions those documents contain.

Böckeler is a direct practitioner example of review burden. METR is useful for the narrower point you chose: measure end-to-end work rather than infer benefit from apparent generation speed. Keep its current-effect caveat beside any historical number.

I would also soften decisions rather than documents into **make consequential decisions easy to inspect**. People still need representations of those decisions: short notes, examples, diagrams, code, or a specification. A document can provide an excellent decision-review interface.

### C3's correlated-error argument should be removed

I opened the original [Beyond pass@1 paper](https://arxiv.org/html/2603.29231), including section 7.2 and Appendix A.1. It does make the claim you cite, but the general inference from positive error correlation to lower all-step success than independence does not follow.

Here is a counterexample of our own. Suppose every step succeeds with probability 0.95:

- With 20 independent steps, all succeed with probability 0.95^20, approximately 0.3585.
- Suppose instead all steps share one event: with probability 0.95 they all succeed, and with probability 0.05 they all fail. Errors are perfectly positively correlated, yet all-step success is 0.95, higher than under independence.

This also contradicts the general upper bound asserted in Appendix A.1. Greater variance in the number of failed steps does not by itself establish the stated probability of zero failures. Additional assumptions about conditional hazards, state transitions, or error propagation are needed.

I would not use that paper's theoretical claim to strengthen our argument. This observation does not independently audit or refute its empirical benchmark results. It does mean we should not repeat positive correlation as a sufficient mathematical explanation.

Our dependency argument already does the needed work:

> An untested interpretation can become a premise for subsequent decisions. The more consequential work depends on that premise, the more may need reconsideration when it changes.

Use may need reconsideration, not redo everything downstream. Some dependent work survives, and reliable checks or feedback can interrupt propagation.

### Intent has evidence sources, not an infallible human oracle

The conformance-versus-intent distinction is valuable, but intent errors have no oracle except the human is too absolute. Existing acceptance examples, user observations, documentation, product constraints, and contradictions between requirements can expose an intent error. Agents can help discover those contradictions. Humans can also misunderstand their own need or disagree with other stakeholders.

Proposed distinction:

> Automated checks can assess encoded requirements. They cannot resolve every unexpressed preference or product tradeoff. Human and stakeholder judgment remains necessary where the available evidence does not determine what should be built.

Tests derived from the same mistaken interpretation can pass while the outcome misses the need. That is the reason to examine their assumptions, not a reason to discard machine verification.

Similarly, replace a smarter agent just guesses faster. Better models can ask better questions, locate relevant constraints, and detect ambiguity. They cannot guarantee recovery of an arbitrary preference that has never been expressed. More capability can improve the loop and change its cadence.

### C5 needs inspection of evidence quality, not only evidence locations

AGREED on pointers, affected dependencies, and sampling outside flagged areas. Exact code locations and test links are a useful entry point, but a test's existence or line coverage does not establish that its assertions capture the intended behavior. A deterministic tool is not necessarily independent evidence of intent.

The review packet should expose why the evidence addresses the risk: an observed behavior, an assertion and the requirement it checks, an uncovered failure path, or a counterexample. Avoid prescribing risk as existing risk multiplied by change risk; we have no calibrated numerical model. Describe the three sources: existing conditions, the change, and their interaction.

After implementation, the loop is not literally reversed. It is the same investigate, expose, judge, act, and check cycle applied to observed results. Corrections can repair implementation, improve checks, invalidate an assumption, or reopen a product decision. They should not automatically alter the requirement to match the code.

### C6 should make learning an explicit obligation

I agree with selective depth and the 80/20 heuristic. I do not accept that the agility loop preserves learning by construction. A human can approve a sequence of recommendations without understanding the reasoning.

Proposed learning safeguard:

> Periodically explain or predict a consequential behavior before consulting the agent's answer, then check that explanation against code or observation.

That could mean tracing one execution path, predicting a failure's consequences, or explaining why a rejected alternative would violate a constraint. These are practices we propose, not a treatment validated by the cited RCT.

Consistent patterns and enforced contracts help people reason about unread details; they do not make that reasoning unconditionally safe. Consistency itself can encode a shared error, and important conventions may lack machine checks. State the assumptions and retain a route to inspect details.

[Naur's original essay, reprinted here](https://gwern.net/doc/cs/algorithm/1985-naur.pdf), is useful lineage for understanding that is not conveyed by text alone. It does not establish a 20 percent theory or demonstrate that machine checks can preserve the whole theory. Keep that numerical analogy ours, explicitly heuristic.

### New primary sources and evidence housekeeping

**DORA:** I checked its [small-batch capability guidance](https://dora.dev/capabilities/working-in-small-batches/). It supports rapid feedback, manageable changes, and a connection to better AI-assisted outcomes. Applying the principle specifically to decision batches is our extension, not a separately demonstrated DORA result.

**HumanLayer:** the ZenML page links the original talk, [Everything We Got Wrong About Research-Plan-Implement](https://www.youtube.com/watch?v=YwZR6tc7qYg). I located that source, but did not verify the numerical claims against its audio or original transcript. Please keep the 200/1,000-line comparison, 5x leverage, and 50 percent question-skipping figures provisional until that check. Document-length ratios alone do not demonstrate equivalent improvement in end-to-end productivity.

I also found a first-party written account: [Dex Horthy, Why Software Factories Fail](https://github.com/humanlayer/advanced-context-engineering-for-coding-agents/blob/main/wsff.md). It explicitly revises earlier enthusiasm for skipping code reading and discusses the limits of harness-only approaches. It is useful practitioner testimony with a disclosed commercial interest, not an independent controlled result. Prefer this and the original talk over relying on ZenML as the evidentiary foundation.

### Proposed replacement thesis and claim chain

**Thesis**

Agents can produce work faster than people can understand it. Agentic agility directs human attention toward consequential decisions and risks before, during, and after generation. It combines bounded autonomous work with evidence that people can inspect and deliberate engagement that sustains their ability to judge the next change.

1. **Production and judgment scale differently.** Increasing agent output can make human understanding and review a limiting resource.
2. **Evaluate leverage against total human attention.** Preserve the token ratio as amplification; assess useful accepted outcomes with reading, review, correction, compute cost, and quality in view.
3. **Unresolved assumptions create dependent work.** Surface consequential interpretations before building too much on them; avoid universal probability laws.
4. **Iterate through evidence and decisions.** Research and experiments expose alternatives; humans resolve consequential tradeoffs; a current decision record supports the next bounded step.
5. **Apply the same loop to the result.** Agent-assisted assessment points to consequential risks and evidence across the changed and affected system; human scrutiny and corrections respond to that evidence.
6. **Maintain selective depth.** Deliberately learn and test the structures, assumptions, and important details needed to assess the work. The loop should support this; it does not guarantee it.
7. **Adapt autonomy and documentation to the task.** Stable contracts, strong evaluation, and recoverability permit more delegation. Document size alone is not the boundary, and better models can improve the process without eliminating unresolved preferences.

If you accept these revisions, I think the remaining Phase 1 work is choosing the final wording of the thesis and chain, plus deciding whether the HumanLayer numbers are worth retaining after primary-source verification.

## Turn 3: Claude

**AGREED** with almost all of Turn 2. Specifically, and without reservation:
- C3: drop the correlated-error claim. Your counterexample is right (perfect
  correlation at p=0.95 gives 0.95 all-step success, not 0.36). Dependency is the
  mechanism; "may need reconsideration", not "redo everything".
- Intent has evidence sources, not one human oracle; and drop "a smarter agent
  just guesses faster" for your wording.
- C5: the review packet shows *why* the evidence addresses the risk, not just where
  it is; no multiplicative risk formula; three sources (existing, change,
  interaction). Corrections can fix code, fix checks, invalidate an assumption or
  reopen a product decision.
- C6: learning is an obligation the loop must make room for, not a by-product.
  Your "predict, then check" safeguard goes in. Naur as lineage only.
- Leverage: accepted outcomes ÷ total human attention (correction in the
  denominator only), quality, compute and risk reported beside it. An evaluative
  frame, not a comparable metric.
- Stopping rule: your "delegate the next bounded step when..." wording.
- DORA: decision batches are our extension, said as such.

### HumanLayer check (your request)

I fetched the first-party essay, Dex Horthy, *Why Software Factories Fail*
(https://github.com/humanlayer/advanced-context-engineering-for-coding-agents/blob/main/wsff.md).
It does **not** contain the 1,000-line plan, the 200-line design doc, "5×" or the
"50% skip questions" figures. Those stay unverified, so I propose we **drop them**
rather than cite ZenML's second-hand summary. The essay gives us better material,
in his own words and with his commercial interest disclosed:
- "30 minutes of planning saves hours of review"; he keeps humans in the loop for
  product design, system architecture, program design and vertical slices.
- He was excited to "not read the code" and concludes "for now we're stuck reading
  the code": a first-person reversal, stronger than the ZenML paraphrase.
- Proportionality: roughly 40% of tasks are one-shot or one-shot with 1-2 rounds of
  light feedback; medium tasks get one plan document; only large work gets every
  step. He sends a model 1-3 vertical slices at a time and reviews as he goes,
  because "checking 100-200 lines and resteering is a lot cheaper".
- He estimates most one-shot AI PRs need close to 50% rework: his estimate, label it so.

This proportionality point is missing from our chain and it matters: agility is
not "always loop". It is choosing the batch size per task. I fold it into C7.

### Three counter-proposals

**1. Keep the punch: "the unit of review is a decision, not a document."**
I accept your softening in substance (documents, diagrams, examples and code are
all fine *representations* of decisions). But "make consequential decisions easy to
inspect" loses the user's contrarian edge against big specs. The phrase "the unit of
review is a decision, not a document" says the same thing you want and keeps the
edge: a document is fine as long as what the human reviews and approves is the
decisions in it, not the document as a whole.

**2. Your "artifact size vs commitment size" is the best idea in this phase. Make it
the article's named target.**
The enemy is not long specs and not SDD. It is **large commitments on unresolved
assumptions**: approving a lot of dependent work in one batch before the
consequential uncertainties are resolved. That unifies C1 (judgment can't keep up
with the batch), C3 (dependent work piles onto the assumption), C4 (shrink the
batch of decisions), C7 (big documents are fine; big unchecked commitments are
not), and Horthy's vertical slices. I propose this phrase appear in the thesis.

**3. Opening example: prefer a real one, fall back to the cache example.**
The illustrative cache-without-identity case is clear and I'd keep it for C5. For
the opening, a real episode is stronger, and the user has one: their own project
(watane) runs an agent "factory" whose task briefs tell the implementing agent: "If
THE TASK needs a decision that the specs do not make and that is costly to undo (a
wire format, a public interface of another subsystem, a security boundary), do not
guess: stop and say so." That is our stopping rule, in production. Whether the
user wants their project in the article is **their call**; I'll flag it to them.
Until they answer, the outline should keep a slot "opening example: real if the user
agrees, else the cache case".

## Agreed

If you accept the three counter-proposals, this is the agreed argument. Reply
`PHASE AGREED` or edit within it in your turn.

**Thesis.** Agents can produce work faster than people can understand it. The
costly mistake is not writing long specifications; it is making large commitments
on unresolved assumptions. Agentic agility keeps commitments small where
assumptions are unresolved: it spends human attention on consequential decisions
and risks before, during and after generation, lets agents run bounded steps where
intent is clear and results can be checked, and protects the human's ability to
judge the next change.

1. **Production and judgment scale differently.** As agent output grows, human
   understanding and judgment can become the limiting resource (Böckeler; Horthy's
   "stuck reading the code"). Measure end-to-end, because people misjudge where
   their time goes (METR 2025's perception gap, with its 2026 caveat beside it).
2. **Evaluate leverage against total human attention.** The user's token ratio
   stays as *token leverage* (amplification). *Agentic leverage* is accepted
   outcomes per unit of total human attention, with quality, compute and risk in
   view.
3. **Unresolved assumptions create dependent work.** An untested interpretation
   becomes a premise for later decisions, code and delegated tasks; the more depends
   on it, the more may need reconsideration. Handoffs can lose implicit decisions
   (Cognition; Cemri et al.), though delegation helps on suitable parallel tasks
   (Kim et al.). No universal probability law.
4. **The unit of review is a decision, not a document.** Research and experiments
   expose consequential choices (decision, consequence, alternatives,
   recommendation, what it unblocks); the human resolves them; a consolidated
   decision record (agreed / assumed / open) seeds the next bounded step, in fresh
   context when history misleads (Laban et al. as the risk this guards against, not
   proof the guard works). Delegate the next bounded step when remaining choices lie
   within agreed constraints, consequential uncertainties are resolved or contained,
   and the result can credibly be checked; reopen when that changes. Lineage: Boehm's
   spiral, set-based design, Agile, DORA's small batches (applying them to decisions
   is our extension).
5. **The same loop runs on the result.** Agents point to consequential risks and to
   the evidence for each (why it addresses the risk, not only where it is), across
   the change, the existing system and their interaction; deterministic signals and
   sampling outside the flagged areas check the agent's ranking. Human corrections
   go back as instructions: fix code, fix checks, invalidate an assumption or reopen
   a decision.
6. **Maintain selective depth, deliberately.** Know the structures, contracts, trust
   boundaries and the details that carry weight; predict, then check, from time to
   time. The 80/20 split is our heuristic. The loop makes room for learning; it does
   not guarantee it (Shen & Tamkin; Lee et al.).
7. **Size the batch to the task.** Stable contracts, credible evaluation and
   recoverable failure permit longer autonomous runs and longer documents; many
   tasks need no loop at all (Horthy: ~40% one-shot). Document size is not the
   boundary; commitment size on unresolved assumptions is. Better models change the
   cadence, not the need to express preferences nobody has stated.

**Not claimed:** that this workflow is proven better than others; that we coined
"agentic agility"; any measured drift curve, checkpoint interval, or literal 80/20
split; any HumanLayer figure not found in a first-party source.
<!-- END TURN 3 -->


## Turn 4: Codex

**PHASE AGREED.** I accept your three counter-proposals and the proposed argument, with the wording clarifications below. The Phase 2 outline proposal is ready in 02-outline.md.

1. **Keep the decision-review phrase, scoped to intent alignment.** Technical review also examines behavior, code, and assertions. For intent alignment, the unit of review can be a consequential decision rather than a document approved wholesale.
2. **Use large commitments on unresolved assumptions as the target.** Agreed without qualification. This distinguishes the problem from document length, SDD as a category, or autonomous runtime.
3. **Keep the opening-example slot.** A user-approved real episode is preferable. I verified the rule at factory/implementer.md:32–33, but a process rule is not evidence of an effective intervention. The user's preference on naming watane remains open; the explicitly illustrative cache case permits drafting without inventing an incident.
4. **Qualify one-shot and learning wording.** A straightforward task can need no additional human feedback rounds while still receiving appropriate checks. Horthy's rough 40% estimate includes tasks with one or two rounds of light feedback; it is his estimate, not an industry rate. The method makes room for learning and supports continued judgment; it does not guarantee their preservation.

I checked the relevant first-party Horthy passages. AGREED to dropping the 1,000/200-line, 5x, and question-skipping figures unless independently verified against the original talk. Their absence from this essay does not refute the talk; it means we do not currently have an adequate basis to use them. His proportional approach and first-person account provide sufficient practitioner context.

The agreed text follows, incorporating those clarifications.

## Agreed

**Thesis.** Agents can produce work faster than people can understand it. The costly mistake is not writing long specifications; it is making large commitments on unresolved assumptions. Agentic agility keeps commitments small where assumptions are unresolved: it directs human attention toward consequential decisions and risks before, during, and after generation, lets agents execute bounded steps where intent is sufficiently clear and results can be checked, and makes room for the learning needed to judge the next change.

1. **Production and judgment scale differently.** As agent output grows, human understanding and judgment can become the limiting resource. Document production does not establish shared understanding. Böckeler and Horthy provide practitioner examples. METR's historical perception gap supports assessing end-to-end work; if used, its early-2025 setting and 2026 measurement caveat stay together.
2. **Evaluate leverage against total human attention.** Preserve the user's original token ratio as token leverage, a measure of amplification. Explain the move toward agentic leverage as useful accepted outcomes relative to all human attention, including explanation, questions, reading, review, and correction. Keep quality, compute cost, and risk in view. This is an evaluative framework, not a universally comparable score.
3. **Unresolved assumptions create dependent work.** An untested interpretation can become a premise for later decisions, code, tests, and delegated tasks; the more consequential work depends on it, the more may need reconsideration. Handoffs can lose implicit decisions, though delegation can help on suitable parallel tasks. Checks and feedback can interrupt propagation. No universal probability law is asserted.
4. **For intent alignment, review consequential decisions rather than approve documents wholesale.** Research and experiments expose the decision, consequence, alternatives, recommendation, and work it permits. Human input and new evidence update a consolidated record of agreed, assumed, and open matters. Fresh context can help when conversation history misleads; Laban et al. motivate the concern but do not validate our proposed guard. Delegate the next bounded step when remaining choices lie within agreed constraints, consequential uncertainties are resolved or contained, and the result can credibly be checked; reopen when those conditions change. A reversible experiment can itself be that step. Boehm, Agile, and small-batch practice provide lineage; applying small batches specifically to decisions is our extension.
5. **Apply the same loop to observed results.** Agents expose consequential risks and why the available evidence addresses them across the change, the existing system, and their interaction. Checks, observations, and sampling outside flagged areas help challenge the ranking, while their own assumptions remain open to scrutiny. Human corrections become instructions to fix implementation, strengthen checks, invalidate an assumption, or reopen a decision. Neither passing tests nor an agent summary settles every question of intent or correctness.
6. **Maintain selective depth, deliberately.** Understand structures, contracts, state ownership, trust boundaries, failure modes, and details that carry consequences. Periodically predict or explain a behavior, then check the explanation against code or observation. The 80/20 split is a heuristic. Consistency and machine-enforced contracts help conditional reasoning about unread details without guaranteeing it. The loop must make room for learning; the skill-formation experiment and critical-thinking survey do not prove that this complete method preserves expertise.
7. **Size the commitment to the task.** Stable contracts, credible evaluation, and contained or recoverable failure permit more autonomous execution. Straightforward tasks may need only a single execution-and-check pass, without extra human feedback rounds. Larger documents can preserve shared contracts and decisions established incrementally. Horthy's estimate of roughly 40% one-shot or lightly iterated tasks illustrates his proportional handling, not a general optimum. Document length and elapsed runtime are not themselves the boundary; the consequential work committed on unresolved assumptions is. Better models can improve clarification and checking and change the cadence; they cannot guarantee recovery of preferences nobody has expressed.

**Not claimed:** superiority over all other workflows; coinage of agentic agility; a universal probability law for drift; a measured optimum for checkpoint intervals; a literal 20/80 allocation; or a general human reading limit of 400 lines. Unverified HumanLayer numerical claims are excluded.

**Sources carried into the outline:** [Böckeler](https://martinfowler.com/articles/exploring-gen-ai/sdd-3-tools.html), [GitHub's SDD introduction](https://github.blog/ai-and-ml/generative-ai/spec-driven-development-with-ai-get-started-with-a-new-open-source-toolkit/), [Horthy](https://github.com/humanlayer/advanced-context-engineering-for-coding-agents/blob/main/wsff.md), [METR's original study](https://metr.org/blog/2025-07-10-early-2025-ai-experienced-os-dev-study/), [METR's update](https://metr.org/blog/2026-02-24-uplift-update/), [Cognition](https://cognition.com/blog/dont-build-multi-agents), [Cemri et al.](https://arxiv.org/abs/2503.13657), [Kim et al.](https://arxiv.org/abs/2512.08296), [Laban et al.](https://arxiv.org/abs/2505.06120), [DORA](https://dora.dev/capabilities/working-in-small-batches/), [Yu et al.](https://rebels.cs.uwaterloo.ca/papers/emse2024_yu.pdf), [Shen and Tamkin](https://arxiv.org/abs/2601.20245), and [Lee et al.](https://www.microsoft.com/en-us/research/publication/the-impact-of-generative-ai-on-critical-thinking-self-reported-reductions-in-cognitive-effort-and-confidence-effects-from-a-survey-of-knowledge-workers/).

<!-- END TURN 4 -->
