# Agentic agility in software development

Source: https://claude.ai/chat/356349c1-56cf-4b52-ab73-17fabb068d56 (exported 2026-11-20)

---

## You (2026-11-20 07:45)

There's something that I want to write a blog article about: we have approaches in agentic software development like spec-driven design, and in particular, the interpretation of that, that you should be building big specifications upfront and then one shot their implementation. I see this as flawed for the same reason as big PRDs in the past were flawed. Yes, we now have agents to maintain those documents for us, but in order to assess whether we are aligned, I still need to read them. And humans do a poor job of reading anything beyond 400 lines in one go (or so I hear).

There's also this idea that everything needs to be checked retroactively. That is not feasible if we really want to harness the power of Agentic software development.

Then there is this notion of what I call agentic leverage. It's the leverage you get by providing input tokens and the agent providing effective output tokens. By effective output tokens in this case I mean to exclude thinking tokens, the tool calls, and the starting of other agents in order to produce the effective result measured in tokens. In a way, we seek to maximize our agentic leverage, Producing minimal Instructions for the agent with maximum effect. What we see in practice is that errors compound. The more a single agent starts to follow a trajectory that's not aligned with your intention, the worse it gets. And through the telephone game, it will also instruct other agents to do things that are increasingly not aligned with your intention.

The proper middle ground is agentic agility:

* Provide enough instructions so that the agent can start working on a better definition of why something needs to happen, or what needs to happen, or how it should be done.
* Research gets condensed to just the pain points, the crucial/risky/costly decisions to be made, the ambiguities to resolve.
* The human provides enough input on each of these so that the agent can start a next iteration.
* The human and agent iteratively shape / constrain the solution space until one is carved out that matches the human intent regardless of further implementation details.


This allows me to use the Agentic leverage, while preventing error compounding from escalating / spiraling.

These are just my rough draft notes / thoughts. Before you write this down, let's see if we can provide some structure to these thoughts. And maybe also explore some visualizations of the ideas. Do some digging into research and blog articles that go over the same topics that I should be referencing. And critically assess my thoughts on the basis of that.

---

## Claude (2026-11-20 07:48)

I'll research the existing work first, then give you a proposed structure, a critical assessment and a couple of draft visuals.

> _Searched the web: Böckeler spec-driven development Kiro spec-kit Tessl martinfowler_

> _Searched the web: spec-driven development waterfall strikes back_

> _Searched the web: Cognition "Don't build multi-agents" context sharing_

> _Searched the web: SmartBear Cisco code review study 400 lines of code defect detection_

> _Searched the web: HumanLayer advanced context engineering research plan implement bad line of research_

> _Searched the web: AI agents compounding errors per-step reliability multi-step tasks math_

> _Read https://www.zenml.io/llmops-database/evolving-ai-coding-agent-workflows-from-research-plan-implement-to-crispy_

> _Searched the web: comprehension debt AI generated code review bottleneck_

> _Searched the web: Böckeler spec-driven development review markdown files "rather review code"_

> _Searched the web: METR AI task time horizon doubling 2026_

I've structured your notes, checked them against what's been published, and drafted two visuals. Short version: your core argument holds up, and the strongest evidence for it comes from a team that already learned it the hard way. But three of your claims will draw pushback as written, and one of them conflicts with your own argument.

## A proposed spine for the article

1. **The PRD comes back.** "Write a big spec, then one-shot it" repeats the waterfall mistake. Agents make the documents cheap to write, but they don't make them cheap to read.
2. **Human attention is the bottleneck.** Reading and judging alignment is what doesn't scale, both before and after generation.
3. **Agentic leverage.** A definition, and why maximising it naively backfires.
4. **Errors compound.** Drift within one agent's trajectory, and the telephone game across sub-agents.
5. **Agentic agility.** The loop: a light prompt, research condensed into decisions, human input, the next iteration.
6. **Carving the solution space.** When to stop: once every remaining degree of freedom is one you don't care about.
7. **Where big specs do work,** and what improving models change and don't change.

## Critical assessment

**1. You risk attacking a strawman.** The main spec-driven tools aren't purely one-shot. Spec-kit's workflow is Constitution → Specify → Plan → Tasks, repeated, and Spec Kit's /clarify runs "sequential, coverage-based questioning" to cut rework. Böckeler (Thoughtworks) divides spec-driven development into three levels: spec-first, spec-anchored, and spec-as-source. Frame your target as a specific *interpretation*, as your notes already do, and name it precisely.

Your best evidence is HumanLayer, the team behind the popular Research→Plan→Implement workflow, which iterated its way to your position:

- Plans ran about 1,000 lines for features that produced about 1,000 lines of code. Reviewing a plan took roughly an hour and was nearly as much work as reviewing the code, so it gave little leverage.
- Their replacement is a roughly 200-line design document covering current state, desired end state, patterns, decisions, and open questions, which they claim offers about 5x the leverage of reviewing a 1,000-line plan. That is essentially your "condense to pain points" step.
- About 50% of the time, agents skipped the interactive steps and wrote full plans without asking questions.
- In another interview, Horthy said the difference between good and bad results was a single line: 'Work back and forth with me, starting with your open questions and outline before writing the plan.'

That is a practitioner reaching agentic agility empirically. Use it as your central case study.

**2. The "400 lines" figure is real but about something else.** It comes from the SmartBear/Cisco study of code review: reviewing 200-400 lines of code over 60 to 90 minutes should yield 70-90% defect discovery; beyond 400 lines, the ability to find defects diminishes. That's defect-finding in code, not reading prose specs. You can still use it as an analogy for an attention budget, but say so. "Or so I hear" will get picked apart.

**3. "Retroactive checking is infeasible" is your most vulnerable claim.** The counter-evidence is strong:

- HumanLayer reversed its advice to skip reading code after six months, and had to replace large parts of systems built that way.
- Addy Osmani's "comprehension debt" is the growing gap between how much code exists in your system and how much of it any human being genuinely understands.
- An O'Reilly piece makes your own point back at you: the organizational cost of writing specs thorough enough to substitute for review may well exceed the productivity gains from using AI to execute them. And you still haven't reviewed what was actually produced.

A defensible version: verification doesn't go away, it moves. Intent alignment moves earlier, into the decision loop. Conformance goes to machine checks (tests, types, invariants). Human review becomes targeted at the risky slices. Exhaustive reading of everything afterwards is what doesn't scale.

**4. Your leverage metric conflicts with your own argument.** You define leverage as human input tokens versus effective output tokens. But your whole case is that the cost is *reading*, not writing. A verbose agent scores high on that metric while burning your attention. It's also an easy metric to game: pad the output and leverage goes up. A better definition: **intended outcome delivered per unit of human attention, counting both writing and reading, net of rework.** That definition makes your conclusion follow directly: one-shot runs look cheap up front but get expensive in review and rework.

**5. Sharpen the error-compounding argument with a distinction.** The basic arithmetic: 95% per-step accuracy gives only ~60% success over 10 steps, and 36% over 20 steps. Reality is worse. A recent paper finds errors are positively correlated across steps, so failure grows faster than the independent-steps model predicts. The telephone game has a solid source too: Cognition's principles that you should share context because actions carry implicit decisions.

The critics will say that agents self-correct. Answer that the environment catches some errors (compiler, tests, type errors) and agents fix those well. Intent errors have no automated check except you. That is exactly why the human should be in the loop for decisions and ambiguities, and not for everything.

**6. "Agentic agility" has strong ancestry, so claim it.** It closely resembles Boehm's spiral model (risk-driven iterations, where you resolve the riskiest unknowns first) and Toyota's set-based concurrent engineering (keep a set of options open and narrow it by constraints, rather than committing to one design early). What's new is the economics: research and prototypes are now cheap enough to iterate on *decisions* rather than documents. Two open questions to address:

- **Stopping criterion.** Your phrase "regardless of further implementation details" is good. Make it explicit: stop when the remaining freedom lies inside your indifference set.
- **Who does the condensing.** Deciding which questions are worth your attention is itself an agent capability. The 50% question-skipping figure above shows it isn't reliable yet.

**7. A forward-looking point worth adding.** Agents' autonomous range keeps growing. METR's measure of task length agents can complete doubles every 131 days for models since 2023, under its updated 1.1 methodology. That lengthens the safe interval between checkpoints, but it doesn't remove the loop. An underspecified intent is not a capability gap, so a smarter agent just guesses faster.

## Visual 1: drift with and without checkpoints

This contrasts unchecked drift with bounded drift, and is drawn below.

> _Visual: [intent_drift_one_shot_vs_agility](visuals/intent_drift_one_shot_vs_agility.svg)_

> _Visual: [carving_solution_space_iterations](visuals/carving_solution_space_iterations.svg)_

The second visual also gives you a stopping rule: you're done once the dashed box sits inside the teal shape, and the remaining choices are ones you don't care about.

Two more visuals worth considering:
- **A leverage curve.** Human attention on the x-axis, delivered intent on the y-axis. One-shot shoots up early and then sags under review and rework. The agile loop climbs more steadily.
- **A "what the human sees" comparison.** Side-by-side document sizes: a 1,300-line spec versus a ~200-line decision summary. Marmelab's example fits here: a spec-kit run to display the current date in a time-tracking app produced 8 files and 1,300 lines of text.

Other references to cite, besides the sources linked above:
- **Peter Naur, "Programming as Theory Building" (1985):** intent lives in people's heads, and that is the root of comprehension debt.
- **Barry Boehm's spiral model:** the precedent for risk-driven iteration.
- **Toyota's set-based concurrent engineering:** the precedent for narrowing a set of options instead of committing early.

The biggest fix before drafting is the leverage definition. If its denominator becomes human attention rather than input tokens, the rest of the argument follows from it. When you're happy with the structure, I can draft the article as a doc. Want me to put this on a Whiteboard?

---

## You (2026-11-20 07:57)

I see also that I forgot to fill in that section on retro-active checking. What I meant to say is that much of the checking ought to be up-front, rather than in detail after the fact. And that we can use agents also to help us in the same kind of loop afterwards. Taking the large outputs and reduce them so that our human attention can cope, providing instructions that do the corrections. Agents can help us identify the risky areas so that we can scale our attention proportional to risk. And risks can come from both the established solution and the change.

Another note to add: We also see that learning suffers when employing agents. There is no substitute for humans to time, attention, deeply engaging with a problem. Not doing that unfortunately means having limited understanding of the problem. Again, trying to understand everything (reading every line of code) means foregoing agentic leverage. What we need instead is to understand the major structures of the problem and solution space. We can go more fuzzy on the details, of which there are many. Ideally, we leverage the pareto 20/80 principle. Understand enough of the 20% top-level structure that the 80% blank area can be somewhat safely speculated about.

---

## Claude (2026-11-20 07:58)

> _Searched the web: Anthropic study "How AI Impacts Skill Formation" coding comprehension results_

Both notes strengthen the piece. The first fixes the weakest claim from my last reply, and together they give the article one unifying idea: **the same loop runs on both sides of generation, and human attention is spent in proportion to risk.**

## Revised spine

1. **The PRD comes back:** big specs, then one-shot.
2. **Human attention is the scarce resource,** not tokens.
3. **Agentic leverage,** redefined as intended outcome per unit of human attention.
4. **Errors compound:** drift and the telephone game.
5. **Agentic agility before the build:** shape the solution space by working through decisions.
6. **Agentic agility after the build:** the same loop in reverse. Agents condense, rank by risk and propose corrections; the human steers.
7. **Understanding without reading everything:** structure over detail, the 80/20 principle, and what that means for learning.
8. **Where big specs do work.**

## On checking after the build (section 6)

"Most checking up front; agents help afterwards" now reads as a deliberate design rather than a dismissal of review. Four refinements:

**Condense into pointers, not paraphrases.** If an agent summarises a large diff and you only read the summary, you're trusting the same kind of system that made the mistake. Have the agent reduce the output to *pointers*: "these 3 places carry the risk; here are the exact lines, the test that covers each one, and what would break." You still read real evidence, just much less of it.

**Cross-check the agent's risk ranking with deterministic signals.** An agent that misunderstood the intent will also misjudge where the risk is. Anchor its ranking with signals that don't share its blind spots: test coverage of the changed lines, type and static-analysis results, change-impact graphs, and "hotspots" (code that changes often and is complex), as in Adam Tornhill's *Your Code as a Crime Scene*.

**Your two-sided view of risk is the most original part. Name it.** Risk comes from:
- **The existing system:** blast radius, how central the touched area is, how fragile it was already.
- **The change itself:** how novel it is, how many decisions the agent made implicitly, how far it strays from established patterns.

A small change in a critical, fragile area and a large change in a leaf module both deserve attention, for different reasons. A simple 2×2 grid (existing-system risk × change risk, with attention per cell) would make a strong visual.

**Corrections flow back as instructions, not edits.** This closes the loop: corrections become new constraints for the next iteration, so the after-phase feeds the before-phase. It's one loop, not two.

## On learning and the 80/20 principle (section 7)

The evidence supports you, and it supports your method specifically. In Anthropic's randomized trial, developers using AI coding assistance scored 17% lower on comprehension tests than those coding manually, and the productivity gains didn't reach statistical significance. But the decline wasn't universal: participants who showed stronger mastery used AI not just to produce code but to build comprehension, by asking follow-up questions, requesting explanations, or posing conceptual questions while coding. The study names these patterns, generation-then-comprehension, hybrid code-explanation, and conceptual inquiry, as the ones where skills kept developing.

That is your argument in empirical form. The agility loop, where you engage with decisions, ambiguities and trade-offs, is itself a high-engagement learning pattern. Pure delegation is what loses the learning. So agility protects understanding as well as alignment. One caveat: the study looked at mostly junior developers learning a new library. That's evidence for your claim, not proof of it for experienced engineers on their own systems.

Two cautions on the 80/20 framing:

1. **Risk isn't spread evenly across the details.** The dangerous parts of software often sit in small details: concurrency edge cases, error paths, security boundaries, data migrations. So the 20% you understand should be "the major structure *plus* the places where details carry weight." Finding those places is exactly what the risk-scanning agent from section 6 is for. Say so explicitly; it ties the two sections together.

2. **"Safely speculating about the blank area" requires a predictable codebase.** You can only infer unread code from the structure if the code follows that structure: consistent patterns, conventions and architectural rules. Agents tend to introduce small inconsistencies, which quietly breaks that assumption. A concrete takeaway for readers: **invest in consistency, and enforce it with machine checks, because consistency is what makes partial understanding safe.** Naur's "Programming as Theory Building" fits here: the 20% is the theory, and the rules are what keep the code faithful to it.

On the label: "Pareto" is fine as a heuristic, but present it as such, not as a measured ratio, or a pedantic reader will ask where the numbers come from.

Want me to draft the full article as a doc from this outline?

---

## You (2026-11-20 08:08)

Create a Claude Doc from your last answer.

---

## Claude (2026-11-20 08:08)

> _Created Claude Doc_

I've turned my last answer into a Claude Doc: [Agentic agility: revised outline and notes](https://claude.ai/artifact/2XyCQiz4dpj3MkEp8W45LG). You can edit it inline or leave comments, and I'll revise from there.
