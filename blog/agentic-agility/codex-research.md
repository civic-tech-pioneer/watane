# Agentic agility research and collaboration notes

Added 2026-11-20 from the Codex conversation. The existing research.md and conversation.md contain the separate Claude discussion and have been preserved.

## Collaboration brief

The user wants to develop a blog article and has asked to structure, research, and critically assess the ideas before drafting. The user has now asked Claude and Codex to collaborate on writing the document. This file is the current Codex research handoff; the substantive exchange is preserved in codex-conversation.md.

### Current working thesis

Agentic agility combines broad delegation with selective human depth. Agents help expose consequential decisions and assess results; humans invest attention in the structures, assumptions, and risks needed to direct the work and preserve their ability to keep directing it.

Agents make producing specifications and implementations cheaper. They do not automatically make establishing shared understanding cheaper.

### User intent and accepted clarifications

- Criticize the interpretation of spec-driven development that favors large upfront specifications and a large one-shot implementation. Do not portray all SDD as requiring this approach.
- Agentic leverage originally means effective deliverable output tokens relative to human input tokens, excluding reasoning, tool calls, and delegation overhead from the deliverable. Preserve this origin even if the article adopts a broader measure.
- Much checking should happen upfront. Afterwards, agents should help reduce large outputs into manageable, risk-focused evidence that humans can assess and use to instruct corrections.
- Risk comes from the established solution, the change, and their interaction.
- Humans need time and active engagement to develop understanding. Reading every line sacrifices leverage; the goal is enough structural understanding, plus depth where details carry consequences.
- The 20/80 idea is a heuristic for selective understanding, not an empirical ratio.
- The original request was for research and structure before writing the article. The later request anticipates collaboration with Claude on writing; coordinate the actual draft rather than creating competing versions.

### Decisions still worth resolving

- Whether to call the token ratio token leverage and reserve agentic leverage for useful accepted progress per unit of total human attention.
- How prescriptive the method should be: a decision-making principle, a lightweight workflow, or a named development process.
- Which worked example can demonstrate an ambiguity, the decisions surfaced, the human response, and the implementation or review work changed by that response.
- How to express the stopping condition: several acceptable solutions within explicit constraints, with triggers for reopening decisions, rather than a guarantee that every remaining detail is immaterial.
- How to illustrate risk and attention without fabricated empirical curves or numerical scores.
- How to preserve learning without requiring exhaustive comprehension or a human checkpoint for every routine choice.

## Proposed argument structure

1. The apparent shortcut: agents generate extensive plans and implementations from little human input.
2. The remaining cost: humans must understand what they are agreeing to.
3. Error propagation: an unresolved interpretation becomes a planning assumption and accumulates dependent work.
4. Agentic agility before execution: investigate and experiment, expose consequential uncertainties, make decisions, and revise the shared understanding.
5. Agentic agility during and after execution: collect evidence, identify risks, direct human inspection, and translate corrections into the next iteration.
6. Selective depth: learn the structures, contracts, assumptions, and consequential details needed to supervise the work.
7. Conditional autonomy: permit substantial autonomous work where boundaries and verification are effective.
8. Limits and evaluation: distinguish a reasoned proposal from a workflow whose overall superiority has been experimentally demonstrated.

## Critical distinctions

### Specifications and shared understanding

Document maintenance and human agreement are different costs. A long reference document can be useful; approving it as a whole may still be a poor mechanism for establishing alignment. A useful review experience exposes consequential decisions, evidence, alternatives, and unresolved assumptions.

### Amplification and value

A deliverable-token to human-instruction-token ratio measures amplification. It rewards verbosity and misses useful deletion, comprehension, and rework. A practical objective is useful accepted progress per unit of human attention, subject to compute cost and failure risk. Include explaining, answering questions, reading, reviewing, and correction in the attention denominator.

### Error propagation and exponential claims

The clearest mechanism is dependency: an early interpretation shapes a plan, then implementation and delegated tasks depend on it. Correction affects dependent work. This does not require assuming independent per-step failures, a fixed error probability, or universally exponential error growth.

Distinguish distortion during handoffs from faithful propagation of an interpretation that was already wrong. Give delegated work access to agreed intent, constraints, and unresolved questions.

### Iteration and context discipline

More conversation is not automatically better alignment. Early assumptions can persist across turns. Explicitly distinguish agreed decisions, working assumptions, and unresolved questions; invalidate superseded decisions; consolidate current requirements; use fresh context when accumulated history is misleading.

### Alignment and verification

Upfront work clarifies intended behavior, constraints, architecture, and acceptance examples. Implementation evidence establishes how the result behaves. Usage establishes whether it solves the actual need. Earlier alignment can reduce rework but cannot prove correctness of unwritten code.

The same loop can operate before and after generation. A human-facing assessment should point to evidence rather than merely provide a reassuring paraphrase.

### System risk and change risk

Review the change and the assumptions and dependencies it affects, including unchanged code. A small caching change can violate an existing authorization boundary if identity is absent from the cache key.

Potential harm, uncertainty, weak evidence, unfamiliar behavior, and difficult recovery all justify more attention. Agent risk estimates are hypotheses, not guarantees. Deterministic signals, independent evidence, and occasional inspection outside flagged areas can expose blind spots.

### Learning and engagement

Delegation can remove the cognitive activity through which skills would otherwise form. Time permits engagement but does not ensure it. Passive reading of an agent explanation can leave a fragile mental model.

Candidate learning activities: predict how a failure propagates; explain why an alternative was rejected; identify an assumption that makes an optimization safe; walk through one representative execution path; test the prediction against behavior.

### Selective depth and Pareto

Understand goals, constraints, major components and contracts, state ownership, trust boundaries, important failure modes, recovery, and the limits of personal understanding. Delegate details within credible boundaries and checks.

Important knowledge is not always top-level knowledge: transaction isolation, cancellation, authorization, and migration details can determine whether the architecture works. Prefer conditional reasoning grounded in explicit assumptions over safely speculating about unread details.

## Decision summary format

For a consequential question, expose:

- Decision: what needs resolving.
- Consequence: why a wrong choice matters.
- Alternatives: materially different choices and tradeoffs.
- Recommendation: supporting evidence and uncertainty.
- Boundary: what can proceed after resolution.

Keep links to underlying evidence and preserve meaningful dissent or contrary evidence. An iteration can produce a prototype, failing test, user observation, or implementation slice; it need not produce more planning prose.

Proposed heuristic: seek human input when resolving an uncertainty is likely to prevent more consequential rework than the interruption costs. This is a design principle, not a validated numerical formula.

## Visual materials

- visuals/codex-error-dependencies.mmd: the conceptual chain from ambiguity to dependent work.
- visuals/codex-agility-loop.mmd: the proposed alignment and execution loop.
- Attention map concept: show established components, overlay the change and affected dependencies, and mark where uncertainty or consequences warrant deeper human engagement.
- Acceptable solution paths concept: successive decisions leave several acceptable implementations, rather than implying a single fully determined solution.
- The existing Claude SVGs remain separate draft illustrations. Neither conversation establishes measured drift curves, a universal optimal checkpoint interval, or a literal 20/80 allocation.

## Possible evaluation

Compare workflows on matched tasks with explicit acceptance criteria. Measure total human attention, accepted functionality, correction and rework, defects over a defined observation period, and compute cost. Include planning and review time; output volume alone is not success.

## Annotated source register

These are relevant sources located or consulted during the Codex research, not a complete raw search log. Primary research, vendor engineering reports, practitioner opinion, and terminology references are labeled separately. Dates and quantitative results below retain their original scope. Sources present in both the Claude and Codex records are kept in each record for provenance.

### 1 Understanding Spec-Driven-Development: Kiro, spec-kit, and Tessl

- Author or organization: Birgitta Böckeler
- Publication or revision: 2025-10-15
- Type: Practitioner evaluation
- Source: [Understanding Spec-Driven-Development: Kiro, spec-kit, and Tessl](https://martinfowler.com/articles/exploring-gen-ai/sdd-3-tools.html)
- Relevance: Closest match to the critique: verbosity, review burden, false sense of control, and distinctions among spec-first, spec-anchored, and spec-as-source.
- Limits: An evaluation of particular tools at a particular time, not a controlled demonstration that SDD is inferior.

### 2 Spec-driven development with AI: Get started with a new open source toolkit

- Author or organization: GitHub
- Type: Primary description of the workflow
- Source: [Spec-driven development with AI: Get started with a new open source toolkit](https://github.blog/ai-and-ml/generative-ai/spec-driven-development-with-ai-get-started-with-a-new-open-source-toolkit/)
- Relevance: Counterpoint: explicitly calls for human critique and refinement at checkpoints. Do not equate SDD as a whole with one-shot implementation.
- Limits: Vendor workflow guidance, not an effectiveness trial.

### 3 How to write a good spec for AI agents

- Author or organization: Addy Osmani
- Publication or revision: 2026-01-13
- Type: Practitioner guidance
- Source: [How to write a good spec for AI agents](https://addyosmani.com/blog/good-spec/)
- Relevance: Concise initial direction, modular context, explicit boundaries, and iteration; closely related prior writing.
- Limits: Recommendations and experience rather than a controlled workflow comparison.

### 4 What is Code Review?

- Author or organization: SmartBear
- Type: Vendor account of code-review research
- Source: [What is Code Review?](https://smartbear.com/learn/code-review/what-is-code-review/)
- Relevance: Source for the 200–400 lines guidance, which concerns code review, not a universal prose-reading limit.
- Limits: Do not generalize the numerical recommendation to human comprehension of specifications.

### 5 LLMs Get Lost In Multi-Turn Conversation

- Author or organization: Philippe Laban, Hiroaki Hayashi, Yingbo Zhou, Jennifer Neville
- Publication or revision: 2025-05
- Type: Empirical research
- Source: [LLMs Get Lost In Multi-Turn Conversation](https://arxiv.org/abs/2505.06120)
- Relevance: Challenge to naive iteration: early assumptions and prior responses can anchor later behavior.
- Limits: Simulated conversations with requirements distributed across turns, not a direct trial of the proposed agentic-agility workflow.

### 6 LLMs Get Lost In Multi-Turn Conversation — practical recommendations

- Author or organization: Laban et al.
- Type: Original paper section
- Source: [LLMs Get Lost In Multi-Turn Conversation — practical recommendations](https://arxiv.org/html/2505.06120v1#S7.SS4)
- Relevance: Consolidate requirements before retrying; fresh conversations can help when a conversation has gone off course.
- Limits: Mitigations, not guarantees of reliability.

### 7 Why Do Multi-Agent LLM Systems Fail?

- Author or organization: Mert Cemri et al.
- Publication or revision: 2025; revised 2025-10-26
- Type: Empirical research
- Source: [Why Do Multi-Agent LLM Systems Fail?](https://arxiv.org/abs/2503.13657)
- Relevance: More than 1,600 traces across seven frameworks; taxonomy includes system design, inter-agent misalignment, and verification failures.
- Limits: Does not establish that delegation always worsens alignment. The original version used a smaller dataset; cite the revised version consistently.

### 8 Towards a Science of Scaling Agent Systems

- Author or organization: Yubin Kim et al.
- Publication or revision: 2025-12
- Type: Empirical research
- Source: [Towards a Science of Scaling Agent Systems](https://arxiv.org/abs/2512.08296)
- Relevance: Task structure and coordination architecture determine when multi-agent work helps; sequential dependencies and overhead matter.
- Limits: Benchmark-specific effects, not universal error-amplification constants for software teams.

### 9 Towards a science of scaling agent systems: When and why agent systems work

- Author or organization: Google Research
- Publication or revision: 2026-01-28
- Type: Authors' research explanation
- Source: [Towards a science of scaling agent systems: When and why agent systems work](https://research.google/blog/towards-a-science-of-scaling-agent-systems-when-and-why-agent-systems-work/)
- Relevance: Accessible explanation of the scaling paper, including architecture-dependent error propagation.
- Limits: Keep quantitative claims tied to the experimental configurations.

### 10 Effective context engineering for AI agents

- Author or organization: Anthropic
- Publication or revision: 2025-09-29
- Type: Engineering guidance
- Source: [Effective context engineering for AI agents](https://www.anthropic.com/research/effective-context-engineering-for-ai-agents)
- Relevance: Select a small amount of highly informative context; supports information density rather than brevity alone.
- Limits: Context efficiency and human understanding are related but distinct.

### 11 How we built our multi-agent research system

- Author or organization: Anthropic
- Publication or revision: 2025-06-13
- Type: Vendor engineering report
- Source: [How we built our multi-agent research system](https://www.anthropic.com/research/multi-agent-research-system)
- Relevance: Benefits of parallel research and costs of coordination; useful counterpoint to a blanket criticism of delegation.
- Limits: Reported 4× agent and 15× multi-agent token use are both relative to chats, not a 15× multi-agent-to-single-agent comparison.

### 12 Effective harnesses for long-running agents

- Author or organization: Anthropic
- Publication or revision: 2025-11-26
- Type: Vendor engineering report
- Source: [Effective harnesses for long-running agents](https://www.anthropic.com/research/effective-harnesses-for-long-running-agents)
- Relevance: Incremental work, persistent progress artifacts, and structured handoffs across sessions.
- Limits: Does not prove the reliability of all autonomous development or replace human alignment.

### 13 Harness design for long-running application development

- Author or organization: Prithvi Rajasekaran / Anthropic
- Publication or revision: 2026-03-24
- Type: Vendor engineering report
- Source: [Harness design for long-running application development](https://www.anthropic.com/research/harness-design-long-running-apps)
- Relevance: Counterpoint to frequent mandatory human interruptions: planners, generators, and evaluators can sustain multi-hour runs on constrained tasks.
- Limits: Not an independent controlled comparison of production workflows.

### 14 A spiral model of software development and enhancement

- Author or organization: Barry W. Boehm
- Publication or revision: 1988
- Type: Foundational process paper
- Source: [A spiral model of software development and enhancement](https://ieeexplore.ieee.org/document/59)
- Relevance: Historical predecessor for organizing development around resolving consequential risks.
- Limits: Use as intellectual lineage rather than evidence that the agent workflow is already validated.

### 15 Principles behind the Agile Manifesto

- Author or organization: Agile Manifesto authors
- Publication or revision: 2001
- Type: Primary principles
- Source: [Principles behind the Agile Manifesto](https://agilemanifesto.org/principles.html)
- Relevance: Frequent delivery, changing requirements, collaboration, and working software provide historical context.
- Limits: Principles are not a trial of agentic development.

### 16 We are Changing our Developer Productivity Experiment Design

- Author or organization: METR
- Publication or revision: 2026-02-24
- Type: Research update
- Source: [We are Changing our Developer Productivity Experiment Design](https://metr.org/blog/2026-02-24-uplift-update/)
- Relevance: Caution about measuring productivity and repeating the early-2025 slowdown finding as if it described current tools.
- Limits: Later estimates suffer selection and timing-measurement problems. Not a direct test of agentic agility.

### 17 How AI Impacts Skill Formation

- Author or organization: Judy Hanwen Shen and Alex Tamkin
- Publication or revision: 2026-01; revised 2026-02-01
- Type: Randomized experiment
- Source: [How AI Impacts Skill Formation](https://arxiv.org/abs/2601.20245)
- Relevance: Direct evidence on learning an unfamiliar programming library; cognitive engagement matters alongside delegation.
- Limits: Short task and immediate assessment, mostly junior participants. Does not establish long-term deskilling; interaction styles were observed, not randomly assigned.

### 18 How AI assistance impacts the formation of coding skills

- Author or organization: Anthropic
- Publication or revision: 2026-01-29
- Type: Authors' study report
- Source: [How AI assistance impacts the formation of coding skills](https://www.anthropic.com/research/AI-assistance-coding-skills)
- Relevance: 52 mostly junior engineers; AI group averaged 50% versus 67% without AI on immediate assessment; largest gap in debugging; task-time difference not statistically significant.
- Limits: The score difference is 17 percentage points. Do not misstate it as a 17% relative reduction or generalize it to all agent use.

### 19 Studying the Impact of Risk Assessment Analytics on Risk Awareness and Code Review Performance

- Author or organization: Xueyao Yu, Filipe R. Cogo, Shane McIntosh, Michael W. Godfrey
- Publication or revision: 2024
- Type: Controlled experiment
- Source: [Studying the Impact of Risk Assessment Analytics on Risk Awareness and Code Review Performance](https://rebels.cs.uwaterloo.ca/papers/emse2024_yu.pdf)
- Relevance: 48 participants; historical risk information improved risk awareness and review effectiveness.
- Limits: No statistically significant speed improvement. Evaluated a risk analytics tool, not an LLM agent.

### 20 The Impact of Generative AI on Critical Thinking: Self-Reported Reductions in Cognitive Effort and Confidence Effects From a Survey of Knowledge Workers

- Author or organization: Hao-Ping Lee et al.
- Publication or revision: 2025-04
- Type: Survey
- Source: [The Impact of Generative AI on Critical Thinking: Self-Reported Reductions in Cognitive Effort and Confidence Effects From a Survey of Knowledge Workers](https://www.microsoft.com/en-us/research/publication/the-impact-of-generative-ai-on-critical-thinking-self-reported-reductions-in-cognitive-effort-and-confidence-effects-from-a-survey-of-knowledge-workers/)
- Relevance: 319 knowledge workers, 936 examples; greater confidence in AI associated with less reported critical thinking.
- Limits: Self-report and association, not causal evidence of lost critical-thinking ability.

### 21 Agentic Agility newsletters

- Author or organization: Martin Hinshelwood / naked Agility
- Type: Terminology reference
- Source: [Agentic Agility newsletters](https://nkdagility.com/resources/newsletters/)
- Relevance: The term agentic agility already has published usage, including a May 2025 newsletter.
- Limits: Establishes prior use of the phrase, not equivalence with the proposed method.

## Claims to avoid overstating

- The 400-line recommendation is about code review, not a general human prose-reading threshold.
- The skill-formation study's reported means differ by 17 percentage points. Its observed interaction styles do not prove that this entire proposed workflow preserves learning.
- The risk-analytics experiment supports improved awareness and defect detection, not a demonstrated speed gain or agent risk-ranking reliability.
- Benchmark error amplification and token multipliers are specific to the measured tasks, architectures, and denominators.
- Early-2025 productivity findings should not be presented as current universal effects; METR's later update explicitly discusses measurement limitations.
- No source reviewed here directly establishes that agentic agility, as a complete proposed method, outperforms other development workflows.
- Existing use of agentic agility means this article should define its specific interpretation without claiming to have coined the phrase.

