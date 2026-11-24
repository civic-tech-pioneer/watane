#!/bin/sh
set -eu
cd "$(dirname "$0")"
for name in counter echo hog shelf recs loop-a loop-b cyc-a cyc-b menu item-a item-b item-c item-bad item-spin item-back manager latch relay relay-front advo advo-front claude claude-b claude-front claude-evt tool tool-plus tool-manage toolbox toolbox-front; do
  [ -f "src/$name/Cargo.toml" ] || continue
  # One target folder for all fixtures, so their shared dependencies build once: under the
  # shared target directory when set, else a git-ignored one here.
  target="${CARGO_TARGET_DIR:+$CARGO_TARGET_DIR/host-fixtures}"
  target="${target:-target}"
  cargo build --target-dir "$target" --locked --release --target wasm32-wasip2 --manifest-path "src/$name/Cargo.toml"
  cp "$target/wasm32-wasip2/release/$(printf "%s" "$name" | tr - _).wasm" "mods/$name/logic.wasm"
done
