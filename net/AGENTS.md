# net: rules for the builder of `watane-net`

## Read first, in this order

1. `../spec/VISION.md`, `../spec/FUNCTIONAL.md`, `../spec/TECHNICAL.md`,
   then `../spec/PEERS.md`: the architecture; stage D1 is step 8.
2. `VISION.md`, `FUNCTIONAL.md`, `TECHNICAL.md` here. TECHNICAL.md is the contract.
3. The specs in `../log/` (the types `net` uses) and `../dist/` (behind the
   holder). `REVIEW.md` and `ARCHITECTURE-PROPOSAL.md` here are records of
   how the design was decided, not contracts.

## Commands

`cargo build && cargo test` (loopback only, no internet);
`cargo clippy --all-targets -- -D warnings && cargo fmt --check`.

## Rules that matter most here

1. The caller's node key comes from the QUIC handshake, never from a
   request field.
2. Nothing unverified reaches the holder: decode each entry, check its
   author and seq order, check each blob's BLAKE3 hash and size.
3. Forward original bytes only. Never re-sign, re-encode or summarise
   another key's object.
4. Never open `<home>/log/` or `<home>/dist/`. Channels live in the holder.
5. The node key signs only the forms of TECHNICAL.md 4.3, each with its
   own prefix. Add no general `sign`.
6. Every request has a byte limit and a deadline, and every caller a
   budget. Check them before doing the work. Pull only; never forward.
   Nothing is reachable by default: no relay, no DHT.
7. A test computes its expectation without `watane_net`. No test reaches
   the internet (TECHNICAL.md 7).
8. Build the stages of TECHNICAL.md section 9 in order (step 8). Build
   nothing of the later stages of `PEERS.md` 10: they are a sketch.

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

- `log` exists; N1 to N4 need nothing else (`tests/support::MemHoldings`
  stands in for the holder).
- The `Holdings` implementation over `dist` belongs to `node`, and the
  keyless open to `log` and `dist`. Record what they need in TECHNICAL.md
  section 11; never edit their code.
- A design question: decide it as "Reading the specs" says, record it in
  section 12, and go on.
