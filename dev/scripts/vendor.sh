#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo vendor --locked --versioned-dirs --manifest-path allowlist/Cargo.toml "$@" allowlist/vendor
