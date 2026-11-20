#!/bin/bash
# factory/check-main.sh: what CI (.github/workflows/ci.yml) cannot run, on `main` after each merge:
# the timing tests, one at a time on the Mac while no slice builds (factory/README.md, "Timing
# tests"). Build, default tests, clippy, fmt and cargo deny on Linux, macOS and Windows are CI's,
# before the merge ("Two feedback loops"). A failure counts only if it repeats.
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
T=(-- --ignored --test-threads=1)
cargo build -q -p watane-host >> "$LOG" 2>&1; export WATANE_BIN=${CARGO_TARGET_DIR:-$W/target}/debug/watane  # dev's tests
step host-runtime cargo test -p watane-host --test it "${T[@]}" runtime::ac6_limits
step host-shutdown cargo test -p watane-host --test it "${T[@]}" program::catalog::ac3_shutdown
step dev-unit cargo test -p watane-dev --test it "${T[@]}" unit_tests::ac10_endless
if [ ${#FAILED[@]} -eq 0 ]; then echo "MAIN PASS $HEAD" | tee -a "$LOG"; exit 0; fi
echo "MAIN FAIL $HEAD: ${FAILED[*]} (log $LOG)" | tee -a "$LOG"; exit 1
