#!/bin/bash
# A guided tour of the mod manager (stage I, app TECHNICAL 5.3 and 5.4) in the real app:
# the first start, the Library, a mod's details and connections, the main mod at launch,
# the menu, Settings and, in part 2, Find and Updates with a second person (Alice).
# Run from anywhere: demo/demo-manager.sh. It pauses wherever you click in an app window.
# Set CARGO_TARGET_DIR to build somewhere other than target/.
set -euo pipefail
# A test build (--features driver, e.g. from factory/check-main.sh) hides windows unless told otherwise.
export WATANE_DRIVER_SCREEN=front
cd "$(dirname "$0")/.."
ROOT=$PWD
BIN=${CARGO_TARGET_DIR:-$ROOT/target}/debug
FIX=$ROOT/app/tests/fixtures
DEMO=$(mktemp -d "${TMPDIR:-/tmp}/watane-manager.XXXX")
SITE_PORT=${SITE_PORT:-8775}
PIDS=()
cleanup() { for p in "${PIDS[@]}"; do kill "$p" 2>/dev/null || true; done; echo; echo "Stopped. Demo files stay in $DEMO"; }
trap cleanup EXIT

say()   { printf '\n\033[1m%s\033[0m\n' "$*"; }
step()  { printf '  - %s\n' "$*"; }
pause() { printf '\n  [press Enter when done] '; read -r _; }
ask()   { printf '\n  %s [y/N] ' "$1"; read -r a; [ "$a" = y ] || [ "$a" = Y ]; }

APP_PID=
start_app() { # name -> sets APP_PID, PORT
  rm -f "$DEMO/$1/run.json"
  "$BIN/watane-app" --home "$DEMO/$1" --port 0 >>"$DEMO/$1.log" 2>&1 & APP_PID=$!; PIDS+=($APP_PID)
  for _ in $(seq 60); do [ -f "$DEMO/$1/run.json" ] && break; sleep 1; done
  [ -f "$DEMO/$1/run.json" ] || { echo "  $1's app did not start; see $DEMO/$1.log"; exit 1; }
  PORT=$(python3 -c "import json,sys;print(json.load(open(sys.argv[1]))['port'])" "$DEMO/$1/run.json")
}
stop_app() { kill "$1" 2>/dev/null || true; wait "$1" 2>/dev/null || true; }

say "Building watane-app and watane-dev (a minute if nothing changed)..."
cargo build -q -p watane-app -p watane-dev
mkdir -p "$DEMO/mods"
for m in desk reading-list recommendations ratings stats; do cp -R "$FIX/$m" "$DEMO/mods/$m"; rm -rf "$DEMO/mods/$m/tests"; done
step "Five test mods are in $DEMO/mods:"
step "  Desk (main = true in its mod.toml, a page), Reading list, Recommendations (calls and"
step "  listens to Reading list), Ratings, Stats (calls Ratings). Their pages are bare test pages."

# ------------------------------------------------------------ first start
say "1. First start: an empty home (design: Empty)"
start_app you; YOU_PID=$APP_PID
step "A window 'watane' opens on Mods > Library: 'Install your first mod', Choose a folder, Install from link."
step "Click Find, then Updates: both are empty (no index, no channel, nothing installed)."
step "Menu watane > Settings… (⌘,): identity, site, agents, own machines. Then watane > Mods… (⇧⌘M)."
pause

# ------------------------------------------------------------ install
say "2. Install four mods, the way a coding agent does (watane-dev install: one POST /api/install)"
JSON=$(python3 -c "import json,sys;print(json.dumps({'folders':[sys.argv[1]+'/'+m for m in ['reading-list','recommendations','ratings','stats']]}))" "$DEMO/mods")
curl -s -X POST -H 'content-type: application/json' --data "$JSON" "http://127.0.0.1:$PORT/api/install" >"$DEMO/install.json" 2>&1 &
PIDS+=($!)
step "The consent window opens with one plan for all four. Read it, then click Approve."
pause
say "   ...and Desk through the app itself, which makes it the main mod"
printf '%s' "$DEMO/mods/desk" | pbcopy
step "Library > Install a mod > Choose a folder. In the dialog press ⇧⌘G, paste (the desk folder is on your"
step "clipboard), Return, Open. Approve 'Install Desk'."
step "Desk says main = true, and no main mod was set, so Desk becomes main. Note: the four mods"
step "from step 2 could not: only the app's own installs adopt a main mod (TECHNICAL 5.4)."
pause

# ------------------------------------------------------------ library
say "3. Library and details"
step "Five entries, each with Open, Disable, Remove and a 'Details' disclosure."
step "Reading list > Details: 'Used by: recommendations', 'Extended by: recommendations',"
step "  'Make main', version, 'local', page, package, and its grant with a Withdraw button."
step "Desk > Details: 'Unset main' and 'Works on its own…'."
step "Recommendations > Details: click Withdraw next to 'Call … of reading-list'. 'Uses:' goes away"
step "  (and 'Used by' at Reading list). Click 'Give back' to restore it."
step "Click 'recommendations' under Reading list's connections: it jumps to that entry."
step "Reading list > Disable: a warning names recommendations; Cancel is focused. Cancel."
step "Type 'stat' in Filter mods; set Source to Local / Published. Clear it again."
pause

# ------------------------------------------------------------ launch with main mod
say "4. Launch with a main mod (design: Shell)"
stop_app "$YOU_PID"
start_app you; YOU_PID=$APP_PID
step "The app restarted. It opened 'Desk — local' (a bare 'Your desk' test page), not the manager."
step "Menu watane > Mods… (⇧⌘M): the manager opens on Library. watane > Settings… (⌘,): Settings."
step "Close the manager window; Desk stays open. ⇧⌘M brings the manager back."
pause

if ! ask "Part 2: Find and Updates with a second person, Alice (about 5 minutes, two app windows)?"; then
  say "Done. Without part 2, Find and Updates stay empty: they need an index, a channel and a release."
  exit 0
fi

# ------------------------------------------------------------ alice publishes
say "5. Alice publishes a mod"
start_app alice; ALICE_PID=$APP_PID
mkdir -p "$DEMO/alice/dist/site"
python3 -m http.server "$SITE_PORT" --bind 127.0.0.1 --directory "$DEMO/alice/dist/site" >"$DEMO/alice-site.log" 2>&1 & SITE_PID=$!; PIDS+=($SITE_PID)
printf 'http://127.0.0.1:%s/' "$SITE_PORT" | pbcopy
step "A second 'watane' window opened: Alice's. Her menu watane > Settings… (⌘,)."
step "Identity: type 'Alice' as profile name, click 'Set name', approve."
step "Site: paste http://127.0.0.1:$SITE_PORT/ (on your clipboard) as Site URL, click 'Set site URL'."
pause
(cd "$DEMO" && "$BIN/watane-dev" new tally >/dev/null)
step "Her agent publishes 'tally' (builds and tests first, a minute): approve 'Sign a release' in Alice's app."
RESULT=$(cd "$DEMO/tally" && "$BIN/watane-dev" publish --json --home "$DEMO/alice" || true)
LINK=$(python3 -c "import json,sys;print(json.loads(sys.argv[1]).get('result',{}).get('link') or '')" "$RESULT")
[ -n "$LINK" ] || { echo "  $RESULT"; echo "  No link came back."; exit 1; }
CHANNEL=${LINK%%#*}

# ------------------------------------------------------------ find
say "6. Find (design: Find)"
printf '%s' "$CHANNEL" | pbcopy
step "Back in YOUR app (the manager, ⇧⌘M): Find. Paste Alice's channel link (on your clipboard)"
step "into 'Index link', click 'Add index'. Rows: 'Your index · you' and 'Alice (…) · 1 mod'."
step "(Everything runs on 127.0.0.1, so it must be her own channel: a stranger's index may not"
step " point your app at a local address. See demo4.sh.)"
step "Type 'tally' in Words, click Search. Tally 0.1.0 by Alice, signature verified, via Alice. Install, approve."
pause

# ------------------------------------------------------------ updates
say "7. Updates (design: Updates)"
step "Alice's app, Settings > Identity: next to 'tally 0.1.0' type a reason (\"counts wrong\"), Withdraw, approve."
step "Your app: Updates > 'Check now' (top right). The Updates tab gets a count of 1 and shows"
step "  'Tally: Withdrawn by the publisher: counts wrong'. Library > Tally shows it too."
pause
kill "$SITE_PID" 2>/dev/null || true
step "Alice's site is now down. Your app: Updates > 'Check now' again."
step "Updates > 'Channels that could not be read': Alice … Connection refused."
step "'Installed without a question' stays empty: that needs a newer release asking nothing new."
pause

say "That is the manager. Quit with Enter; the script closes every app it started."
pause
