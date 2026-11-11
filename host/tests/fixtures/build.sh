#!/bin/sh
set -eu
cd "$(dirname "$0")"
for name in counter echo hog shelf recs loop-a loop-b cyc-a cyc-b; do
  [ -f "src/$name/Cargo.toml" ] || continue
  cargo build --target-dir "src/$name/target" --locked --release --target wasm32-wasip2 --manifest-path "src/$name/Cargo.toml"
  cp "src/$name/target/wasm32-wasip2/release/$(printf "%s" "$name" | tr - _).wasm" "mods/$name/logic.wasm"
done
