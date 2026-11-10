# dev: guide for the builder

You build `watane-dev`, the maker tools of watane. Its commands and their
JSON results are the product: a coding agent reads them and acts alone.

## Read in this order

1. `VISION.md` (short), then `FUNCTIONAL.md` (requirements, scenarios).
2. `TECHNICAL.md`: build stage by stage (section 16).
3. `../spec/` for the whole; `../pkg/`, `../host/`, `../app/` for the
   items of `TECHNICAL.md` section 14.

## Commands

```bash
scripts/vendor.sh                       # once: the allowlist crates (needs the registry cache or network)
cargo build
cargo test                              # stages 0-4
cargo test --features fake-host         # adds serve, install, publish against fake-watane
WATANE_BIN=../host/target/debug/watane cargo test --features fake-host -- --include-ignored
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check                       # the toolchain needs: rustup target add wasm32-wasip2
```

## Rules that matter most here

- One stdout JSON object per command with `--json`; exit status by class (TECHNICAL 2.2). Every error has a code, a file and line where known, and a fix.
- No mod code runs natively: no `build.rs` or proc macro in a mod, builds in the cache copy, unit tests in Wasmtime. Keep `env_clear()` on every cargo run.
- dev opens no connection except HTTP to `localhost`. It holds no key and signs nothing.
- A test never calls the code under test to get its expectation (TECHNICAL 15).
- Do not redefine a sibling type (`Mod`, `Grant`, `Package`, `Harness`, `Ready`): use it.
- Keep it lean: no abstraction with one user, no seam "for later".
- Steps 4 and 5 (`search`, `--from`, `--release`) are not designed: do not build them.

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

- Follow the "Fake until it exists" column of TECHNICAL 16. Mark each test that needs the sibling `#[ignore = "needs <crate or program>"]`.
- If a sibling gives an item under another name or shape, use the sibling's and update TECHNICAL 14.
- If you need something no sibling offers, do not build it in dev. Add it to the owner's spec, record it under TECHNICAL 14 and 17, fake it, and go on.
