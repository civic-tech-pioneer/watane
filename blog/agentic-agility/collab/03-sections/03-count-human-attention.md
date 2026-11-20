## Count all the human attention

My first definition of *agentic leverage* was a token ratio: effective output tokens divided by the human input tokens that prompted them. Effective output meant the deliverable, excluding reasoning, tool calls, and the instructions used to start other agents.

That captures the appeal. A small contribution from me can produce a substantial result. But the ratio rewards verbosity. Removing unnecessary code might be valuable while producing almost no output. A short prompt can also leave me with hours of reading and correction.

I would keep that ratio as *token leverage*: a measure of amplification.

**Agentic leverage is the useful, accepted work I get per unit of my attention: explaining, answering, reading, reviewing, and correcting.**

| Token leverage accounts for | The human attention account also includes |
| --- | --- |
| Instructions I provide | Explaining context and resolving questions |
| Deliverable tokens produced | Reading plans, inspecting evidence, and judging results |
| Output relative to input | Correcting mistakes and reconsidering dependent work |

Quality, compute cost, and risk still matter; this is a way to evaluate a workflow, rather than a universal productivity score.

In [METR's early-2025 trial](https://metr.org/blog/2025-07-10-early-2025-ai-experienced-os-dev-study/), developers took 19% longer with AI but believed it had made them 20% faster; those historical results concern that setting, while [METR's 2026 follow-up](https://metr.org/blog/2026-02-24-uplift-update/) reports selection and measurement problems that complicate later estimates.

Felt speed is insufficient. For the dashboard, investigating the visibility constraint and asking how stale the numbers may be could save later review and correction. That input costs attention too. The useful comparison includes both its cost and the dependent work it changes.
