#!/bin/sh
set -eu
cd "$(dirname "$0")"
for name in counter echo hog shelf recs loop-a loop-b cyc-a cyc-b menu item-a item-b item-c item-bad item-spin item-back manager; do
  [ -f "src/$name/Cargo.toml" ] || continue
  cargo build --target-dir "${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR to the shared target directory}/host-fixtures/$name" --locked --release --target wasm32-wasip2 --manifest-path "src/$name/Cargo.toml"
  cp "$CARGO_TARGET_DIR/host-fixtures/$name/wasm32-wasip2/release/$(printf "%s" "$name" | tr - _).wasm" "mods/$name/logic.wasm"
done
