#!/bin/sh
# Tart host driver; never shares the host checkout (TECHNICAL 12.3).
set -eu
if ! command -v tart >/dev/null 2>&1; then
  printf '%s\n' 'vm_mac.sh: tart is not installed (see install/TECHNICAL.md 12.3)' >&2
  exit 2
fi
fail() { printf 'vm_mac.sh: %s\n' "$*" >&2; exit 1; }
check() { "$@" || fail "command failed: $*"; }
prepare=no
from=
while [ "$#" -gt 0 ]; do
  case $1 in
    --prepare) prepare=yes ;;
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
work=$(mktemp -d)
vm=
runner=
cleanup() {
  status=$?
  trap - EXIT HUP INT TERM
  if [ -n "$vm" ]; then
    tart stop "$vm" || :
    [ -z "$runner" ] || kill "$runner" 2>/dev/null || :
    [ -z "$runner" ] || wait "$runner" 2>/dev/null || :
    if ! tart delete "$vm"; then
      printf 'vm_mac.sh: cleanup failed: tart delete %s\n' "$vm" >&2
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
if [ "$prepare" = yes ]; then
  base=ghcr.io/cirruslabs/macos-golden-gate-vanilla:latest
  tart get watane-clean >/dev/null 2>&1 || check tart clone "$base" watane-clean
  boot "$base"
  # Only image preparation uses the guest's passwordless sudo; the
  # installer and clean_mac.sh never do. Derive the current CLT label.
  # shellcheck disable=SC2016 # These variables expand in the guest shell.
  check remote 'set -eu
    sudo touch /tmp/.com.apple.dt.CommandLineTools.installondemand.in-progress
    trap "sudo rm -f /tmp/.com.apple.dt.CommandLineTools.installondemand.in-progress" EXIT
    softwareupdate --list > /tmp/watane-softwareupdate
    label=$(sed -n "s/^[[:space:]]*\\* Label: \\(Command Line Tools.*\\)/\\1/p" /tmp/watane-softwareupdate | tail -n 1)
    [ -n "$label" ] || { cat /tmp/watane-softwareupdate; exit 1; }
    sudo softwareupdate --install "$label"
    xcode-select -p'
  stop_run
  check tart clone "$vm" watane-clean-clt
  delete_run
fi
for part in clean clt; do
  case $part in clean) image=watane-clean ;; clt) image=watane-clean-clt ;; esac
  boot "$image"
  check copy "$here/clean_mac.sh" "admin@$ip:clean_mac.sh"
  if [ -n "$from" ]; then
    check remote 'mkdir -p /Users/admin/watane-release'
    check copy "$from/." "admin@$ip:/Users/admin/watane-release/"
    set -- --from /Users/admin/watane-release
  else
    set --
  fi
  # Guest paths and part names here are constants, not host shell inputs.
  if ! remote "sh clean_mac.sh $* --part $part" >"$work/guest.log" 2>&1; then
    cat "$work/guest.log"
    fail "guest $part failed (ssh -tt sh clean_mac.sh)"
  fi
  cat "$work/guest.log"
  last=$(tr -d '\r' <"$work/guest.log" | awk 'NF {last=$0} END {print last}')
  [ "$last" = 'CLEAN-MAC PASS' ] || fail "guest $part did not print CLEAN-MAC PASS (ssh -tt)"
  stop_run
  delete_run
done
printf 'VM PASS\n'
