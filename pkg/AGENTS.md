# pkg: notes for the builder

`watane-pkg` defines what a mod, a package and a release are. `host`,
`dev` and `dist` depend on it, so its formats and API are load-bearing.

## Read first

1. `../spec/VISION.md`, `../spec/FUNCTIONAL.md`, `../spec/TECHNICAL.md` (§3, §4, §7).
2. `VISION.md`, `FUNCTIONAL.md`, `TECHNICAL.md` of this folder. `TECHNICAL.md` is the contract.
3. Reference only, never copied: `../../colos/skeleton/src/share.rs`,
   `../../colos/skeleton/crates/colos-catalog`, `../../colos/crates/colos-log/src/zbase32.rs`.

## Commands

```bash
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

## Rules that matter most here

- Build in the stages of `TECHNICAL.md` section 12; each stage ends green.
- Write the test first, with its expectation from the spec (golden hex,
  tables, literals). Never compute an expected value with the code under test.
- Do not change a byte format, a grammar or a public signature without
  first changing `TECHNICAL.md`. Other subsystems build against it.
- No `wit-parser` type in the public API. No `unsafe`. No panic on input:
  every refusal is a value that names the rule.
- Parse untrusted input only after its size limit is checked.
- No dependency on another watane subsystem. No network, no process, no Wasm runtime.

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

- `pkg` needs no sibling to build or test. If a sibling's need is missing
  from `TECHNICAL.md` section 13, add it to section 15 ("Questions for spec/")
  with a proposed signature, use the proposal, and go on.
- If the spec is unclear, decide the simplest answer that keeps the formats
  stable, record it in section 15, and go on.
