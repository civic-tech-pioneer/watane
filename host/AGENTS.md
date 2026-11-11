# host: rules for the building agent

## Read in this order
1. `../spec/VISION.md`, `../spec/FUNCTIONAL.md`, `../spec/TECHNICAL.md`: the whole.
2. `VISION.md`, `FUNCTIONAL.md` (requirements and scenarios S1 to S32), `TECHNICAL.md` here.
3. `../pkg/` and `../log/` specs: the types that the host uses (TECHNICAL.md section 17).
4. Reference only, never copied: `../colos/skeleton/crates/colos-runtime`,
   `colos-store`, `colos-catalog`, `colos-mcp`, `../colos/skeleton/src/lib.rs`.

## Commands
```bash
tests/fixtures/build.sh                        # rebuild fixture components (wasm32-wasip2)
cargo build
cargo test                                     # offline; network tests are #[ignore]
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

## Rules that matter most here
- Build in the stages of TECHNICAL.md section 15. A stage is done only when its tests pass.
- Each scenario of FUNCTIONAL.md is at least one test. A test compares with a literal
  or an artefact on disk, never with a value that the code under test computed.
- `host` is the security core. Never weaken a check of TECHNICAL.md section 14 to make
  a test pass. If a check blocks you, stop and report it.
- The caller of a call comes from the entry point, never from a request field.
- No new mechanism (a host interface, a route, a grant kind, a file in the home)
  without a change to TECHNICAL.md first. Edit the spec in place.
- Do not redefine a type of `pkg` or `log`. Do not parse `mod.toml` or WIT for
  anything that `pkg` gives.
- Do not sign, fetch, or compile a mod in `src/`. The fixture build script is the
  only compile step, and it is for tests.
- Write prose in plain English: short sentences, active voice.

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
- Record each change in one line where this file records spec gaps, naming
  the subsystems it affects.
- Ask only to go against a vision document or to weaken a security check.

## When a sibling is missing
- `pkg` missing: build stages 1 to 4 on the crate-private `RawMod`. Wait for `pkg`
  for stage 5; do not write a stand-in reader.
- `pkg` or `log` has another signature than section 17 cites (from their
  TECHNICAL.md): adapt the call site, and record the difference in section 17.
- `log` missing: stage 8 waits. Do not add the `watane-log` dependency before it.
- A spec gap: propose an answer, use it, and add it to TECHNICAL.md section 18.

## Spec gaps

- Q19 (host, dev): S4 provides Harness open/call and recorded events; defer linked fakes and deliver to links and functions to S5, keep tempfile for tests, and distinguish a missing Origin header.

- Q20 (host, dev): S5 adds MCP and preview tokens, reserves preview in every home, and defers app toolset management and Harness functions; wit-parser resolves to 0.254.2.
