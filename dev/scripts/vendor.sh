#!/bin/sh
set -eu
# Resolve the script, but keep relative output paths relative to the caller.
dev=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
out=${1:-"$dev/allowlist/vendor"}
if [ "$#" -gt 1 ]; then
    echo "usage: $0 [<out>]" >&2
    exit 2
fi
# Cargo's workspace and lock writes stay outside the watane checkout.
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT HUP INT TERM
cp "$dev/allowlist/Cargo.toml" "$dev/allowlist/Cargo.lock" "$dev/allowlist/lib.rs" "$work/"
cargo vendor --offline --locked --versioned-dirs --manifest-path "$work/Cargo.toml" "$out" ||
    cargo vendor --locked --versioned-dirs --manifest-path "$work/Cargo.toml" "$out"
