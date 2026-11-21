#!/bin/bash
# A guided tour of what watane does today (steps 1 to 3).
# Run from the repo root: demo/demo.sh. It pauses wherever you click in an app window.
set -euo pipefail
# A test build (--features driver, e.g. from factory/check-main.sh) hides windows unless told otherwise.
export WATANE_DRIVER_SCREEN=front
cd "$(dirname "$0")/.."
ROOT=$PWD
BIN=$ROOT/target/debug
DEMO=$(mktemp -d "${TMPDIR:-/tmp}/watane-demo.XXXX")
SITE_PORT=8765
PIDS=()
cleanup() { for p in "${PIDS[@]}"; do kill "$p" 2>/dev/null || true; done; echo; echo "Stopped. Demo files stay in $DEMO"; }
trap cleanup EXIT

say()   { printf '\n\033[1m%s\033[0m\n' "$*"; }
step()  { printf '  - %s\n' "$*"; }
pause() { printf '\n  [press Enter when done] '; read -r _; }

start_app() { # name -> sets PORT
  # --port 0: each app gets a free port (they all default to 47341).
  "$BIN/watane-app" --home "$DEMO/$1" --port 0 >"$DEMO/$1.log" 2>&1 & PIDS+=($!)
  for _ in $(seq 60); do [ -f "$DEMO/$1/run.json" ] && break; sleep 1; done
  [ -f "$DEMO/$1/run.json" ] || { echo "  $1's app did not start; see $DEMO/$1.log"; exit 1; }
  PORT=$(python3 -c "import json;print(json.load(open('$DEMO/$1/run.json'))['port'])")
}

say "Building watane (a minute if nothing changed)..."
cargo build -q -p watane-app -p watane-dev -p watane-host

# ---------------------------------------------------------------- step 1
say "1. A maker's coding agent makes a mod (step 1)"
step "This is what an agent runs: watane-dev new, then serve (the first build takes a minute)."
(cd "$DEMO" && "$BIN/watane-dev" new tally)
(cd "$DEMO/tally" && "$BIN/watane-dev" serve) | tee "$DEMO/serve.log"
URL=$(grep -o 'http://tally.localhost:[0-9]*/' "$DEMO/serve.log" | head -1)
open "$URL"
step "Your browser shows the mod's page: a counter. Click it. The logic runs as sandboxed WebAssembly."
step "The agent also got an MCP URL and token, so it can call the mod's tools itself."
pause

# ---------------------------------------------------------------- step 2
say "2. Alice's app runs mods that work together (step 2)"
start_app alice; ALICE_PORT=$PORT
step "Alice's app is open. Click 'Install folders'. In the dialog press Cmd+Shift+G and paste"
step "(it is on your clipboard): $ROOT/app/tests/fixtures"
step "Then Cmd-click both 'reading-list' and 'recommendations', and choose Open."
printf '%s' "$ROOT/app/tests/fixtures" | pbcopy
step "The consent window lists both mods and their permissions, reading list first (recommendations"
step "depends on it). Approve."
step "Open both mods. Add a book in the reading list: recommendations updates at once."
step "Optional: withdraw a permission from recommendations and watch it say so."
pause

say "   ...and the agent installs its own mod into Alice's app"
step "The agent runs: watane-dev install --home <Alice's home>. Approve in Alice's app."
(cd "$DEMO/tally" && "$BIN/watane-dev" install --home "$DEMO/alice") || true
step "'tally' is now in Alice's list next to the other two."
pause

# ---------------------------------------------------------------- step 3
say "3. Alice publishes, Bob follows her link (step 3)"
mkdir -p "$DEMO/alice/dist/site"
python3 -m http.server "$SITE_PORT" --bind 127.0.0.1 --directory "$DEMO/alice/dist/site" >"$DEMO/site.log" 2>&1 & PIDS+=($!)
step "Alice's site folder is now served at http://127.0.0.1:$SITE_PORT/ (it stands in for any static host)."
step "In Alice's app, Identity: type 'Alice' as profile name, click 'Set name', approve."
step "Site: paste http://127.0.0.1:$SITE_PORT/ (on your clipboard) as Site URL, click 'Set site URL'."
printf 'http://127.0.0.1:%s/' "$SITE_PORT" | pbcopy
pause

step "The agent publishes the tally mod it made in part 1: watane-dev publish --home <Alice's home>."
step "Alice's app asks 'Sign a release': it names tally 0.1.0, the package id and whether the source is in. Approve."
RESULT=$(cd "$DEMO/tally" && "$BIN/watane-dev" publish --json --home "$DEMO/alice" || true)
echo "  $RESULT"
LINK=$(python3 -c "import json,sys;print(json.loads(sys.argv[1]).get('result',{}).get('link') or '')" "$RESULT")
ID=$(python3 -c "import json,sys;print(json.loads(sys.argv[1]).get('result',{}).get('package') or '')" "$RESULT")
[ -n "$LINK" ] || { echo "  No link came back; see above."; exit 1; }
printf '%s' "$LINK" | pbcopy
step "Alice's link (on your clipboard): $LINK"

start_app bob
step "Bob's app is open (a separate home and identity). Paste the link into 'Install from link'"
step "and click it. The consent says: by Alice (her fingerprint), signature verified. Approve, then open tally."
pause

say "   ...and a changed file on the site is refused"
python3 - "$DEMO/alice/dist/site/packages/$ID" <<'EOF'
import sys; p=sys.argv[1]; b=bytearray(open(p,'rb').read()); b[0]^=1; open(p,'wb').write(b)
EOF
start_app carol
step "Someone changed one byte of the package on Alice's site. Carol's app is open: paste the same"
step "link (still on your clipboard) into 'Install from link'. It is refused with 'hash mismatch' and"
step "the package URL, and nothing is installed."
pause

say "That is everything up to step 3. Next: step 4, finding mods (search and following publishers)."
