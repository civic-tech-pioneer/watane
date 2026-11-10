#!/bin/sh
# factory/watch.sh <sessionId>: returns when that background foreman's process exits or its state
# is `blocked` (a permission prompt). The product owner runs it in the background after a handout.
SID=$1
while :; do
  L=$(claude agents --json 2>/dev/null | python3 -c "
import json,sys
for a in json.load(sys.stdin):
    if a.get('sessionId')=='$SID': print(a.get('pid',''), a.get('state') or a.get('status'))")
  set -- $L
  [ -z "${1:-}" ] && { echo "$SID: not listed"; exit 1; }
  kill -0 "$1" 2>/dev/null || { echo "$SID: process $1 exited"; exit 1; }
  [ "${2:-}" = blocked ] && { echo "$SID: blocked on a prompt"; exit 2; }
  sleep 60
done
