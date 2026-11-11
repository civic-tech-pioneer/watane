Subsystem: dev
Effort: low
Depends on: none
Slice: S11 — The recommendations mod calls the reading list (user-sim rejection fix).

## Goal
When `serve` is refused because a local provider fails its tests, `next` (and the failing
problems' `fix`) send the agent to the provider: `watane-dev test --json --dir <absolute provider
folder>`, which shows the provider's failure, instead of `watane-dev test --json`, which passes in
the consumer and loops. And the template's agent guide shows the `fakes` and `deliver` forms that
now work, instead of saying they arrive in step 2.

## Acceptance
- AC1 The red test `dev/tests/serve.rs::regress_s11_provider_failure_next_reproduces_it`
  (commit bf85bf3) passes, unchanged.
- AC2 `serve` refused by the consumer's own test failure keeps `next` `watane-dev test --json`
  (existing tests stay green). The first failing provider decides `next`.
- AC3 `dev/template/AGENTS.md`: replace "Provider fakes and deliver steps arrive with links
  between mods (step 2)." with two short lines and one example of `fakes` (key
  `mods:<slug>/<iface>` without version → function → JSON answer; a call with no fake fails "no
  fake") and of a `deliver` step (`{"deliver": "<provider iface>.<fn>", "args": {...}}`), as dev
  TECHNICAL 8.1 says. Template tests stay green (the template still passes `test`).
- AC4 Update dev TECHNICAL 2.4 **serve** (the `next` rule) and 9.2 in one sentence each.
  Default `cargo test -p watane-dev`, clippy `--all-targets --all-features -D warnings`,
  `cargo fmt --check` pass; nothing else changes.

## Spec
dev `TECHNICAL.md` 2.4 **serve**, 8.1, 9.2, 9.3 step 1.

## Interface
none (dev only).

## Files
`dev/src/{cli,preview}.rs`, `dev/template/AGENTS.md`, `dev/TECHNICAL.md`.

## Out of scope
Anything else the user-sim noted (fixes for cycle errors, `check.providers[].path`, the
`/events` noise); `contract.rs`; `host/`, `pkg/`.
