#!/bin/bash
# factory/check-main.sh: the whole workspace's checks, for `main` after each merge. check.sh checks
# one task's subsystem; this catches the breakage that lands between subsystems (S85: app clippy red
# on sibling log/pkg code). Build, default tests, clippy, fmt and cargo deny, over every crate.
# The product owner runs it after each merge (product-owner skill, section 5). About 5 to 15 minutes.
# Log: factory/runs/main.check.log of the main checkout. Prints `MAIN PASS <commit>` or
# `MAIN FAIL <commit>: <steps>`; exits 1 on a failure.
set -u
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
W=$(cd "$(dirname "$0")/.." && pwd)
R=$(dirname "$(git -C "$W" rev-parse --path-format=absolute --git-common-dir)")/factory/runs
mkdir -p "$R"; LOG=$R/main.check.log
cd "$W" || exit 2
# Tests and agents never touch the person's real home (S58 created ~/.watane on the Mac).
WATANE_HOME=$(mktemp -d); export WATANE_HOME; trap 'rm -rf "$WATANE_HOME"' EXIT
HEAD=$(git rev-parse --short HEAD); [ -n "$(git status --porcelain)" ] && HEAD="$HEAD+dirty"
FAILED=()
echo "# main at $HEAD, $(date)" > "$LOG"
step() {
  echo "== $1: ${*:2}" >> "$LOG"
  "${@:2}" >> "$LOG" 2>&1 || { FAILED+=("$1"); echo "== $1 FAILED" >> "$LOG"; }
}
# app's `driver` and dev's `fake-host` as check.sh uses them; never app's `real-github`.
FEAT=(--features watane-app/driver,watane-dev/fake-host)
cargo build -q -p watane-host >> "$LOG" 2>&1; export WATANE_BIN=${CARGO_TARGET_DIR:-$W/target}/debug/watane  # dev's tests
step build cargo build --workspace "${FEAT[@]}"
step test cargo test --workspace "${FEAT[@]}" --no-fail-fast
step clippy cargo clippy --workspace --all-targets "${FEAT[@]}" -- -D warnings
step fmt cargo +nightly fmt --all --check
step deny cargo deny check  # advisories too: a new one is news for the product owner, not a task
if [ ${#FAILED[@]} -eq 0 ]; then echo "MAIN PASS $HEAD" | tee -a "$LOG"; exit 0; fi
echo "MAIN FAIL $HEAD: ${FAILED[*]} (log $LOG)" | tee -a "$LOG"; exit 1
