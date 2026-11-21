#!/bin/bash
# A guided tour of step 4 (find): Ivy lists Alice's mod in her index, Carol finds it by search.
# Run from the repo root: demo/demo4.sh. It pauses wherever you click in an app window.
set -euo pipefail
# A test build (--features driver, e.g. from factory/check-main.sh) hides windows unless told otherwise.
export WATANE_DRIVER_SCREEN=front
cd "$(dirname "$0")/.."
ROOT=$PWD
BIN=$ROOT/target/debug
DEMO=$(mktemp -d "${TMPDIR:-/tmp}/watane-demo4.XXXX")
PIDS=()
cleanup() { for p in "${PIDS[@]}"; do kill "$p" 2>/dev/null || true; done; echo; echo "Stopped. Demo files stay in $DEMO"; }
trap cleanup EXIT

say()   { printf '\n\033[1m%s\033[0m\n' "$*"; }
step()  { printf '  - %s\n' "$*"; }
pause() { printf '\n  [press Enter when done] '; read -r _; }

start_app() { # name -> sets PORT
  "$BIN/watane-app" --home "$DEMO/$1" --port 0 >"$DEMO/$1.log" 2>&1 & PIDS+=($!)
  for _ in $(seq 60); do [ -f "$DEMO/$1/run.json" ] && break; sleep 1; done
  [ -f "$DEMO/$1/run.json" ] || { echo "  $1's app did not start; see $DEMO/$1.log"; exit 1; }
}
serve_site() { # name port: a static host for that person's site folder
  mkdir -p "$DEMO/$1/dist/site"
  python3 -m http.server "$2" --bind 127.0.0.1 --directory "$DEMO/$1/dist/site" >"$DEMO/$1-site.log" 2>&1 & PIDS+=($!)
}
identity() { # name port
  printf 'http://127.0.0.1:%s/' "$2" | pbcopy
  step "In $1's app, Identity: type '$1' as profile name, click 'Set name', approve."
  step "Site: paste http://127.0.0.1:$2/ (on your clipboard) as Site URL, click 'Set site URL'."
}

say "Building watane (a minute if nothing changed)..."
cargo build -q -p watane-app -p watane-dev -p watane-host
(cd "$DEMO" && "$BIN/watane-dev" new tally >/dev/null)

# ---------------------------------------------------------------- Alice publishes
say "1. Alice publishes a mod (step 3, quickly)"
start_app alice; serve_site alice 8765
identity Alice 8765
pause
step "Her agent publishes 'tally': approve 'Sign a release' in Alice's app."
RESULT=$(cd "$DEMO/tally" && "$BIN/watane-dev" publish --json --home "$DEMO/alice" || true)
LINK=$(python3 -c "import json,sys;print(json.loads(sys.argv[1]).get('result',{}).get('link') or '')" "$RESULT")
[ -n "$LINK" ] || { echo "  $RESULT"; echo "  No link came back."; exit 1; }
CHANNEL=${LINK%%#*}
step "Alice's channel: $CHANNEL"

# ---------------------------------------------------------------- Ivy curates
say "2. Ivy keeps an index and lists Alice's mod in it"
start_app ivy; serve_site ivy 8766
identity Ivy 8766
pause
printf '%s' "$CHANNEL" | pbcopy
step "Channels: paste Alice's channel link (on your clipboard), click 'Follow channel'."
step "Under Alice, next to 'tally 0.1.0', type a note (\"counts things\") and click 'List in my index'."
step "Approve the signing. Ivy's site now publishes her index: one statement, signed by her."
step "Finally click 'Copy link' under Site: that is Ivy's index link."
pause

# ---------------------------------------------------------------- Carol finds
say "3. Carol finds Alice's mod through Ivy (step 4)"
IVY_LINK=$(pbpaste)
start_app carol
step "Everything here runs on 127.0.0.1. A listing may not send Carol's app to a local address"
step "(dist DD-26: a stranger's index could otherwise probe her network), unless Carol chose that"
step "publisher herself. On the internet Alice's site is public and this step falls away."
printf '%s' "$CHANNEL" | pbcopy
step "So first: in Find, paste Alice's channel link (on your clipboard) into 'Index link', click 'Add index'."
pause
printf '%s' "$IVY_LINK" | pbcopy
step "Now paste Ivy's link (on your clipboard again) into 'Index link' and click 'Add index'."
step "The Ivy row reads Ivy · 1 mod."
step "Type 'tally' in Words, click Search. The hit is by Alice, listed by Ivy, with Ivy's note,"
step "verified from Alice's own channel (Ivy only points at it; she cannot change it). Install it."
pause

say "   ...and Carol's coding agent can search the same indexes"
(cd "$DEMO" && "$BIN/watane-dev" search tally --home "$DEMO/carol") || true
pause

# ---------------------------------------------------------------- Alice withdraws
say "4. Alice withdraws the release; Carol is told"
step "Alice's app, Identity: next to 'tally 0.1.0' type a reason (\"counts wrong\"), click Withdraw, approve."
step "Carol's app: click 'Refresh channels'. Her tally shows withdrawn, with Alice's reason."
pause

say "That is step 4. Step 5 (build on another's mod) is landing now: S34, S35 done; S36, S37 in flight."
