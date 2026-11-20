# Claude × Codex co-writing

The user asked Claude and Codex to co-write the article, taking turns
proposing and critiquing / counter-proposing, in four phases:

1. `01-argument.md`: the argumentative structure that holds up against all evidence
2. `02-outline.md`: the outline that best lays out that argument
3. `03-sections/`: each section written, with its supporting visual; then `draft.md`
4. `04-language.md`: unsloppifying the language of `draft.md`

Rules of the exchange:
- Each turn appends a `## Turn N: <Claude|Codex>` heading to the phase file. Never
  rewrite the other's turn.
- A turn either agrees (says `AGREED` on what it accepts) or counter-proposes with
  reasons and evidence. A phase closes when both have written `PHASE AGREED`, and
  the last turn ends with the agreed text under `## Agreed`.
- Cite sources by URL. New research is welcome; add it to the turn.
- Source material: `../conversation.md`, `../research.md` (Claude), `../codex-conversation.md`,
  `../codex-research.md` (Codex), `../visuals/`.

## Result (2026-11-20)

All four phases closed with PHASE AGREED from both. The article is `draft.md`
(about 2,650 words, three figures in `03-sections/figures/`). History:
`draft-v1.md` (assembled sections) → `draft-v2-claude.md` (Claude's language pass)
→ `draft.md` (with Codex's L01–L24 and two counter-edits). The optional paragraph
quoting watane's task-brief rule is kept in the section source
(`03-sections/04-decisions.md`) and out of `draft.md` until the user decides.
