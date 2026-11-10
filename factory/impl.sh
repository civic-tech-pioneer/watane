#!/bin/bash
# Run the Sol implementer (Codex) on one task brief, or send it back with feedback in the same session.
#   factory/impl.sh <task>            new run on factory/tasks/<task>.md
#   factory/impl.sh <task> resume     same Codex session; the feedback is read from stdin
# The brief's header sets `Subsystem: <folder>` and `Effort: <low|medium|high|xhigh>`.
# Run it from the slice's worktree. Output goes to factory/runs/ of the main checkout, so it survives
# the worktree: <task>.{jsonl,last.md,session}, and one line per run in ledger.jsonl.
set -u
F=$(cd "$(dirname "$0")" && pwd); W=$(dirname "$F")
G=$(git -C "$W" rev-parse --path-format=absolute --git-common-dir); GD=$(git -C "$W" rev-parse --absolute-git-dir)
R=$(dirname "$G")/factory/runs
ID=$1 MODE=${2:-new} T=$F/tasks/$1.md
[ -f "$T" ] || { echo "no brief $T" >&2; exit 2; }
SUB=$(sed -n 's/^Subsystem: *//p' "$T" | head -1); EFFORT=$(sed -n 's/^Effort: *//p' "$T" | head -1)
[ -n "$SUB" ] && [ -f "$W/$SUB/AGENTS.md" ] || { echo "brief needs 'Subsystem: <pkg|host|dev|app|log|dist>'" >&2; exit 2; }
mkdir -p "$R"
# ponytail: Codex's own workspace-write sandbox plus network for cargo. Colos needed a custom Seatbelt
# profile (factory/sol-impl.sb there) only for iroh's AF_ROUTE socket; watane has no iroh. Revisit if a test needs it.
OPTS=(-m gpt-6.1-sol -c "model_reasoning_effort=\"${EFFORT:-medium}\"" -c 'sandbox_mode="workspace-write"'
      -c 'sandbox_workspace_write.network_access=true' --json -o "$R/$ID.last.md"
      # writable: the worktree, both git dirs (Codex protects .git unless each is named), runs, cargo home.
      # Given as -c, not --add-dir, because `codex exec resume` takes only -c.
      -c "sandbox_workspace_write.writable_roots=[\"$W\",\"$G\",\"$GD\",\"$R\",\"${CARGO_HOME:-$HOME/.cargo}\"]")
cd "$W/$SUB" || exit 2  # resume runs in the current folder
T0=$(date +%s)
if [ "$MODE" = resume ]; then
  SID=$(cat "$R/$ID.session") || exit 2
  codex exec resume "$SID" "${OPTS[@]}" - >> "$R/$ID.jsonl" 2>&1; RC=$?
else
  { cat "$F/implementer.md"; printf '\n\n# THE TASK\n\n'; cat "$T"; } |
    codex exec "${OPTS[@]}" - > "$R/$ID.jsonl" 2>&1; RC=$?
  grep -m1 -o '"thread_id":"[^"]*"' "$R/$ID.jsonl" | cut -d'"' -f4 > "$R/$ID.session"
fi
printf '{"task":"%s","mode":"%s","effort":"%s","start":%s,"end":%s,"exit":%s}\n' \
  "$ID" "$MODE" "${EFFORT:-medium}" "$T0" "$(date +%s)" "$RC" >> "$R/ledger.jsonl"
echo "$ID $MODE exit=$RC"; tail -5 "$R/$ID.last.md" 2>/dev/null
exit $RC
