# Recommendations contract fixture

`recommendations/` is a built-only mod, with its committed `logic.wasm`.
It calls `mods:reading-list/books`, retains delivered titles, and emits a
`changes.received` event from its subscription export. Tests copy it into
a temporary folder, then run `watane-dev test --json` without compiling it.
`src/recommendations/` contains the pinned source crate. The slow S11
journey writes that same logic into a newly created mod and builds it
through watane-dev's sandboxed builder.

To rebuild the committed component from the dev folder (requires the
Rust wasm32-wasip2 target and the crates in the registry cache):

```sh
env -i PATH="$PATH" RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}" \
  CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}" \
  cargo build --offline --locked --release --target wasm32-wasip2 \
  --target-dir /tmp/watane-s11-fixture-target \
  --manifest-path tests/fixtures/src/recommendations/Cargo.toml
cp /tmp/watane-s11-fixture-target/wasm32-wasip2/release/recommendations.wasm \
  tests/fixtures/recommendations/logic.wasm
```
