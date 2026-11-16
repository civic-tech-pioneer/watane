#!/bin/sh
set -eu
cd "$(dirname "$0")"
for name in counter echo hog shelf recs loop-a loop-b cyc-a cyc-b menu item-a item-b item-c item-bad item-spin item-back manager; do
  [ -f "src/$name/Cargo.toml" ] || continue
  # The shared target directory when set; else the fixture's own (git-ignored) one.
  target="${CARGO_TARGET_DIR:+$CARGO_TARGET_DIR/host-fixtures/$name}"
  target="${target:-src/$name/target}"
  cargo build --target-dir "$target" --locked --release --target wasm32-wasip2 --manifest-path "src/$name/Cargo.toml"
  cp "$target/wasm32-wasip2/release/$(printf "%s" "$name" | tr - _).wasm" "mods/$name/logic.wasm"
done
