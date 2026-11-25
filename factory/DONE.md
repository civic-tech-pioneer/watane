# Done

Sketches of merged slices, moved out of PLAN.md (deputy skill, section 6). Newest last.

- [~] **S119 (upkeep) S111's polish in host and dev, S118's, and pkg's reserved Windows names.** (in flight, cloud,
  builder codex, reviewer claude; rule 3: last two S118 claude, S117 codex; the upkeep lane, sketched 2026-11-25 by
  deputy-3 from Leftovers; Codex task https://chatgpt.com/local/6b067827-da14-83ed-ab43-79bfbda7704a, dispatched 05:20 UTC; DELIVERED 05:59 UTC as PR #35, final head 7264e38 (+640/-102, all four CI jobs green; item 4 was already on main from S112, a red test added); r1: Claude review, routine `trig_01QxsQGcSfzYy1Hjj5buRF2r`, fires 06:42 UTC.) Outcomes, each item's check: (1) S111 r1 P1.1: connect and TLS handshake each get their own
  `idle-s` in `host/src/agent/https_stream.rs`, with a test each; P1.2 to P1.5, P1.11, P1.14 (host tests, the
  reset-before-head message, one sentence each in host 19.4 for the accepted R2 and R5); (2) S111 r1 P1.6 to P1.10,
  P1.12, P1.13 in `scripted-responses` and dev TECHNICAL; (3) S118 r2 P2.1 and P2.2; (4) S110 R2: pkg's `grammar`
  refuses Windows reserved device names on every platform, red test first. Touches: `host/`, `dev/`, `pkg/`. No boundary
  moves (item 4 narrows what a package may name; adversary not run). Protects: *polish*, item 4 *guard*.
