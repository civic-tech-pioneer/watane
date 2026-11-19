#!/bin/sh
# Clean guest and isolated-home release checks (TECHNICAL 12.3, 12.4).
set -eu
from=
mode=clean
selected=no
usage() { printf 'clean_mac.sh: usage: clean_mac.sh [--from <dir>] [--fresh-home | --part clean | --part clt]\n' >&2; exit 1; }
while [ "$#" -gt 0 ]; do
  case $1 in
    --from) [ "$#" -ge 2 ] || usage; from=$2; shift ;;
    --fresh-home) [ "$selected" = no ] || usage; selected=yes; mode=fresh ;;
    --part)
      [ "$#" -ge 2 ] && [ "$selected" = no ] || usage
      case $2 in clean|clt) ;; *) usage ;; esac
      selected=yes; mode=$2; shift ;;
    *) printf 'clean_mac.sh: unknown argument: %s\n' "$1" >&2; exit 1 ;;
  esac
  shift
done
case $mode in clean|clt|fresh) ;; *) usage ;; esac
if [ "$mode" = fresh ]; then
  # shellcheck disable=SC2016 # Expand only in the cleared child environment.
  H=$(/usr/bin/env -i HOME="$HOME" PATH=/usr/bin:/bin:/usr/sbin:/sbin TERM=dumb /bin/sh -c 'cd "$(mktemp -d)" && pwd -P')
else
  H=$(cd "$HOME" && pwd -P)
fi
# The here-document is the whole test: no inherited build variables or tools.
exec /usr/bin/env -i HOME="$H" PATH=/usr/bin:/bin:/usr/sbin:/sbin TERM=dumb /bin/sh -s -- "$from" "$mode" <<'CLEAN_TEST'
set -eu
from=$1
case $from in ''|/*) ;; *) from=$(pwd -P)/$from ;; esac
mode=$2
H=$HOME
cd "$H"
fail() { printf 'clean_mac.sh: %s\n' "$*" >&2; exit 1; }
check() { "$@" || fail "assertion: $*"; }
clean() {
  for t in cargo rustc rustup; do
    if command -v "$t" >/dev/null 2>&1; then fail "12.4: $t on PATH (command -v $t)"; fi
  done
  for d in "$H/.cargo" "$H/.rustup" "$H/Library/Caches/watane-dev" "$H/.watane" "$H/.local/bin/watane"; do
    [ ! -e "$d" ] && [ ! -L "$d" ] || fail "12.4: $d exists (test ! -e $d)"
  done
  for d in "$H"/.rustup/toolchains/*/lib/rustlib/wasm32-wasip2; do
    [ ! -e "$d" ] || fail "12.4: target exists (test ! -e $d)"
  done
  d=$H
  while :; do
    [ ! -e "$d/Cargo.toml" ] && [ ! -e "$d/.git" ] || fail "12.4: checkout at $d (test ! -e $d/Cargo.toml && test ! -e $d/.git)"
    [ "$d" != / ] || break
    d=$(dirname "$d")
  done
  if env | grep -E '^(CARGO[A-Z_]*|RUSTUP[A-Z_]*|WATANE_[A-Z_]*)='; then fail '12.4: variable set (env | grep)'; fi
}
clean
check test "$(uname -s)" = Darwin
work=$(mktemp -d)
app_pid=
cleanup() {
  [ -z "$app_pid" ] || kill "$app_pid" 2>/dev/null || :
  rm -rf "$work"
  if [ "$mode" = fresh ]; then rm -rf "$H"; fi
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
if [ -n "$from" ]; then
  from_arg=$from
  from=$(cd "$from_arg" && pwd -P) || fail "cd $from_arg"
  check test -f "$from/install.sh"
fi

# IPC::Open3 keeps script(1)'s input open until the command has consumed
# its answer and exited. No sleep guesses when /dev/tty is ready (12.1).
pty() {
  perl -MIPC::Open3 -MIO::Select -MSymbol=gensym -e '
    my ($answer, $marker, @cmd) = @ARGV;
    my ($input, $output); my $err = gensym;
    my $pid = open3($input, $output, $err,
      "/usr/bin/script", "-q", "/dev/null", @cmd);
    $SIG{ALRM} = sub { kill "TERM", $pid; die "script(1): 1800 s deadline\n"; };
    alarm 1800;
    my $select = IO::Select->new($output, $err);
    my $seen = ""; my $sent = 0;
    while ($select->count) {
      for my $fh ($select->can_read) {
        my $n = sysread($fh, my $chunk, 4096);
        die "read script(1): $!" unless defined $n;
        if (!$n) { $select->remove($fh); next; }
        $chunk =~ s/\r//g; print $chunk;
        $seen .= $chunk;
        if (!$sent && index($seen, $marker) >= 0) {
          syswrite($input, "$answer\n") == length($answer) + 1 or die "type answer: $!";
          $sent = 1;
        }
      }
    }
    waitpid($pid, 0); my $status = $?; close $input; alarm 0;
    die "question never appeared: $marker\n" unless $sent;
    exit($status & 127 ? 1 : $status >> 8);
  ' "$@"
}
install_release() {
  answer=$1
  check find "$H" -print >"$work/home-before"
  # Pass paths as arguments, never interpolate them into shell source.
  if [ -n "$from" ]; then
    set -- env "WATANE_RELEASE_BASE=file://$from" sh "$from/install.sh"
  else
    set -- sh -c 'curl -fsSL https://github.com/civic-tech-pioneer/watane/releases/latest/download/install.sh | sh'
  fi
  if [ "$answer" = none ]; then
    perl -MPOSIX -e 'POSIX::setsid() >= 0 or die "setsid: $!"; exec @ARGV; die "exec: $!"' "$@" >"$work/install.log" 2>&1 || {
      cat "$work/install.log"; fail "install (no terminal): $*";
    }
  else
    pty "$answer" '[y/N]' "$@" >"$work/install.log" 2>&1 || {
      cat "$work/install.log"; fail "install (answer $answer): $*";
    }
  fi
  cat "$work/install.log"
}
contains() { check grep -F -- "$1" "$2"; }
no_toolchain() {
  for t in cargo rustc rustup; do
    if command -v "$t" >/dev/null 2>&1; then fail "12.4: $t on PATH after install (command -v $t)"; fi
  done
  check test ! -e "$H/.cargo"
  check test ! -e "$H/.rustup"
}
home_changes() {
  check find "$H" -print >"$work/home-tree"
  while IFS= read -r path; do
    case $path in "$H"|"$H/.local"|"$H/.local/"*|"$H/Applications"|"$H/Applications/"*) continue ;; esac
    if [ "$mode" != fresh ]; then
      case $path in "$H/.watane"|"$H/.watane/"*) continue ;; esac
      relative=${path#"$H/"}
      case $relative in
        *[Ww]atane*) check grep -Fxq -- "$path" "$work/home-before"; continue ;;
      esac
      if [ "$mode" = clt ]; then
        case $path in "$H/.cargo"|"$H/.cargo/"*|"$H/.rustup"|"$H/.rustup/"*) continue ;; esac
      fi
      # find lists each new top-level folder on its own line, which fails below.
      case $relative in */*) continue ;; esac
    fi
    # A live macOS session writes its own ~/Library files (Siri, caches) during the run;
    # only a watane name there is the installer's. cargo test's INST-S-1 proves the rest hermetically.
    case $path in "$H/Library/"*[Ww]atane*) ;; "$H/Library/"*) continue ;; esac
    check grep -Fxq -- "$path" "$work/home-before"
  done <"$work/home-tree"
}
# otool is a CLT shim on vanilla macOS. Read the same LC_*_DYLIB names
# with stock Perl when CLT is absent; this release is thin arm64 only.
native_dependencies() {
  perl -e '
    my $file = shift;
    open my $fh, "<", $file or die "open $file: $!\n";
    binmode $fh;
    read($fh, my $header, 32) == 32 or die "short Mach-O header: $file\n";
    my ($magic, $cpu, $sub, $type, $count, $size) = unpack "V8", $header;
    $magic == 0xfeedfacf && $cpu == 0x100000c && $type == 2
      or die "not a thin arm64 executable: $file\n";
    my $file_size = -s $fh;
    $count > 0 && $count <= $size / 8 && $size <= 16777216 && $size <= $file_size - 32
      or die "invalid load-command table: $file\n";
    read($fh, my $table, $size) == $size or die "short load commands: $file\n";
    my $offset = 0;
    for (1 .. $count) {
      $offset + 8 <= $size or die "missing load command: $file\n";
      my ($cmd, $length) = unpack "V2", substr($table, $offset, 8);
      $length >= 8 && $length % 8 == 0 && $offset + $length <= $size
        or die "invalid load command: $file\n";
      if (grep { $cmd == $_ } (0xc, 0x80000018, 0x8000001f, 0x20, 0x80000023)) {
        $length >= 24 or die "short dylib command: $file\n";
        my $start = unpack "V", substr($table, $offset + 8, 4);
        $start >= 24 && $start < $length or die "invalid dylib name offset: $file\n";
        my $name = substr($table, $offset + $start, $length - $start);
        $name =~ s/\0.*\z//s or die "unterminated dylib name: $file\n";
        $name =~ m{\A(?:/usr/lib/|/System/)} or die "non-system dependency: $name ($file)\n";
        print "$name\n";
      }
      $offset += $length;
    }
    $offset == $size or die "extra load-command bytes: $file\n";
  ' "$1"
}
installed() {
  v=$(cat "$H/.local/share/watane/VERSION")
  # Assert the ordered INST-S-1 progress, using the release's own version.
  check awk -v v="$v" '
    $0 == "watane " v ": installing for aarch64-apple-darwin" { if (n == 0) n=1 }
    $0 == "downloading watane-" v "-aarch64-apple-darwin.tar.gz" { if (n == 1) n=2 }
    $0 == "checked: sha256 matches SHA256SUMS of release v" v { if (n == 2) n=3 }
    $0 == "watane " v " is installed." { if (n == 3) n=4 }
    END { exit n != 4 }' "$work/install.log"
  for p in watane watane-dev watane-node; do
    check test -x "$H/.local/bin/$p"
    version=$("$H/.local/bin/$p" --version) || fail "$p --version"
    check test "$version" = "$p $v"
    printf '%s\n' "$version"
  done
  home_changes
  check test -f "$H/.local/share/watane/receipt"
  check test -f "$H/.local/share/watane/install.sh"
  check test -d "$H/.local/share/watane-dev/vendor"
  plist=$H/Applications/watane.app/Contents/Info.plist
  exe=$(plutil -extract CFBundleExecutable raw -o - "$plist") || fail "plutil CFBundleExecutable $plist"
  app_exe=$H/Applications/watane.app/Contents/MacOS/$exe
  check test -x "$app_exe"
  for p in "$H/.local/bin/watane" "$H/.local/bin/watane-dev" "$H/.local/bin/watane-node" "$app_exe"; do
    if xcode-select -p >/dev/null 2>&1; then
      check otool -L "$p" >"$work/otool"
      check awk 'NR == 1 {next} NF { if ($1 !~ /^\/usr\/lib\// && $1 !~ /^\/System\//) exit 1 }' "$work/otool"
    else
      check native_dependencies "$p"
    fi
  done
  if xcode-select -p >/dev/null 2>&1; then
    if grep -i -e 'xcode-select' -e 'command line tools' "$work/install.log"; then fail 'unexpected command line tools text (xcode-select -p succeeded)'; fi
  else
    contains "watane-dev needs the macOS command line tools (a C linker and git). Run this and follow macOS's own dialog:" "$work/install.log"
    contains '  xcode-select --install' "$work/install.log"
  fi
}
doctor() {
  status=0
  "$H/.local/bin/watane-dev" doctor --json >"$work/doctor.json" || status=$?
  check test "$status" -eq "$1"
  for key in result.watane result.allowlist.vendor; do
    value=$(plutil -extract "$key" raw -o - "$work/doctor.json") || fail "plutil $key doctor.json"
    case $key in
      result.watane) check test "$value" = "$H/.local/bin/watane" ;;
      *) check test "$value" = "$H/.local/share/watane-dev/vendor" ;;
    esac
  done
}
declined() {
  if [ "$1" = none ]; then
    contains 'no terminal to ask on: the toolchain was not installed' "$work/install.log"
    if grep -F '[y/N]' "$work/install.log"; then fail 'no-terminal install asked a question (grep [y/N])'; fi
  else
    contains 'toolchain: not installed' "$work/install.log"
    check awk '/\[y\/N\]/ {n++} END {exit n != 1}' "$work/install.log"
  fi
  contains "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --no-modify-path -t wasm32-wasip2" "$work/install.log"
  no_toolchain
  doctor 3
  code=$(plutil -extract errors.0.code raw -o - "$work/doctor.json") || fail 'plutil errors.0.code doctor.json'
  fix=$(plutil -extract errors.0.fix raw -o - "$work/doctor.json") || fail 'plutil errors.0.fix doctor.json'
  check test "$code" = env.toolchain
  check test "$fix" = 'Install rustup with your package manager, then run: rustup default stable'
  no_toolchain
}
reset_home() {
  # Only the paths fixed in 12.3; this runs in the disposable guest only.
  check rm -rf "$H/.local" "$H/Applications/watane.app" "$H/.cargo" "$H/.rustup" "$H/.watane" "$H/Library/Caches/watane-dev"
  clean
}
uninstalled() {
  for p in "$H/.local/bin/watane" "$H/.local/bin/watane-dev" "$H/.local/bin/watane-node" "$H/.local/share/watane" "$H/.local/share/watane-dev" "$H/Applications/watane.app"; do
    check test ! -e "$p"
  done
}

case $mode in
  fresh|clean)
    if [ "$mode" = clean ]; then
      if xcode-select -p >/dev/null 2>&1; then fail 'clean part requires no CLT (xcode-select -p)'; fi
    fi
    install_release none
    installed
    check test ! -e "$H/.watane"
    declined none
    if [ "$mode" = clean ]; then
      if xattr -p com.apple.quarantine "$H/Applications/watane.app"; then fail 'quarantine remains (xattr -p com.apple.quarantine watane.app)'; fi
      check open "$H/Applications/watane.app"
      sleep 5
      app_pid=$(pgrep -f "$app_exe") || fail "the app did not start: see the screen (pgrep -f $app_exe)"
      check kill "$app_pid"
      app_pid=
      attempts=0
      while pgrep -f "$app_exe" >/dev/null; do
        attempts=$((attempts + 1))
        [ "$attempts" -le 10 ] || fail "app did not quit (pgrep -f $app_exe)"
        sleep 1
      done
      reset_home
      for answer in n ''; do
        install_release "$answer"
        installed
        declined "$answer"
        if [ "$answer" = n ]; then reset_home; fi
      done
      check mkdir -p "$H/.watane"
      check touch "$H/.watane/marker"
      # Preserve a copy because uninstall removes its installed script.
      check cp "$H/.local/share/watane/install.sh" "$work/uninstall.sh"
      check pty n 'keep it: ' sh "$work/uninstall.sh" --uninstall >"$work/uninstall.log"
      contains 'kept: ~/.watane (your home: identity, mods, channels)' "$work/uninstall.log"
      uninstalled
      check test -f "$H/.watane/marker"
      install_release none
      check pty delete 'keep it: ' sh "$work/uninstall.sh" --uninstall
      uninstalled
      check test ! -e "$H/.watane"
    fi ;;
  clt)
    check xcode-select -p
    install_release y
    installed
    contains 'toolchain: installed' "$work/install.log"
    check awk '/\[y\/N\]/ {questions++} /^toolchain: installed$/ {installed=NR} /^watane .* is installed\.$/ {done=NR} END {exit !(questions == 1 && installed && done > installed)}' "$work/install.log"
    check test -x "$H/.cargo/bin/cargo"
    check "$H/.cargo/bin/rustup" target list --installed >"$work/targets"
    contains wasm32-wasip2 "$work/targets"
    doctor 0
    target=$(plutil -extract result.toolchain.wasm32_wasip2 raw -o - "$work/doctor.json") || fail 'plutil wasm32_wasip2 doctor.json'
    check test "$target" = true
    PATH=$H/.local/bin:$PATH
    export PATH
    check mkdir "$work/new"
    check cd "$work/new"
    check watane-dev new reading-list
    check test -f reading-list/mod.toml
    check cd reading-list
    check watane-dev test ;;
esac
printf 'CLEAN-MAC PASS\n'
CLEAN_TEST
