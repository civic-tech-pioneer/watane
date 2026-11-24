# log: rules for the builder

`log` gives watane its identity and its signed, hash-chained channels. It
depends on no watane subsystem, so you can build all of it now.

## Read in this order

1. `VISION.md`: why, and what `log` must never become.
2. `FUNCTIONAL.md`: the requirements `LOG-F-n` and the scenarios `S1`–`S28`.
3. `TECHNICAL.md`: the API, every byte format, the build plan (section 9).
4. Only if you need context: `../spec/` (authoritative for the whole) and
   `../colos/crates/colos-log` (a reference prototype; never copy blindly).

## Commands

`cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`,
`cargo fmt --check`.

## Rules that matter most here

- Build stage by stage (TECHNICAL.md section 9). Write the tests of a stage
  first, from its scenarios, then the code.
- An expected value comes from the oracle of section 8.1 (golden vectors,
  hand-built bytes, `ed25519-compact`), never from `watane_log` itself.
- Every byte format in TECHNICAL.md section 4 is fixed. If a golden vector
  fails, suspect the code first; change the vector only after checking it
  against the oracle of section 8.1.
- `Entry` has no public constructor except `decode` and `append`. Add no
  public `sign`, no setter, no `unsafe` in `watane-log`, no `legacy_compatibility`.
- `windows-fs/` is the shared Windows platform crate for log, net, dist, host
  and app. Only its FFI to the Windows security and file APIs may use
  `unsafe` (spec/TECHNICAL 2).
- No network code, and no knowledge of what a body means. Never weaken a check to make a test pass. Fix a real defect in the spec
  yourself and record it in "Questions for spec/" (TECHNICAL.md section 12).
- Write in plain English: short sentences, active voice.

## Reading the specs

The specs were drafted from earlier experiments and are not validated.
Their detail exists to surface integration problems early, not to be obeyed
word for word. Follow their intent, and do not ask whether to.

- Priority: `../spec/` vision, functional, technical; then this folder's
  vision, functional, technical. The whole wins over this subsystem.
- When a spec is wrong, unclear or in the way, change it yourself: edit the
  document that owns the fact (any folder's spec, never another folder's
  code), fix each document that cites it, and keep it true to the documents
  above it.
- Ask only to go against a vision document or to weaken a security check.

## When blocked

- `log` needs no sibling. Stages L1 to L4 need nothing else.
- Stage L5 needs `dist` and `host`. If they do not exist yet, stop after L4:
  `log` is done when its own tests pass. Do not write a fake `dist`.
- If a sibling asks for an item that section 10 does not list, add it only
  if it needs no new format or mechanism. Else record it in section 12.
