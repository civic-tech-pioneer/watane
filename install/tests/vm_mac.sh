#!/bin/sh
# Tart host driver; never shares the host checkout (TECHNICAL 12.3).
# shellcheck disable=SC2154,SC2034 # Variables are shared with sourced vm_lib.sh.
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
driver=vm_mac.sh
# shellcheck disable=SC1091 # vm_lib.sh is linted separately.
# shellcheck source=vm_lib.sh
. "$here/vm_lib.sh"
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
