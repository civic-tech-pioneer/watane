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
cargo build -p watane-host && WATANE_BIN=$PWD/../target/debug/watane cargo test -- --include-ignored
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

## Spec gaps resolved

- S1 (dev): stage 1 ships a page-only template and next instruction; section 4 remains the full template.
- S1 (spec, pkg, host, dev): workspace resolver 3, one root lock file and shared target directory.
- S1-3 (dev): stop locates previews by canonical-path hash without checking mod files; absent matches report home null; §12 records the typed S1 check entry point.

- S2 cut (dev): counter template has no tests/; check stage 6, unit and contract tests join in S4; serve runs check and build only.
- S2 (dev): page reads once at startup; S3 restores watane.on(read).
- S2 (dev): cargo target directory uses shared build.target-dir config; generate-lockfile has no --target-dir argument.

- S3-2 (dev): the counter page displays counted event values and reads on changed, including stream open (host 8.4–8.5).
- S3-2 (dev): the fake-host feature does not exist yet; the commands run the real-host tests with WATANE_BIN and --include-ignored.

- S4-2 (dev): test runs sandboxed Wasm units with counts and locations; check stage 6 and the template contract file ship now; contract execution and serve testing wait for S4-3.
- S4-2 (dev): Wasmtime 48.0.8 unknown-import helper breaks WASI resource aliases; dev traps non-WASI functions directly and preserves the typed, capability-free WASI linker.
- S4-2 (dev): libtest needs --nocapture to retain panic locations on Wasm abort; the parser tracks a test across its printed output.
- S4-3 (dev): test runs contracts after units and serve gates previews on the whole pipeline; deliver and uncovered warnings wait for links/toolsets, fakes pass through unlinked, and CallError supplies code text through serialization.
- S4-3 (dev): a contract host-open error is one file Problem without a synthetic step; invalid-file errors retain syntax locations and do not stop other files.

- S5-2 (dev): serve builds API and MCP URLs from the cached Ready port, reuses its preview token, and points failed test pipelines to watane-dev test --json.
- S8 (dev): release cdylibs emit `<crate>.d` as well as hashed dependency names; scan and clear both forms for release and unit builds, refreshing the copied crate root to force new dep-info, completing DEV-S-10/12.

- S6-1 (dev): doctor retains targetless toolchain details, reports every environment gap, and shares the absolute vendor bootstrap fix with test/serve; template work starts with doctor.
- S6-2 (dev): refused serve warns with preview.previous only for a verified answering preview; J1 lives in tests/journey.rs and drives the reading-list mod through page calls and MCP.

- S11-3 (dev): local traversal reads pkg Deps before world validation, and add checks cycles before provider Mod validation, so pending import edits cannot hide cycles; serve tests and installs providers first and gives page/doctor/stop follow-ups (§17 item 12).

- S11-4 (dev): contracts dispatch delivery to Harness and compare its events; file fakes answer calls, and journey_s11 drives two served mods.

- S11 rejection fix (dev): refused serve points next and problem fixes to the first failing provider with an absolute --dir; the template documents working fakes and delivery.

- S13 (dev): §15 records the cache-copy fingerprint experiment, shared warm per-binary fixtures, unit rule replacements, slow/timing reasons and the complete current scenario proof map (27–31 remain planned).
