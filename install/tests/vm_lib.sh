#!/bin/sh
# Sourced Tart lifecycle and SSH helpers (TECHNICAL 12.3, 12.6).
# The caller supplies driver and check; sourcing allocates the run workspace.
: "${driver:?vm_lib.sh: the sourcing script sets driver}"
work=$(mktemp -d)
vm=
runner=
cleanup() {
  status=$?
  trap - EXIT HUP INT TERM
  if [ -n "${journey:-}" ]; then
    kill "$journey" 2>/dev/null || :
    wait "$journey" 2>/dev/null || :
  fi
  for bridge_file in "$work"/bridge-*.pid; do
    [ -f "$bridge_file" ] || continue
    bridge_pid=$(cat "$bridge_file")
    kill "$bridge_pid" 2>/dev/null || :
  done
  if [ -n "${web:-}" ]; then
    kill "$web" 2>/dev/null || :
    wait "$web" 2>/dev/null || :
  fi
  if [ -n "$vm" ]; then
    tart stop "$vm" || :
    [ -z "$runner" ] || kill "$runner" 2>/dev/null || :
    [ -z "$runner" ] || wait "$runner" 2>/dev/null || :
    if ! tart delete "$vm"; then
      printf '%s: cleanup failed: tart delete %s\n' "$driver" "$vm" >&2
      status=1
    fi
  fi
  rm -rf "$work"
  exit "$status"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
printf '#!/bin/sh\nprintf "admin\\n"\n' >"$work/askpass"
check chmod 700 "$work/askpass"
SSH_ASKPASS=$work/askpass
SSH_ASKPASS_REQUIRE=force
DISPLAY=watane-test
export SSH_ASKPASS SSH_ASKPASS_REQUIRE DISPLAY
remote() {
  ssh -tt -o LogLevel=ERROR -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
    -o PreferredAuthentications=password -o PubkeyAuthentication=no \
    -o NumberOfPasswordPrompts=1 -o ConnectTimeout=5 \
    "admin@$ip" "$@" </dev/null
}
copy() {
  scp -r -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
    -o PreferredAuthentications=password -o PubkeyAuthentication=no \
    -o NumberOfPasswordPrompts=1 -o ConnectTimeout=5 "$@" </dev/null
}
boot() {
  # Own the name only once this run created it; never delete another VM.
  check tart clone "$1" "watane-run-$$"
  vm=watane-run-$$
  tart run --no-graphics "$vm" >"$work/tart.log" 2>&1 &
  runner=$!
  deadline=$(( $(date +%s) + 600 ))
  ip=$(tart ip "$vm" --wait 600) || { cat "$work/tart.log"; fail "tart ip $vm --wait 600"; }
  while :; do
    remaining=$(( deadline - $(date +%s) ))
    [ "$remaining" -gt 0 ] || fail '600 s deadline waiting for ssh as admin'
    # Bound the entire authentication attempt, including askpass.
    if perl -e 'alarm shift; exec @ARGV; die "exec: $!"' "$remaining" \
      ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
      -o PreferredAuthentications=password -o PubkeyAuthentication=no \
      -o NumberOfPasswordPrompts=1 -o ConnectTimeout=5 "admin@$ip" true \
      </dev/null >"$work/ssh.log" 2>&1; then break; fi
    sleep 1
  done
}
stop_run() {
  check tart stop "$vm"
  wait "$runner" || :
  runner=
}
delete_run() {
  check tart delete "$vm"
  vm=
}
