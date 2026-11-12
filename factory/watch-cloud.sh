#!/bin/sh
# factory/watch-cloud.sh <slice id> [<commit already seen>]: returns when a cloud foreman has pushed a
# commit whose subject is `DELIVERED <id>` or `BLOCKED <id>`, to any branch on origin. Prints the
# branch, the commit and its message. Pass the last BLOCKED commit to wait for the next report.
ID=$1 SEEN=${2:-}
while :; do
  git fetch -q origin 2>/dev/null
  C=$(git log --remotes=origin -1 --format=%H -E --grep="^(DELIVERED|BLOCKED) $ID\$")
  if [ -n "$C" ] && [ "$C" != "$SEEN" ]; then
    git branch -r --contains "$C" | head -1; echo "$C"; git log -1 --format=%B "$C"; exit 0
  fi
  sleep 120
done
