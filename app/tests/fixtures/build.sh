#!/bin/sh
# Run manually when fixture sources change; tests use committed components.
set -eu
cd "$(dirname "$0")"
for name in reading-list recommendations ratings ratings-nofit stats judge-provider; do
  cargo build --locked --release --target wasm32-wasip2 --target-dir /private/tmp/watane-app-fixtures --manifest-path "src/$name/Cargo.toml"
  cp "/private/tmp/watane-app-fixtures/wasm32-wasip2/release/$(printf '%s' "$name" | tr - _).wasm" "$name/logic.wasm"
done
