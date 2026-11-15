# node: rules for the builder of `watane-node`

## Read first, in this order

1. `../spec/VISION.md`, `../spec/FUNCTIONAL.md`, `../spec/TECHNICAL.md`,
   then `../spec/PEERS.md` (stage D1 is step 8).
2. `VISION.md`, `FUNCTIONAL.md`, `TECHNICAL.md` here. TECHNICAL.md is the contract.
3. `../net/TECHNICAL.md` 3 (the `Holdings` trait, `Net`) and
   `../dist/TECHNICAL.md` 3 and 5.12 (the holder methods, the keyless open).

## Commands

`cargo build && cargo test` (loopback only, no internet);
`cargo clippy --all-targets -- -D warnings && cargo fmt --check`.

## Rules that matter most here

1. `node` decides nothing that `dist`, `log` or `net` decide. It calls them
   in the order TECHNICAL.md 5 gives, and adds no check, cache or socket.
2. No holder method (TECHNICAL.md 4) calls `net` or reads from a web host.
   `told` is called after dist returned, never inside a holder method.
3. `node` never signs on its own: own statements come only from the app,
   through dist's `Confirm`.
4. A node home holds no identity: open it with `Node::open_keyless` only.
5. A test computes its expectation without `watane_node`. No test reaches
   the internet (TECHNICAL.md 10).
6. Build the stages of TECHNICAL.md 9 in order; each waits for the sibling
   stages its row names.

## Reading the specs

The specs were drafted ahead of any code and are not validated. Their
detail exists to surface integration problems early, not to be obeyed
word for word. Follow their intent, and do not ask whether to.

- Priority: `../spec/` vision, functional, technical, PEERS; then this
  folder's vision, functional, technical. The whole wins over this subsystem.
- When a spec is wrong, unclear or in the way, change it yourself: edit the
  document that owns the fact, fix each document that cites it, and keep
  it true to the documents above it.
- Ask only to go against a vision document or to weaken a security check.

## When blocked by a missing sibling

- Stage 1 needs dist S8, log L6 and net N1 and N2; stage 2 needs net N4. Never
  edit a sibling's code: record what is missing in TECHNICAL.md 12.
