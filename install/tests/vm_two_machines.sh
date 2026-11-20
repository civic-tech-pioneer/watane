#!/bin/sh
# J3 across the VM network (TECHNICAL 12.6).
set -eu
if ! command -v tart >/dev/null 2>&1; then
  printf '%s\n' 'vm_two_machines.sh: tart is not installed (see install/TECHNICAL.md 12.6)' >&2
  exit 2
fi
driver=vm_two_machines.sh
fail() { printf '%s: %s\n' "$driver" "$*" >&2; exit 1; }
check() { "$@" || fail "command failed: $*"; }
from=
while [ "$#" -gt 0 ]; do
  case $1 in
    --from) [ "$#" -ge 2 ] || fail '--from needs a directory'; from=$2; shift ;;
    *) fail "unknown argument: $1" ;;
  esac
  shift
done
here=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd -P)
if [ -n "$from" ]; then
  from=$(cd "$from" && pwd -P) || fail "cd $from"
  for asset in install.sh SHA256SUMS; do check test -f "$from/$asset"; done
fi
vm_step_limit=300
vm_whole_limit=3600
# shellcheck source-path=SCRIPTDIR source=vm_lib.sh
. "$here/vm_lib.sh"
check cd "$here/../.."
check bounded "$whole_bound" "cargo build" cargo build -p watane-app --features driver
check bounded "$whole_bound" "cargo test --no-run" cargo test -p watane-app --features driver --test it --no-run
install_bound=$(seconds 900)
journey_bound=$(seconds 1200)
boot watane-clean
# No controlling terminal: both installation and the driver channel require -T.
remote_plain() {
  bounded "$step_bound" remote_plain ssh -T -o LogLevel=ERROR -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
    -o PreferredAuthentications=password -o PubkeyAuthentication=no \
    -o NumberOfPasswordPrompts=1 -o ConnectTimeout=5 "admin@$ip" "$@" </dev/null
}
remote_install() {
  bounded "$install_bound" install ssh -T -o LogLevel=ERROR -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
    -o PreferredAuthentications=password -o PubkeyAuthentication=no \
    -o NumberOfPasswordPrompts=1 -o ConnectTimeout=5 "admin@$ip" "$@" </dev/null
}
check remote_plain 'set -eu
  mount > /tmp/watane-mounts
  cat /tmp/watane-mounts
  if grep -Ei "[ (](virtiofs|smbfs|nfs|afpfs|webdav)[ ,)]" /tmp/watane-mounts; then
    echo "shared filesystem found (mount)" >&2; exit 1
  fi
  [ ! -e "/Volumes/My Shared Files" ]'
if [ -n "$from" ]; then
  check remote_plain 'mkdir -p /Users/admin/watane-release'
  check copy "$from/." "admin@$ip:/Users/admin/watane-release/"
  check remote_install 'WATANE_RELEASE_BASE=file:///Users/admin/watane-release sh /Users/admin/watane-release/install.sh'
else
  check remote_install 'curl -fsSL https://github.com/civic-tech-pioneer/watane/releases/latest/download/install.sh | sh'
fi
# shellcheck disable=SC2016 # HOME expands in the guest.
check remote_plain '"$HOME/.local/bin/watane" --version'
check mkdir -p "$work/Bob.app/Contents/MacOS"
check cp target/debug/watane-app "$work/Bob.app/Contents/MacOS/watane-app"
cat >"$work/Bob.app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>watane-app</string>
<key>CFBundleIdentifier</key><string>org.watane.bob-driver</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>
PLIST
ats=$(plutil -extract NSAppTransportSecurity json -o - app/Info.plist) || fail 'plutil: extract release ATS'
check plutil -insert NSAppTransportSecurity -json "$ats" "$work/Bob.app/Contents/Info.plist"
cat >"$work/bob.sh" <<'BOB'
#!/bin/sh
# Guest GUI-session bridge (TECHNICAL 12.6).
set -eu
case ${1:-} in .watane|bob-tamper) home_name=$1 ;; *) exit 2 ;; esac
work=$(mktemp -d)
app=
writer=
reader=
cleanup() {
  status=$?
  trap - EXIT HUP INT TERM
  [ -z "$app" ] || kill "$app" 2>/dev/null || :
  [ -z "$writer" ] || kill "$writer" 2>/dev/null || :
  [ -z "$reader" ] || kill "$reader" 2>/dev/null || :
  [ -z "$app" ] || wait "$app" 2>/dev/null || :
  [ -z "$writer" ] || wait "$writer" 2>/dev/null || :
  [ -z "$reader" ] || wait "$reader" 2>/dev/null || :
  cat "$work/stderr" >&2
  rm -rf "$work"
  exit "$status"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
: >"$work/stderr"
mkfifo "$work/in" "$work/out"
cat "$work/out" &
reader=$!
# Explicit stdin redirection keeps the SSH channel for the background copier.
{ cat; printf '%s\n' '{"id":18446744073709551615,"op":"quit"}'; } <&0 >"$work/in" &
writer=$!
open -n -W -i "$work/in" -o "$work/out" --stderr "$work/stderr" \
  --env WATANE_DRIVER_SCREEN=hidden "$HOME/Bob.app" \
  --args --home "$HOME/$home_name" --port 0 &
app=$!
if ! wait "$app"; then
  printf '%s\n' 'bob.sh: command failed: open -n -W Bob.app' >&2
  exit 1
fi
app=
kill "$writer" 2>/dev/null || :
wait "$writer" 2>/dev/null || :
writer=
wait "$reader"
reader=
BOB
check copy "$work/Bob.app" "$work/bob.sh" "admin@$ip:"
host_ip=$(remote_plain "route -n get default | awk '/gateway:/ {print \$2}'") || fail 'route -n get default (ssh -T)'
# These addresses enter URLs and a shell command, so accept IPv4 text only.
case $host_ip in ''|*[!0-9.]*) fail "route -n get default: invalid gateway $host_ip" ;; esac
case $ip in ''|*[!0-9.]*) fail "tart ip: invalid IPv4 address $ip" ;; esac
# Keep Alice's initially empty home inside the trapped workspace.
check mkdir "$work/alice"
WATANE_VM_ALICE_HOME=$work/alice
# The HTTP directory may be created after the listener starts: Alice publishes
# through the app test. Stock Apple's Python is allowed by the host firewall.
PYTHONUNBUFFERED=1 /usr/bin/python3 -m http.server 0 --bind "$host_ip" \
  --directory "$WATANE_VM_ALICE_HOME/dist/site" >"$work/web.log" 2>&1 &
web=$!
listener_bound=$(seconds 30)
deadline=$(( $(date +%s) + listener_bound ))
port=
while [ -z "$port" ]; do
  whole_check
  port=$(sed -n 's/.* port \([0-9][0-9]*\) .*/\1/p' "$work/web.log" | head -n 1)
  [ -z "$port" ] || break
  if ! kill -0 "$web" 2>/dev/null || [ "$(date +%s)" -ge "$deadline" ]; then
    cat "$work/web.log" >&2
    fail '/usr/bin/python3 -m http.server: listener did not start'
  fi
  sleep 1
done
# The wrapper removes its SSH PID file on exit so the run trap never keeps
# a stale PID from an earlier, normally closed driver connection.
quote() { printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"; }
cat >"$work/bridge.sh" <<BRIDGE
#!/bin/sh
pidfile="\${SSH_ASKPASS%/*}/bridge-\$\$.pid"
bridge=
cleanup() {
  status=\$?
  trap - EXIT HUP INT TERM
  [ -z "\$bridge" ] || kill "\$bridge" 2>/dev/null || :
  [ -z "\$bridge" ] || wait "\$bridge" 2>/dev/null || :
  rm -f "\$pidfile"
  exit "\$status"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
perl -e 'alarm shift; exec @ARGV; die "exec: \$!"' "$step_bound" \\
ssh -T -o LogLevel=ERROR -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \\
  -o PreferredAuthentications=password -o PubkeyAuthentication=no \\
  -o NumberOfPasswordPrompts=1 -o ConnectTimeout=5 admin@$ip sh bob.sh "\$@" <&0 &
bridge=\$!
printf '%s\\n' "\$bridge" >"\$pidfile"
if wait "\$bridge"; then
  bridge=
else
  status=\$?
  bridge=
  [ "\$status" -ne 142 ] || printf '%s\\n' 'Bob bridge ssh timed out after $step_bound s' >&2
  exit "\$status"
fi
BRIDGE
# Use a quoted shell path because the command is consumed by sh -c in the test.
WATANE_VM_BOB="sh $(quote "$work/bridge.sh")"
WATANE_VM_SITE_BASE=http://$host_ip:$port/
export WATANE_VM_BOB WATANE_VM_ALICE_HOME WATANE_VM_SITE_BASE
deadline_args "$journey_bound" "cargo test two_machines"
perl -MPOSIX "$work/deadline.pl" "$whole_bound" "$bound" "$step" cargo test -p watane-app --features driver --test it two_machines:: -- --ignored \
  >"$work/journey.log" 2>&1 &
journey=$!
if wait "$journey"; then
  journey=
  cat "$work/journey.log"
else
  journey=
  cat "$work/journey.log" >&2
  fail 'command failed: cargo test -p watane-app --features driver --test it two_machines:: -- --ignored'
fi
# Cargo succeeds even when a filter selects zero tests. Require the fixed
# journey's passing result, so a checkout without s55-1 cannot claim J3.
grep -Eq 'test two_machines::ac1_j3_two_machines \.\.\. ok' "$work/journey.log" || \
  fail 'cargo test: two_machines::ac1_j3_two_machines did not pass'
printf 'TWO-MACHINES PASS\n'
