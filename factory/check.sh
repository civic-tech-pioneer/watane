#!/bin/bash
# factory/check.sh <task> [<base commit>]: runs a task's checks outside Sol's sandbox, in the current
# worktree: build, the default tests, clippy, fmt, and the `slow:` tests of each test file the task
# touched since <base> (app files with the `driver` feature). Never `front:`, `timing:` or `real:` ones.
# impl.sh runs it after every Sol run; a cloud foreman runs it after the implementer agent; a
# validator reruns it after its own fixes. Log: factory/runs/<task>.check.log of the main checkout.
# Prints `CHECK PASS <commit>` or `CHECK FAIL <commit>: <steps>`; exits 1 on a failure.
set -u
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
F=$(cd "$(dirname "$0")" && pwd); W=$(dirname "$F")
R=$(dirname "$(git -C "$W" rev-parse --path-format=absolute --git-common-dir)")/factory/runs
ID=$1 BASE=${2:-$(git -C "$W" merge-base HEAD main)} T=$F/tasks/$1.md  # no base: since main
SUB=$(sed -n 's/^Subsystem: *//p' "$T" | head -1)
[ -n "$SUB" ] && [ -d "$W/$SUB" ] || { echo "no Subsystem in $T" >&2; exit 2; }
mkdir -p "$R"; LOG=$R/$ID.check.log
cd "$W/$SUB" || exit 2
HEAD=$(git rev-parse --short HEAD); [ -n "$(git status --porcelain -- .)" ] && HEAD="$HEAD+dirty"
FAILED=()
echo "# $ID at $HEAD, base $BASE, $(date)" > "$LOG"
step() {  # step <name> <command...>: runs it, logs it, records a failure
  echo "== $1: ${*:2}" >> "$LOG"
  "${@:2}" >> "$LOG" 2>&1 || { FAILED+=("$1"); echo "== $1 FAILED" >> "$LOG"; }
}
# names of the tests in a file whose ignore reason starts with $2 (slow, front, ...)
names() { awk -v r="#[ignore = \"$2:" 'index($0,r){f=1;next} f&&/fn [a-z0-9_]+/{match($0,/fn [a-z0-9_]+/);print substr($0,RSTART+3,RLENGTH-3);f=0}' "$1"; }
CLIPPY=(cargo clippy --all-targets -- -D warnings); FEAT=()
case $SUB in
  app) CLIPPY=(cargo clippy --all-targets --features driver -- -D warnings); FEAT=(--features driver) ;;
  dev) CLIPPY=(cargo clippy --all-targets --all-features -- -D warnings)
       cargo build -q -p watane-host >> "$LOG" 2>&1; export WATANE_BIN=$W/target/debug/watane ;;
esac
step build cargo build
step test cargo test --no-fail-fast
step clippy "${CLIPPY[@]}"
step fmt cargo fmt --check
for f in $(git diff --name-only "$BASE" -- tests/ | grep -E '^'"$SUB"'/tests/[^/]+\.rs$'); do
  p=$W/$f; [ -f "$p" ] || continue
  SLOW=$(names "$p" slow); [ -n "$SLOW" ] || continue
  SKIP=(); for n in $(names "$p" front; names "$p" timing; names "$p" real); do SKIP+=(--skip "$n"); done
  # ponytail: names are substring filters; a slow name that contains a skipped one is skipped too
  step "slow $(basename "$f" .rs)" cargo test ${FEAT[@]+"${FEAT[@]}"} --test "$(basename "$f" .rs)" -- --ignored $SLOW ${SKIP[@]+"${SKIP[@]}"}
done
if [ ${#FAILED[@]} -eq 0 ]; then echo "CHECK PASS $HEAD"; echo "CHECK PASS $HEAD" >> "$LOG"; exit 0; fi
echo "CHECK FAIL $HEAD: ${FAILED[*]} (log $LOG)" | tee -a "$LOG"; exit 1
