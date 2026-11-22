# Recommendations contract fixture

`recommendations/` is a built-only mod, with its committed `logic.wasm`.
It calls `mods:reading-list/books`, retains delivered titles, and emits a
`changes.received` event from its subscription export. Tests copy it into
a temporary folder, then call `watane_dev::contract::contract_tests` on it.
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

# Released ratings fixture

`ratings/` is the unmodified counter template named `ratings`, built through
`watane-dev test --json`. It keeps only the package files without source. To
rebuild, run `watane-dev new ratings --dir /tmp/ratings`, then
`watane-dev test --dir /tmp/ratings --json` and copy `mod.toml`, `wit/`, `ui/`
and `logic.wasm` into `ratings/`. It exports `api.get`, `api.add` and
`changes.counted`; all expectations are the literal template contract.

# Collect fixtures (S65-3)

`menu/` (own outlet `menu` with `ask: func() -> list<string>`, world `shell`) and `item/`
(exports `mods:menu/menu`, `ask` returns `["a"]`) are copies of the host's `menu` and `item-a`
fixtures with their committed `logic.wasm` (`item` renamed from `item-a` in `mod.toml` and
`wit/world.wit`; the component names no own package); rebuild them from `../../host/tests/fixtures/build.sh`.

`probe/` (`world.wit`, `lib.rs`) is the logic of the slow S99 test
`fake_claude::ac9_...`: a mod that starts Claude Code, says what the page
tells it, and keeps what its stream delivers. The test writes it into a new mod
named `hello` (the word `probe` becomes the slug).
