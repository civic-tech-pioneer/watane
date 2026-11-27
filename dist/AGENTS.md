# dist: rules for the builder of `watane-dist`

## Read first, in this order

1. `../spec/VISION.md`, `../spec/FUNCTIONAL.md`, `../spec/TECHNICAL.md`.
2. `VISION.md`, `FUNCTIONAL.md`, `TECHNICAL.md` here. TECHNICAL.md is the contract.
3. The specs in `../log/` and `../pkg/`: the types `dist` uses (TECHNICAL.md 10).

## Commands

`cargo build && cargo test` (local HTTP only);
`cargo clippy --all-targets -- -D warnings && cargo fmt --check`.

## Rules that matter most here

1. `sign_and_append` is the only caller of `Log::append`, and it
   asks `Confirm` first. No other path signs, also not in a test helper.
2. Verify before use: `Entry::decode` for each entry file, the key of the
   link, the seq of the file name, BLAKE3 for each package. Nothing
   unverified reaches the caller, or the disk outside a temp file.
3. Never build a local path from remote text: only from a `u64` seq or a
   verified hash.
4. Every read from a web host has a body limit and a whole-request timeout.
5. A test computes its expectation without `watane_dist` (TECHNICAL.md 7).
   Network tests use `tiny_http` on `127.0.0.1:0` only.
6. Do not redefine a type of `log` or `pkg`. Change a format of section 4
   only by changing section 4 first.
7. No async runtime, no thread (but the lookup thread of `resolve.rs`, TECHNICAL.md 4.5),
   no timer, no listening socket. Step 5
   is designed (section 11, stage S7); build it only in that stage.
   Step 8 (5.12, stage S8) likewise: the holder methods take no read lock.

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

## When blocked by a missing sibling

- Build the stages of TECHNICAL.md section 8 in order; S1 needs almost nothing.
- Missing item: a stub crate in `stubs/<name>/` with the exact signatures of
  section 10; point the path dependency at it; delete it when the sibling
  has the item. Never edit another subsystem's code.
- A sibling offers another signature: adapt the call; update section 10.
- A design question (a new kind, a new site file, a new wire form): decide
  it as "Reading the specs" says, record it in section 12, and go on.
