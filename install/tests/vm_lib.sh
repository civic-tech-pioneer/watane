#!/bin/sh
# Sourced Tart lifecycle and SSH helpers (TECHNICAL 12.3, 12.6).
# The caller supplies driver and fail; sourcing allocates the run workspace.
: "${driver:?vm_lib.sh: the sourcing script sets driver}"
# Test helpers alone accept a divisor; every effective bound is at least 1 s.
vm_scale=${WATANE_VM_TEST_SCALE:-1}
case $vm_scale in ''|*[!0-9]*|0*) fail 'invalid WATANE_VM_TEST_SCALE' ;; esac
seconds() {
  scaled=$(( $1 / vm_scale ))
  [ "$scaled" -gt 0 ] || scaled=1
  printf '%s\n' "$scaled"
}
# Drivers opt in before sourcing; the clean-Mac driver keeps its boot deadline only.
whole_bound=
whole_end=
step_bound=
if [ -n "${vm_whole_limit:-}" ]; then
  whole_bound=$(seconds "$vm_whole_limit")
  whole_end=$(( $(date +%s) + whole_bound ))
fi
if [ -n "${vm_step_limit:-}" ]; then
  step_bound=$(seconds "$vm_step_limit")
fi
# Keep alarm in a supervisor so it can name the cause and reap the command.
# A separate command group also bounds descendants of cargo/ssh on timeout.
run_deadline() {
  if [ -n "$1" ]; then
    perl -MPOSIX "$work/deadline.pl" "$whole_bound" "$@"
  else
    shift 2
    "$@"
  fi
}
whole_check() {
  if [ -n "$whole_bound" ]; then
    [ "$(date +%s)" -lt "$whole_end" ] || fail "whole script timed out after $whole_bound s"
  fi
}
check() { whole_check; "$@" || fail "command failed: $*"; }
deadline_args() {
  whole_check
  bound=$1; step=$2
  [ -n "$whole_bound" ] || return 0
  left=$(( whole_end - $(date +%s) ))
  [ "$left" -gt 0 ] || fail "whole script timed out after $whole_bound s"
  if [ -z "$bound" ] || [ "$left" -le "$bound" ]; then
    bound=$left
    step="whole script"
  fi
}
bounded() {
  deadline_args "$1" "$2"
  shift 2
  run_deadline "$bound" "$step" "$@"
}
work=$(mktemp -d)
vm=
runner=
cleanup() {
  status=$?
  set +e
  trap - EXIT HUP INT TERM
  if [ -n "${journey:-}" ]; then
    kill "$journey" 2>/dev/null || :
    wait "$journey" 2>/dev/null || :
  fi
  for bridge_file in "$work"/bridge-*.pid; do
    [ -f "$bridge_file" ] || continue
    bridge_pid=$(cat "$bridge_file" 2>/dev/null) || continue
    case $bridge_pid in ''|0|*[!0-9]*) continue ;; esac
    kill "$bridge_pid" 2>/dev/null || :
  done
  if [ -n "${web:-}" ]; then
    kill "$web" 2>/dev/null || :
    wait "$web" 2>/dev/null || :
  fi
  if [ -n "$vm" ]; then
    run_deadline "$step_bound" "cleanup tart stop" tart stop "$vm" || :
    [ -z "$runner" ] || kill "$runner" 2>/dev/null || :
    [ -z "$runner" ] || wait "$runner" 2>/dev/null || :
    if ! run_deadline "$step_bound" "cleanup tart delete" tart delete "$vm"; then
      printf '%s: cleanup failed: tart delete %s\n' "$driver" "$vm" >&2
      status=1
    fi
  fi
  rm -rf "$work"
  exit "$status"
}
# Preserve diagnostics while a guest call redirects stderr into a temporary log.
exec 3>&2
trap cleanup EXIT
trap 'printf "%s: interrupted\n" "$driver" >&3; exit 1' HUP INT TERM
cat >"$work/deadline.pl" <<'PERL'
my ($whole, $bound, $step, @cmd) = @ARGV;
my $pid = fork(); defined $pid or die "fork: $!";
if (!$pid) { POSIX::setsid(); exec @cmd; die "exec: $!"; }
my $stop = sub {
  kill "TERM", -$pid; kill "TERM", $pid;
  select undef, undef, undef, 1;
  kill "KILL", -$pid; kill "KILL", $pid;
  waitpid $pid, 0;
};
$SIG{ALRM} = sub {
  my $shown = $step eq "whole script" ? $whole : $bound;
  print STDERR "$step timed out after $shown s\n";
  $stop->(); exit 124;
};
$SIG{$_} = sub { $stop->(); exit 1; } for qw(HUP INT TERM);
alarm $bound;
waitpid $pid, 0;
my $status = $?;
alarm 0;
exit(($status & 127) ? 128 + ($status & 127) : $status >> 8);
PERL
printf '#!/bin/sh\nprintf "admin\\n"\n' >"$work/askpass"
check chmod 700 "$work/askpass"
SSH_ASKPASS=$work/askpass
SSH_ASKPASS_REQUIRE=force
DISPLAY=watane-test
export SSH_ASKPASS SSH_ASKPASS_REQUIRE DISPLAY
remote() {
  bounded "$step_bound" remote ssh -tt -o LogLevel=ERROR -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
    -o PreferredAuthentications=password -o PubkeyAuthentication=no \
    -o NumberOfPasswordPrompts=1 -o ConnectTimeout=5 \
    "admin@$ip" "$@" </dev/null
}
copy() {
  bounded "$step_bound" copy scp -r -o LogLevel=ERROR -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
    -o PreferredAuthentications=password -o PubkeyAuthentication=no \
    -o NumberOfPasswordPrompts=1 -o ConnectTimeout=5 "$@" </dev/null
}
boot() {
  # Own the name only once this run created it; never delete another VM.
  check bounded "$step_bound" "tart clone" tart clone "$1" "watane-run-$$"
  vm=watane-run-$$
  if [ -n "$whole_bound" ]; then
    deadline_args "$whole_bound" "tart run"
    perl -MPOSIX "$work/deadline.pl" "$whole_bound" "$bound" "$step" tart run --no-graphics "$vm" >"$work/tart.log" 2>&1 &
  else
    tart run --no-graphics "$vm" >"$work/tart.log" 2>&1 &
  fi
  runner=$!
  boot_bound=$(seconds 600)
  deadline=$(( $(date +%s) + boot_bound ))
  ip=$(bounded "$boot_bound" "tart ip" tart ip "$vm" --wait "$boot_bound") || { cat "$work/tart.log"; fail "tart ip $vm --wait 600"; }
  while :; do
    remaining=$(( deadline - $(date +%s) ))
    [ "$remaining" -gt 0 ] || fail "boot ssh timed out after $boot_bound s"
    # Bound the entire authentication attempt, including askpass.
    if bounded "$remaining" "boot ssh" \
      ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
      -o PreferredAuthentications=password -o PubkeyAuthentication=no \
      -o NumberOfPasswordPrompts=1 -o ConnectTimeout=5 "admin@$ip" true \
      </dev/null >"$work/ssh.log" 2>&1; then break; fi
    sleep 1
  done
}
stop_run() {
  check bounded "$step_bound" "tart stop" tart stop "$vm"
  if [ -n "$whole_bound" ]; then kill "$runner" 2>/dev/null || :; fi
  wait "$runner" || :
  runner=
}
delete_run() {
  check bounded "$step_bound" "tart delete" tart delete "$vm"
  vm=
}
