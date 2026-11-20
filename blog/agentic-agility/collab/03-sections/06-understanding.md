<!-- Section 6. Author: Claude. Target 360 words. Covers C6. No standalone figure; follows section 5's transition. -->

## Keep enough understanding to challenge the next answer

Delegation can remove the very activity through which understanding forms. There
is evidence for that worry, within limits. In a randomised experiment, 52 mostly
junior developers learned an unfamiliar Python library, with or without an AI
assistant. On a quiz straight afterwards, the AI group averaged 50% against 67%
without it, and the gap was largest on debugging
([Shen and Tamkin](https://arxiv.org/abs/2601.20245)). Within the AI group, those
who asked conceptual questions and asked for explanations did better than those
who only asked for code. That second finding was observed, not randomised, and the
study tested a short task, not years of work on a familiar system. It supports the
worry; it does not settle it.

Reading every line of every change would give away the leverage. Some changes
deserve it. As a default, I aim for the part that lets me reason about the rest:
the main structures and their contracts, who owns which state, where the trust
boundaries are, how things fail, and the few details that carry weight, such as
a cache key that decides who sees what. Call it 20/80 if you like; it is a
heuristic, not a measurement. Consistent patterns and contracts enforced by
checks make reasoning about unread code less of a gamble. They do not make it
safe, because consistency can also repeat a mistake everywhere.

One practice I want to make a habit is to predict before reading. Before opening
the agent's explanation, write down what you expect. A manager moves to an
engineering role. Under the agreed rule, permission changes show at once, so what
does she see on her next refresh? Then check the prediction against the code or a
test. If they disagree, one of them is wrong, and finding out which is the point:
either my model of the system needs fixing, or the code breaks the rule we agreed.

Peter Naur argued in 1985 that a program's real design lives in the heads of the
people who build it, as a theory of how the program meets the problem, which
documentation only partly captures
([Programming as Theory Building](https://gwern.net/doc/cs/algorithm/1985-naur.pdf)).
The loop gives me natural places to keep that theory current, since every
decision asks me to engage. But approving a recommendation without thinking is as
easy as approving a document. The loop makes room for learning; the learning is
still mine to do.
