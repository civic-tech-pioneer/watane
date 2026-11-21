# Agentic agility

*Agents can build faster than we can understand. Keep commitments small where the assumptions are unresolved.*

"Make the dashboard load faster."

That is the whole request. An agent finds the slow aggregate query and caches its result per team for ten minutes. The tests pass, the page is quick, and nothing in the request was ignored. *(This is an illustration, not a real incident.)*

The cache rests on two assumptions the request never spelled out.

The first concerns visibility. The old query ran once per viewer, with that viewer's permissions: a manager saw the salary column, an engineer on the same team did not. Sharing one result per team assumes that team membership is a safe boundary for sharing. The code said otherwise, and the agent did not look.

The second concerns freshness. The numbers used to be live. Now a change to those numbers can take ten minutes to appear. How much delay is acceptable is a product question, and nobody asked it.

One is a constraint the agent should have found, the other a question it should have asked. Both get expensive the same way: through everything built on them before anyone looks.

## Writing the spec is not agreeing on it

The version of spec-driven development that concerns me goes like this: write a large specification, let the agent implement it in one go, then find out how much agreement the document actually held. We have been here before, with the big requirements documents of the past. Agents can write and maintain the document and help me navigate it. Judging the decisions it contains still takes my attention.

That criticism needs a precise target. Spec-driven development also includes workflows with feedback and small tasks; [GitHub's introduction to Spec Kit](https://github.blog/ai-and-ml/generative-ai/spec-driven-development-with-ai-get-started-with-a-new-open-source-toolkit/) builds in checkpoints for reflection and refinement. A specification is a fine place to keep decisions reached that way. The trouble starts when approving the whole document becomes the way we agree. In her [evaluation of Kiro, Spec Kit and Tessl](https://martinfowler.com/articles/exploring-gen-ai/sdd-3-tools.html), Birgitta Böckeler found verbose, repetitive documents, heavy review overhead, and elaborate instructions that gave a false sense of control.

When agents produce more than I can review carefully, my judgment becomes the limiting resource.

The dashboard request was short. A long specification could have hidden the same assumption about sharing, or slipped in the same ten-minute lifetime, and keeping it up to date would not have told me whether I had looked at either.

The size of the document says little about the size of the commitment. A long reference can hold many decisions already reviewed. A short instruction can launch a lot of work on an open question. The expensive mistake is letting that work pile up before anyone examines the question.

## A wrong assumption gathers dependents

An open question gets expensive when the agent commits to an answer without checking it. Investigating, running a throwaway experiment or keeping the existing rule would all have counted as progress. Here the agent committed: the team is the sharing boundary. That premise then gathers dependents. The cache key is the team. The invalidation logic follows team-level events. The tests check that two teammates get the same cached totals. A sub-agent asked to "speed up the CSV export the same way" copies the pattern. Each step is consistent with the premise, and each depends on it.

![One unchecked assumption, four pieces of work built on it](03-sections/figures/fig1-dependents.svg)

When someone finally checks the premise against the permission rules, all four need another look. Not all of them will be wrong; the invalidation logic may survive. But each was built on the premise, and the more work rests on it, the more there is to check.

Delegation can widen the circle in two ways. A handoff can lose a decision: Cognition's [case against multi-agent setups](https://cognition.com/blog/dont-build-multi-agents) is that actions carry implicit decisions a sub-agent working from partial context may miss, and a study of more than 1,600 multi-agent traces lists misalignment between agents as one of its three main failure classes ([Cemri et al.](https://arxiv.org/abs/2503.13657)). Or the handoff passes the premise on faithfully, wrong as it was. This is no argument against delegation; on work that splits into independent parts, more agents can help ([Kim et al.](https://arxiv.org/abs/2512.08296)). The trouble is an unchecked premise that many parts share.

## Count all of my attention

My first definition of *agentic leverage* was a token ratio: output tokens of the deliverable divided by the input tokens I wrote. It captures the appeal, a little from me and a lot from the agent, but it rewards verbosity. Deleting unnecessary code can be valuable and score almost nothing. A short prompt can leave me with hours of reading and correcting. What I want to increase is something else.

**Agentic leverage is the useful, accepted work I get per unit of my attention: explaining, answering, reading, reviewing and correcting.**

It has to be counted, because the feeling of speed is unreliable. In [METR's early-2025 trial](https://metr.org/blog/2025-07-10-early-2025-ai-experienced-os-dev-study/), experienced developers took 19% longer with AI, yet believed it had made them 20% faster. The tools have changed since, and METR's [2026 follow-up](https://metr.org/blog/2026-02-24-uplift-update/) explains why later measurements are harder.

For the dashboard, checking the permission rules and asking how stale the numbers may be costs attention. The comparison that matters is that cost against the work it saves later.

## Make the next decision visible

A useful plan makes its important decisions visible. I want a loop that brings those decisions to the surface a few at a time.

I start with enough direction for the agent to investigate: why something needs to happen, what should happen or how, whichever I know. The agent reads the code, runs a quick experiment if one is cheap, and brings back what it found and the few decisions that would change the work. For the dashboard, with invented findings:

> **Found:** the query applies each viewer's permissions. Any speed-up keeps that rule; it is not up for decision.
> **Decision:** how fresh must the numbers be, and is a cache worth its complexity?
> **Why it matters:** the numbers are live today; a cache makes them older.
> **Options:** (a) cache per viewer, with a short lifetime; (b) share a result between viewers whose permitted data is identical, and recheck access on every hit; (c) no cache: the slow part looks like one join, which may be fixable.
> **Recommendation:** try (c) as a throwaway experiment and measure. Choose between (a) and (b) only if (c) falls short.
> **Unblocks:** the experiment now, the cache design later.

Suppose I answer: at most a minute old, preserve the existing access rules, try (c) first. Answering may take some reading. But I am answering the premise the work will rest on, not approving a document in which that premise is one line among hundreds. When the question is whether we agree on intent, the unit of review is a decision, not a document, even when the decisions live in one.

The answers go into a short record of what is agreed, what is assumed and what is still open. In simulated conversations where requirements emerged across turns, models often held on to early wrong assumptions ([Laban et al.](https://arxiv.org/abs/2505.06120)). Superseded decisions are struck out, and when the history starts to mislead, the next step starts fresh from the record. This is a guard I propose, not a proven one; a record can lose an assumption too.

When do I stop deciding and let the agent run? When the remaining choices fall within what we agreed, the important questions are answered or contained, and there is a credible way to check the result. Then the agent takes the next bounded step, and the questions reopen when any of that changes. The experiment with (c) is such a step: a big question is still open, but contained.

![The loop, and what each pass does to the choices for the next step](03-sections/figures/fig2-loop.svg)

Risk-driven iteration has precedents: [Boehm's spiral model](https://ieeexplore.ieee.org/document/59) organized development around risk, and DORA recommends [small batches](https://dora.dev/capabilities/working-in-small-batches/) for faster feedback in AI-assisted work. Agents lower the cost of the investigating and prototyping this takes.

## Review the change and what it touches

The same loop applies once there is something to inspect. Before the build, the evidence came from research and experiments. After it, the evidence is behavior, diffs and tests.

Suppose the loop was skipped and the cache from the opening arrives as a finished change. I do not want a summary that says permissions are respected. I want pointers I can check: the code, the assertion or observation, and why it addresses the risk.

Two questions need different evidence. Does a cache hit still respect the viewer's permissions, including after a manager moves to an engineering role? The existing rules tell the agent what to preserve; the evidence must show that it does. Accepting older numbers does not relax those access rules. How stale may the numbers be? The code provides live results but gives no agreed tolerance for delay. The agent should put that choice to me, then show that reuse and expiry follow my answer.

Risk comes from the existing system, the change and how they interact. The cache may be a small diff. The authorization path it affects may not have changed at all. Both belong in the review.

![Only the cache code changed, but the review follows its interaction with unchanged access rules.](03-sections/figures/05-attention-map.svg)

*The diff changes the cache. Review follows its effect on access into unchanged code.*

A test that shows two requests return the same result may just encode the sharing assumption. A useful test runs as two viewers with different permissions and checks what each may see. I need to read what is asserted, and why.

Risk information can help focus review: in a [controlled experiment](https://rebels.cs.uwaterloo.ca/papers/emse2024_yu.pdf), historical risk analytics improved defect detection; the difference in review speed was not statistically significant. That study tested historical analytics, not an agent's risk ranking. I would challenge the ranking with coverage information, static-analysis findings and traced dependencies, and inspect some areas it did not flag. These signals have limits: coverage cannot tell me whether an assertion tests the right behavior.

Corrections go back as instructions: fix the implementation, strengthen a check, drop an assumption, or reopen a product decision. Changing the requirement just to fit the code would hide the disagreement.

All of this assumes I can judge what the agent shows me. Its map of the risks does not give me the mental model I need to question it.

## Keep enough understanding to challenge the next answer

Delegation can remove the very activity through which understanding forms. In a randomized experiment, 52 mostly junior developers learned an unfamiliar Python library with or without an AI assistant. On a quiz straight afterwards, the AI group averaged 50% against 67%, with the largest gap on debugging ([Shen and Tamkin](https://arxiv.org/abs/2601.20245)). Those in the AI group who asked for explanations did better than those who only asked for code, though that was observed, not randomized, and a short task is not years on a familiar system.

Reading every line of every change would give away the leverage, though some changes deserve it. As a default, I aim for the part that lets me reason about the rest: the main structures and their contracts, who owns which state, where the trust boundaries are, how things fail, and the few details that carry weight, like a cache key that decides who sees what. Consistent patterns and machine-checked contracts make reasoning about unread code less of a gamble. They do not guarantee correctness: the same mistake can recur consistently.

One practice I want to make a habit is to predict before reading. Before opening the agent's explanation, write down what you expect. A manager moves to an engineering role. Under the existing access rules, what may she see on her next refresh? Then check the prediction against the code or a test. A disagreement calls for investigation: my understanding, the implementation or the test may be wrong.

Peter Naur argued in 1985 that programming builds a theory of how a program addresses a real-world problem, held by its programmers and only partly captured in documentation ([Programming as Theory Building](https://gwern.net/doc/cs/algorithm/1985-naur.pdf)). The loop gives me natural moments to keep that theory current, since every decision asks me to engage. But approving a recommendation without thinking is as easy as approving a document.

## Match the commitment to the task

How much back-and-forth a task needs depends on the task. A familiar change with good checks may need one pass: execute, then check. An uncertain idea may be ready for a small experiment and far from ready for a large implementation.

[Dex Horthy describes this proportional approach](https://github.com/humanlayer/advanced-context-engineering-for-coding-agents/blob/main/wsff.md) from his own practice: straightforward tasks get one shot or light feedback, larger work gets more design and review as it goes.

Settle an open choice that is costly to undo before building on it, and delegate a larger bounded step only once the bounds are agreed and the result can be evaluated.

Long autonomous runs fit this too: [Anthropic describes](https://www.anthropic.com/research/harness-design-long-running-apps) agents that plan, build and evaluate for hours. How long a run takes says little about how much it commits to unchecked. Investigation can find the constraints that already exist, but not a preference nobody has stated; that is still a question for me.

I cannot prove this beats other ways of working; it is a proposal built on the evidence above.

Back to the dashboard. One look at the existing permission rules, and one question about how stale the numbers may be, before anything was built on either. That is the leverage I am after: more useful work from my attention, and enough understanding left to steer the next change.

Before the next commitment, ask: which assumptions does it rest on, and what evidence would make them worth accepting?
