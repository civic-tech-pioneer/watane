#!/bin/sh
# install.sh: installs, upgrades, reports and removes watane (install/TECHNICAL 3).
# POSIX sh, only the tools of TECHNICAL 3.3. The whole body is the function `main`,
# called on the last line, so that a download cut off half way runs nothing.
set -eu
umask 022

DEFAULT_RELEASE="@RELEASE@"
NL='
'
TARGET=aarch64-apple-darwin
RELS="bin/watane bin/watane-dev bin/watane-node share/watane-dev/vendor share/watane/install.sh share/watane/VERSION Applications/watane.app"

TMP=""
SWAP_ACTIVE=0
SWAP_PID=0
RES=""
HOME_R=""
R_OK=0
R_VERSION=""
R_TARGET=""
R_LINES=""
J_OLD=""
J_NEW=""
J_MOVED=""
JOURNALS=""
TC_STATE=""
TC_OK=0
RUSTUP=""
UPGRADED=0
SIGNAL=0

usage() {
  printf '%s\n' \
    'usage: install.sh [--release <version>] [--no-toolchain]' \
    '       install.sh --uninstall' \
    '       install.sh --status' \
    '       install.sh --help | -h'
}

say() { printf '%s\n' "$*"; }
warn() { printf 'watane install: %s\n' "$*" >&2; }

# die <code> <cause> [<what to do> ...]
die() {
  check_signal "$?"
  die_code=$1
  shift
  printf 'watane install: %s\n' "$1" >&2
  shift
  for die_l in "$@"; do printf '%s\n' "$die_l" >&2; done
  exit "$die_code"
}

bad_flag() {
  check_signal
  printf 'watane install: unknown or conflicting flag %s\n' "$1" >&2
  usage >&2
  exit 1
}

# tilde <path>: the path with $HOME shown as ~
tilde() {
  case $1 in
    "$HOME") printf '~' ;;
    "$HOME"/*) printf '~%s' "${1#"$HOME"}" ;;
    *) printf '%s' "$1" ;;
  esac
}

# tilde_r <resolved path>: as tilde, against the resolved home
tilde_r() {
  case $1 in
    "$HOME_R") printf '~' ;;
    "$HOME_R"/*) printf '~%s' "${1#"$HOME_R"}" ;;
    *) printf '%s' "$1" ;;
  esac
}

exists() { [ -e "$1" ] || [ -L "$1" ]; }

# in_list <newline separated list> <item>
in_list() {
  case "$NL$1$NL" in
    *"$NL$2$NL"*) return 0 ;;
  esac
  return 1
}

# add_item <list> <item>: prints the list with the item added
add_item() {
  if [ -z "$1" ]; then printf '%s' "$2"; else printf '%s\n%s' "$1" "$2"; fi
}

# plain_path <path>: absolute, no newline, no //, no trailing /, no . or .. component
plain_path() {
  case $1 in
    '' | *"$NL"* | *//* | */ | */./* | */../* | */. | */..) return 1 ;;
    /*) return 0 ;;
  esac
  return 1
}

# resolve_dir <folder>: RES is the physical path of the folder, or of where it would be
# made, found through its nearest existing parent. A link never counts as missing.
# Returns 1 when a link cannot be entered; RES is then its target.
resolve_dir() {
  rd_d=$1
  rd_rest=""
  while [ ! -d "$rd_d" ] && [ ! -L "$rd_d" ]; do
    rd_rest="/${rd_d##*/}$rd_rest"
    rd_d=${rd_d%/*}
    [ -n "$rd_d" ] || rd_d=/
  done
  if rd_r=$(cd "$rd_d" 2>/dev/null && pwd -P); then
    check_signal "$?"
    if [ "$rd_r" = / ]; then RES=$rd_rest; else RES=$rd_r$rd_rest; fi
    [ -n "$RES" ] || RES=/
    return 0
  else
    check_signal "$?"
  fi
  rd_t=$(readlink "$rd_d" 2>/dev/null) || { check_signal "$?"; rd_t=$rd_d; }
  check_signal "$?"
  RES=$rd_t$rd_rest
  return 1
}

in_home() {
  case $1 in
    "$HOME_R" | "$HOME_R"/*) return 0 ;;
  esac
  return 1
}

# below_home <resolved path>: strictly below the resolved home, never the home itself
below_home() {
  case $1 in
    "$HOME_R"/*) return 0 ;;
  esac
  return 1
}

# allowed <path>: the rule of TECHNICAL 8.5, the only paths a removal may touch
allowed() {
  plain_path "$1" || return 1
  case $1 in
    "$HOME/Applications/watane.app") al_root=$HOME/Applications ;;
    "$HOME"/.local/bin/watane*)
      al_n=${1#"$HOME"/.local/bin/}
      case $al_n in */* | *.mine) return 1 ;; esac
      al_root=$HOME/.local/bin
      ;;
    "$HOME"/.local/share/watane/?*) al_root=$HOME/.local/share/watane ;;
    "$HOME"/.local/share/watane-dev/?*) al_root=$HOME/.local/share/watane-dev ;;
    *) return 1 ;;
  esac
  resolve_dir "$al_root" || return 1
  below_home "$RES" || return 1
  al_rr=$RES
  resolve_dir "${1%/*}" || return 1
  case $RES in
    "$al_rr" | "$al_rr"/*) return 0 ;;
  esac
  return 1
}

# dest_of <rel>: DEST is where the entry goes
dest_of() {
  case $1 in
    Applications/*) DEST=$HOME/$1 ;;
    *) DEST=$PREFIX/$1 ;;
  esac
}

set_entries() {
  PREFIX=$HOME/.local
  SHAREW=$PREFIX/share/watane
  RECEIPT=$SHAREW/receipt
  ENTRIES=""
  for se_r in $RELS; do
    dest_of "$se_r"
    ENTRIES=$(add_item "$ENTRIES" "$DEST")
  done
  ENTRIES_ALL=$(add_item "$ENTRIES" "$RECEIPT")
}

valid_entry() { in_list "$ENTRIES_ALL" "$1" && allowed "$1"; }

# read_receipt <file>: R_OK, R_VERSION, R_TARGET, R_LINES (the `path` lines as they are)
read_receipt() {
  R_OK=0
  R_VERSION=""
  R_TARGET=""
  R_LINES=""
  [ -f "$1" ] && [ ! -L "$1" ] || return 0
  rr_first=1
  while IFS= read -r rr_l; do
    if [ "$rr_first" = 1 ]; then
      rr_first=0
      [ "$rr_l" = 'watane-receipt 1' ] || return 0
      R_OK=1
      continue
    fi
    case $rr_l in
      'version '*) R_VERSION=${rr_l#version } ;;
      'target '*) R_TARGET=${rr_l#target } ;;
      'path '*) R_LINES=$(add_item "$R_LINES" "$rr_l") ;;
    esac
  done <"$1"
  return 0
}

# read_journal <file>: J_OLD, J_NEW, J_MOVED hold the lines that count (TECHNICAL 3.6.1)
read_journal() {
  J_OLD=""
  J_NEW=""
  J_MOVED=""
  [ -f "$1" ] && [ ! -L "$1" ] || return 0
  while IFS= read -r rj_l; do
    case $rj_l in
      'old '*) rj_p=${rj_l#old } rj_k=old ;;
      'new '*) rj_p=${rj_l#new } rj_k=new ;;
      'moved '*) rj_p=${rj_l#moved } rj_k=moved ;;
      *) continue ;;
    esac
    if valid_entry "$rj_p"; then
      case $rj_k in
        old) J_OLD=$(add_item "$J_OLD" "$rj_p") ;;
        new) J_NEW=$(add_item "$J_NEW" "$rj_p") ;;
        moved) J_MOVED=$(add_item "$J_MOVED" "$rj_p") ;;
      esac
    fi
  done <"$1"
  return 0
}

# pid_gone <n>: the process is dead, or is this script's own pid
pid_gone() {
  if [ "$1" = "$$" ]; then return 0; fi
  if kill -0 "$1" 2>/dev/null; then
    check_signal "$?"
    return 1
  else
    check_signal "$?"
  fi
  return 0
}

# find_journals: JOURNALS lists the pids of the journal files that count
find_journals() {
  JOURNALS=""
  for fj_f in "$SHAREW"/journal.*; do
    exists "$fj_f" || continue
    fj_n=${fj_f##*/journal.}
    case $fj_n in '' | *[!0-9]*) continue ;; esac
    [ -f "$fj_f" ] && [ ! -L "$fj_f" ] || continue
    JOURNALS=$(add_item "$JOURNALS" "$fj_n")
  done
  return 0
}

check_no_live_install() {
  find_journals
  while IFS= read -r cn_n; do
    [ -n "$cn_n" ] || continue
    pid_gone "$cn_n" && continue
    die 5 "another install is running (pid $cn_n)" \
      "If no install is running, that number now belongs to another program; remove $SHAREW/journal.$cn_n and run this again."
  done <<EOF
$JOURNALS
EOF
  return 0
}

# rm_path <path>: rm -rf with a warning
rm_path() {
  rm -rf "$1" || { check_signal "$?"; warn "could not remove $1"; return 1; }
  return 0
}

# undo_swap <pid>: put the old files back from the journal of that pid (3.6 step 4)
undo_swap() {
  (
    # The parent retains the signal code; checks inside the undo must let it finish.
    SIGNAL=0
    set +e
    ub_j=$SHAREW/journal.$1
    if [ -f "$ub_j" ] && [ ! -L "$ub_j" ]; then
      # the receipt moved into place: the commit happened, there is nothing to undo
      if ! exists "$RECEIPT.new.$1"; then finish_commit "$1" || exit "$?"; exit 0; fi
      read_journal "$ub_j"
      while IFS= read -r ub_p; do
        [ -n "$ub_p" ] || continue
        exists "$ub_p.old.$1" || continue
        if [ "$ub_p" != "$RECEIPT" ]; then rm_path "$ub_p" || exit "$?"; fi
        mv "$ub_p.old.$1" "$ub_p" || exit "$?"
      done <<EOF
$J_OLD
EOF
      while IFS= read -r ub_p; do
        [ -n "$ub_p" ] || continue
        in_list "$J_MOVED" "$ub_p" || continue
        exists "$ub_p.new.$1" && continue
        # Move back to staging before deleting: a repeated undo then has proof
        # that this fresh entry was already taken out of the installed paths.
        if exists "$ub_p"; then
          mv "$ub_p" "$ub_p.new.$1" || exit "$?"
        else
          # A removed parent leaves no entry at this path to protect on retry.
          exists "${ub_p%/*}" || continue
          mkdir "$ub_p.new.$1" || exit "$?"
        fi
      done <<EOF
$J_NEW
EOF
    fi
    # Retire the journal while every staging name still proves what did not move.
    # A retry after this point cleans staging only; it cannot claim a person's file.
    rm -f "$SHAREW/journal.$1" || exit "$?"
    while IFS= read -r ub_p; do
      allowed "$ub_p" || continue
      if exists "$ub_p.new.$1"; then rm_path "$ub_p.new.$1" || exit "$?"; fi
    done <<EOF
$ENTRIES_ALL
EOF
    if exists "$RECEIPT.old.$1"; then rm_path "$RECEIPT.old.$1" || exit "$?"; fi
    exit 0
  )
}

# finish_commit <pid>: step 5 of 3.6 for the swap of that pid
finish_commit() {
  fc_old=$SHAREW/receipt.old.$1
  read_receipt "$RECEIPT"
  if [ "$R_OK" = 1 ]; then
    fc_cur=$R_LINES
    read_receipt "$fc_old"
    if [ "$R_OK" = 1 ]; then
      while IFS= read -r fc_l; do
        case $fc_l in 'path '*) ;; *) continue ;; esac
        fc_p=${fc_l#path }
        if in_list "$fc_cur" "$fc_l"; then continue; fi
        allowed "$fc_p" || continue
        if exists "$fc_p"; then rm_path "$fc_p" || return; fi
      done <<EOF
$R_LINES
EOF
    fi
  fi
  while IFS= read -r fc_p; do
    allowed "$fc_p" || continue
    if exists "$fc_p.old.$1"; then rm_path "$fc_p.old.$1" || return; fi
  done <<EOF
$ENTRIES
EOF
  if exists "$fc_old"; then rm_path "$fc_old" || return; fi
  rm -f "$SHAREW/journal.$1" || { check_signal "$?"; return 1; }
  return 0
}

# clean_stale: the `.new.<n>` and `.old.<n>` names of dead processes (3.8)
clean_stale() {
  while IFS= read -r cs_e; do
    allowed "$cs_e" || continue
    for cs_k in new old; do
      for cs_f in "$cs_e.$cs_k".*; do
        exists "$cs_f" || continue
        cs_n=${cs_f#"$cs_e.$cs_k."}
        case $cs_n in '' | *[!0-9]*) continue ;; esac
        pid_gone "$cs_n" || continue
        rm_path "$cs_f" || return
      done
    done
  done <<EOF
$ENTRIES_ALL
EOF
  return 0
}

# recover: undo or finish what a dead process left (3.6 step 0), then clean its names
recover() {
  # the journals and receipt.* lie beside the receipt: none is used through a link out (8.5)
  if ! allowed "$RECEIPT"; then
    clean_stale || die 5 "could not recover stale install files" "Recovery stopped; fix the filesystem error and run this again."
    return 0
  fi
  find_journals
  while IFS= read -r rc_n; do
    [ -n "$rc_n" ] || continue
    pid_gone "$rc_n" || continue
    if exists "$RECEIPT.new.$rc_n"; then
      undo_swap "$rc_n" || { check_signal "$?"; die 5 "could not recover interrupted install (pid $rc_n)" "Recovery stopped; fix the filesystem error and run this again."; }
    else
      finish_commit "$rc_n" || die 5 "could not recover interrupted install (pid $rc_n)" "Recovery stopped; fix the filesystem error and run this again."
    fi
  done <<EOF
$JOURNALS
EOF
  clean_stale || die 5 "could not recover stale install files" "Recovery stopped; fix the filesystem error and run this again."
  return 0
}

# rollback: undo the swap in progress (3.8). It counts as undone only when undo_swap ended well: a
# subshell that macOS sh forks while a trapped signal is pending dies of that signal, and the
# handler that runs next must then undo it again.
rollback() {
  if [ "$SWAP_ACTIVE" = 1 ]; then
    undo_swap "$SWAP_PID" || return
    SWAP_ACTIVE=0
  fi
  return 0
}

on_signal() {
  SIGNAL=$1
  trap '' HUP INT TERM PIPE
  rollback || :
  exit "$1"
}

# A subshell gives bash 3.2 a pending-trap dispatch point before any diagnosis.
# It can itself die of the pending signal; the parent must still check its flag.
# Preserve a signalled program's status too: bash can lose the parent's INT
# when the group signal reaches the child first.
check_signal() {
  sig_status=${1:-0}
  ( : ) || true
  if [ "$SIGNAL" = 0 ]; then
    case $sig_status in 129 | 130 | 141 | 143) SIGNAL=$sig_status ;; esac
  fi
  # Ignore further signals as on_signal does: a trap still pending would otherwise run in the
  # exit trap and end it before it removes the temporary folder.
  if [ "$SIGNAL" != 0 ]; then
    trap '' HUP INT TERM PIPE
    exit "$SIGNAL"
  fi
  return 0
}

cleanup() {
  cl_rc=$?
  trap - EXIT
  ( : ) || true
  rollback || { check_signal "$?"; }
  if [ -n "$TMP" ]; then rm -rf "$TMP" || { check_signal "$?"; }; fi
  if [ "$SIGNAL" != 0 ]; then cl_rc=$SIGNAL; fi
  exit "$cl_rc"
}

check_version() {
  # Spelled-out classes: bash 3.2's ranges follow the locale and would admit e.g. é.
  cv_core=${1%%-*}
  case $1 in
    *-*)
      cv_suffix=${1#*-}
      case $cv_suffix in
        '' | *[!0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz.]* | .* | *. | *..*)
          die 1 "not a version: $1" "a version looks like 0.1.0" ;;
      esac
      ;;
  esac
  for cv_i in 1 2 3; do
    cv_part=${cv_core%%.*}
    case $cv_part in
      '' | *[!0123456789]*) die 1 "not a version: $1" "a version looks like 0.1.0" ;;
    esac
    if [ "$cv_i" != 3 ]; then
      [ "$cv_core" != "$cv_part" ] || die 1 "not a version: $1" "a version looks like 0.1.0"
      cv_core=${cv_core#*.}
    else
      [ "$cv_core" = "$cv_part" ] || die 1 "not a version: $1" "a version looks like 0.1.0"
    fi
  done
  return 0
}

check_tools() {
  for ct_t in $1; do
    command -v "$ct_t" >/dev/null 2>&1 ||
      die 2 "the tool $ct_t is missing" "watane install uses only the tools that stock macOS ships; $ct_t was not found on your PATH."
  done
  return 0
}

unsupported() {
  die 2 "unsupported platform: $1" "watane installs on macOS 14.0 or later, on Apple silicon."
}

check_platform() {
  cp_s=$(uname -s) || { cp_rc=$?; check_signal "$cp_rc"; exit "$cp_rc"; }
  check_signal "$?"
  cp_m=$(uname -m) || { cp_rc=$?; check_signal "$cp_rc"; exit "$cp_rc"; }
  check_signal "$?"
  [ "$cp_s" = Darwin ] || unsupported "$cp_s $cp_m"
  check_tools "sw_vers sysctl xattr xcode-select"
  case $cp_m in
    arm64) ;;
    x86_64)
      cp_a=$(sysctl -n hw.optional.arm64 2>/dev/null) || { check_signal "$?"; cp_a=0; }
      check_signal "$?"
      [ "$cp_a" = 1 ] || unsupported "$cp_s $cp_m"
      ;;
    *) unsupported "$cp_s $cp_m" ;;
  esac
  cp_v=$(sw_vers -productVersion) || { cp_rc=$?; check_signal "$cp_rc"; exit "$cp_rc"; }
  check_signal "$?"
  cp_major=${cp_v%%.*}
  case $cp_major in
    '' | *[!0-9]*) unsupported "macOS $cp_v (needs 14.0 or later)" ;;
  esac
  [ "$cp_major" -ge 14 ] || unsupported "macOS $cp_v (needs 14.0 or later)"
  return 0
}

check_home() {
  if ! plain_path "$HOME" || [ "$HOME" = / ]; then
    die 2 "HOME is not a plain absolute folder: $HOME" "Set HOME to your home folder, then run this again."
  fi
  HOME_R=$(cd "$HOME" 2>/dev/null && pwd -P) ||
    die 2 "HOME is not a folder: $HOME" "Set HOME to your home folder, then run this again."
  check_signal "$?"
  [ "$HOME_R" != / ] || die 2 "HOME resolves to /: $HOME" "Set HOME to your home folder, then run this again."
  return 0
}

# check_folders: each install folder resolves inside the home (TECHNICAL 3.4.1)
check_folders() {
  for ch_f in "$HOME/.local/bin" "$HOME/.local/share/watane" "$HOME/.local/share/watane-dev" "$HOME/Applications"; do
    if ! resolve_dir "$ch_f" || ! below_home "$RES"; then
      ch_m="resolves to $RES, outside your home folder $HOME_R"
      [ "$RES" != "$HOME_R" ] || ch_m="resolves to your home folder itself"
      die 2 "$(tilde "$ch_f") $ch_m" \
        "watane writes only under your home folder. Make it a real folder there, then run this again."
    fi
  done
  return 0
}

# download <url> <file> <secure|plain>
download() {
  if [ "$3" = secure ]; then
    curl -fsSL --proto '=https' --tlsv1.2 --retry 2 --connect-timeout 15 -o "$2" "$1" 2>"$TMP/curl.err" && return 0
  else
    curl -fsSL --retry 2 --connect-timeout 15 -o "$2" "$1" 2>"$TMP/curl.err" && return 0
  fi
  dl_rc=$?
  check_signal "$dl_rc"
  printf 'watane install: cannot download %s\n' "$1" >&2
  cat "$TMP/curl.err" >&2
  exit 3
}

check_foreign() {
  read_receipt "$RECEIPT"
  cf_old_ok=$R_OK
  cf_old_ver=$R_VERSION
  cf_known=""
  while IFS= read -r cf_l; do
    case $cf_l in 'path '*) ;; *) continue ;; esac
    cf_p=${cf_l#path }
    if allowed "$cf_p"; then cf_known=$(add_item "$cf_known" "$cf_p"); fi
  done <<EOF
$R_LINES
EOF
  find_journals
  while IFS= read -r cf_n; do
    [ -n "$cf_n" ] || continue
    pid_gone "$cf_n" || continue
    read_journal "$SHAREW/journal.$cf_n"
    while IFS= read -r cf_p; do
      [ -n "$cf_p" ] || continue
      in_list "$J_MOVED" "$cf_p" || continue
      if exists "$cf_p.new.$cf_n"; then continue; fi
      cf_known=$(add_item "$cf_known" "$cf_p")
    done <<EOF
$J_NEW
EOF
  done <<EOF
$JOURNALS
EOF
  cf_found=""
  while IFS= read -r cf_p; do
    if in_list "$cf_known" "$cf_p"; then continue; fi
    if exists "$cf_p"; then cf_found=$(add_item "$cf_found" "$cf_p"); fi
  done <<EOF
$ENTRIES
EOF
  if [ "$cf_old_ok" != 1 ] && exists "$RECEIPT"; then cf_found=$(add_item "$cf_found" "$RECEIPT"); fi
  if [ -n "$cf_found" ]; then
    check_signal
    printf 'watane install: found files that watane did not install:\n' >&2
    while IFS= read -r cf_p; do printf '  %s\n' "$(tilde "$cf_p")" >&2; done <<EOF
$cf_found
EOF
    printf 'Move them aside, then run this again, for example:\n' >&2
    while IFS= read -r cf_p; do printf '  mv %s %s.mine\n' "$(tilde "$cf_p")" "$(tilde "$cf_p")" >&2; done <<EOF
$cf_found
EOF
    exit 8
  fi
  if [ "$cf_old_ok" = 1 ]; then
    if [ "$cf_old_ver" = "$RELEASE" ]; then
      say "reinstalling watane $RELEASE"
    else
      say "upgrading watane $cf_old_ver to $RELEASE"
      UPGRADED=1
    fi
  fi
  return 0
}

verify_archive() {
  va_want=""
  va_count=0
  # The remaining fields are deliberately ignored, as in SHA256SUMS's format.
  # shellcheck disable=SC2034
  while read -r va_hash va_name va_rest || [ -n "$va_hash" ]; do
    [ "$va_name" = "$ARCHIVE" ] || continue
    va_want=$va_hash
    va_count=$((va_count + 1))
  done <"$TMP/SHA256SUMS"
  if [ "$va_count" = 0 ]; then
    die 4 "SHA256SUMS has no line for $ARCHIVE" "Nothing was installed."
  elif [ "$va_count" != 1 ]; then
    die 4 "SHA256SUMS has more than one line for $ARCHIVE" "Nothing was installed."
  fi
  if command -v shasum >/dev/null 2>&1; then
    va_got=$(shasum -a 256 "$TMP/$ARCHIVE") || { check_signal "$?"; va_got=""; }
  else
    va_got=$(sha256sum "$TMP/$ARCHIVE") || { check_signal "$?"; va_got=""; }
  fi
  check_signal
  va_got=${va_got%% *}
  if [ "$va_want" != "$va_got" ]; then
    die 4 "checksum mismatch for $ARCHIVE" "expected $va_want" "got $va_got" "Nothing was installed."
  fi
  say "checked: sha256 matches SHA256SUMS of release v$RELEASE"
  return 0
}

extract_archive() {
  tar -tzf "$TMP/$ARCHIVE" >"$TMP/members" 2>/dev/null ||
    die 5 "$ARCHIVE is not a watane archive" "Nothing was installed."
  check_signal
  if grep -Eq '(^/|(^|/)\.\.(/|$))' "$TMP/members"; then
    check_signal "$?"
    die 5 "$ARCHIVE holds a file name that leaves its folder" "Nothing was installed."
  else
    check_signal "$?"
  fi
  # a link member (type l or h in the listing) could make a later chmod or copy reach a file
  # outside the tree: refused before anything is unpacked (3.4 step 9)
  tar -tvzf "$TMP/$ARCHIVE" >"$TMP/members.v" 2>/dev/null ||
    die 5 "$ARCHIVE is not a watane archive" "Nothing was installed."
  check_signal
  if grep -q '^[lh]' "$TMP/members.v"; then
    check_signal "$?"
    die 5 "$ARCHIVE holds a symbolic or hard link, which a watane archive never does" "Nothing was installed."
  else
    check_signal "$?"
  fi
  mkdir "$TMP/x" || { ea_rc=$?; check_signal "$ea_rc"; exit "$ea_rc"; }
  check_signal "$?"
  tar -xzf "$TMP/$ARCHIVE" -C "$TMP/x" ||
    die 5 "$ARCHIVE could not be unpacked" "Nothing was installed."
  check_signal
  TOP=$TMP/x/watane-$RELEASE-$TARGET
  ea_v=""
  if [ -f "$TOP/VERSION" ]; then IFS= read -r ea_v <"$TOP/VERSION" || { check_signal "$?"; }; fi
  [ "$ea_v" = "$RELEASE" ] ||
    die 5 "$ARCHIVE is not a watane archive of version $RELEASE" "Nothing was installed."
  for ea_r in $RELS; do
    exists "$TOP/$ea_r" ||
      die 5 "$ARCHIVE is not a watane archive: $ea_r is missing" "Nothing was installed."
  done
  return 0
}

stage_files() {
  for sf_r in $RELS; do
    dest_of "$sf_r"
    mkdir -p "${DEST%/*}" || return
    cp -R "$TOP/$sf_r" "$DEST.new.$$" || return
    case $sf_r in
      bin/* | share/watane/install.sh) chmod 755 "$DEST.new.$$" || return ;;
    esac
  done
  mkdir -p "$SHAREW" || return
  # a file the script creates is created new: nothing, a link included, may be at the name
  # (noclobber on top of the check, for a link planted in between; 3.6 step 2)
  ! exists "$RECEIPT.new.$$" || return 1
  set -C
  if {
    printf 'watane-receipt 1\nversion %s\ntarget %s\n' "$RELEASE" "$TARGET"
    while IFS= read -r sf_p; do printf 'path %s\n' "$sf_p"; done <<EOF
$ENTRIES
EOF
  } >"$RECEIPT.new.$$"; then sf_rc=0; else sf_rc=$?; fi
  set +C
  return $sf_rc
}

write_journal() {
  ! exists "$SHAREW/journal.$$" || return 1
  set -C
  if {
    while IFS= read -r wj_p; do
      if exists "$wj_p"; then printf 'old %s\n' "$wj_p"; else printf 'new %s\n' "$wj_p"; fi
    done <<EOF
$ENTRIES_ALL
EOF
  } >"$SHAREW/journal.$$"; then wj_rc=0; else wj_rc=$?; fi
  set +C
  return $wj_rc
}

swap_entries() {
  sw_j=$SHAREW/journal.$$
  while IFS= read -r sw_p; do
    if exists "$sw_p"; then mv "$sw_p" "$sw_p.old.$$" || return; fi
    printf 'moved %s\n' "$sw_p" >>"$sw_j" || return
    mv "$sw_p.new.$$" "$sw_p" || return
  done <<EOF
$ENTRIES
EOF
  if exists "$RECEIPT"; then cp -p "$RECEIPT" "$RECEIPT.old.$$" || return; fi
  printf 'moved %s\n' "$RECEIPT" >>"$sw_j" || return
  mv "$RECEIPT.new.$$" "$RECEIPT" || return
  return 0
}

do_swap() {
  check_signal
  SWAP_PID=$$
  SWAP_ACTIVE=1
  # A failed program's status is returned as is, so a signal the shell lost is still seen.
  stage_files || return
  write_journal || return
  swap_entries || return
  return 0
}

install_files() {
  recover
  while IFS= read -r if_p; do
    for if_k in new old; do
      if exists "$if_p.$if_k.$$"; then
        die 5 "$if_p.$if_k.$$ is in the way" "Remove it, then run this again."
      fi
    done
  done <<EOF
$ENTRIES_ALL
EOF
  if do_swap; then :; else
    if_rc=$?
    if_undo=0
    rollback || { if_undo=$?; check_signal "$if_undo"; }
    check_signal "$if_rc"
    if [ "$if_undo" -ne 0 ]; then
      die 5 "could not put the files in place" "The files could not be put back. Run this script again to restore the installed version."
    fi
    die 5 "could not put the files in place" "Nothing was changed: the version that was installed still works."
  fi
  return 0
}

# read_answer: one terminal line, preserving NUL as a rejected byte (3.7, 8.4).
# Encode before shell substitution: macOS sh's read silently drops NUL bytes.
read_answer() {
  ANSWER=""
  ra_bad=0
  while :; do
    # Keep both statuses: a pipeline would hide dd's signal status behind od.
    # dd must read exactly one byte; od alone may read ahead past that byte.
    if [ -z "$TMP" ]; then
      TMP=$(mktemp -d "${TMPDIR:-/tmp}/watane-install.XXXXXX") ||
        die 1 "cannot make a temporary folder in ${TMPDIR:-/tmp}"
      check_signal "$?"
    fi
    dd bs=1 count=1 >"$TMP/answer-byte" 2>/dev/null || { check_signal "$?"; }
    check_signal "$?"
    ra_byte=$(od -An -v -to1 "$TMP/answer-byte") || { check_signal "$?"; }
    check_signal "$?"
    # Field splitting removes od's padding only.
    # shellcheck disable=SC2086
    set -- $ra_byte
    case ${1:-} in
      '') ANSWER=""; return 0 ;;
      012) break ;;
      000) ra_bad=1 ;;
      *)
        if [ "${#ANSWER}" -lt 6 ]; then
          ANSWER=$ANSWER$(printf '%b' "\\0$1")
        else
          ra_bad=1
        fi
        ;;
    esac
  done
  if [ "$ra_bad" = 1 ]; then ANSWER=""; fi
  return 0
}

# --- the toolchain (TECHNICAL 3.7) ---

detect_toolchain() {
  RUSTUP=""
  if command -v rustup >/dev/null 2>&1; then
    RUSTUP=$(command -v rustup)
    check_signal "$?"
  elif [ -x "${CARGO_HOME:-$HOME/.cargo}/bin/rustup" ]; then
    RUSTUP=${CARGO_HOME:-$HOME/.cargo}/bin/rustup
  elif [ -x "$HOME/.cargo/bin/rustup" ]; then
    RUSTUP=$HOME/.cargo/bin/rustup
  fi
  if [ -z "$RUSTUP" ]; then
    TC_STATE=none_a
    return 0
  fi
  # from $HOME, rustup auto-install off: a rust-toolchain.toml of the folder we start in must not
  # make rustup install or run anything before a typed yes
  if dt_rustc=$(cd "$HOME" 2>/dev/null || cd /; RUSTUP_AUTO_INSTALL=0; export RUSTUP_AUTO_INSTALL; "$RUSTUP" which rustc 2>/dev/null); then
    check_signal "$?"
  else
    check_signal "$?"
    TC_STATE=none_b
    return 0
  fi
  dt_sr=$(cd "$HOME" 2>/dev/null || cd /; RUSTUP_AUTO_INSTALL=0; export RUSTUP_AUTO_INSTALL; "$dt_rustc" --print sysroot 2>/dev/null) || { check_signal "$?"; dt_sr=""; }
  check_signal "$?"
  if [ -n "$dt_sr" ] && [ -d "$dt_sr/lib/rustlib/wasm32-wasip2/lib" ]; then
    TC_STATE=ok
  else
    TC_STATE=target
  fi
  return 0
}

print_commands() {
  say 'To install it later, run:'
  case $TC_STATE in
    none_a) say "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --no-modify-path -t wasm32-wasip2" ;;
    target) say '  rustup target add wasm32-wasip2' ;;
    none_b)
      say '  rustup toolchain install stable --profile minimal -t wasm32-wasip2'
      say '  rustup default stable'
      ;;
  esac
  # Print the command literally.
  # shellcheck disable=SC2016
  say 'Then `watane-dev doctor` checks it again.'
  return 0
}

# rustup_step <command...>: run it, or end with exit 6
rustup_step() {
  rs_rc=0
  ( cd "$HOME" 2>/dev/null || cd /; RUSTUP_AUTO_INSTALL=0; export RUSTUP_AUTO_INSTALL; "$@" ) || rs_rc=$?
  if [ "$rs_rc" != 0 ]; then
    check_signal "$rs_rc"
    die 6 "the toolchain step failed (rustup exited $rs_rc)" "watane is installed; run the step yourself, then \`watane-dev doctor\`."
  fi
  return 0
}

toolchain_step() {
  detect_toolchain
  check_signal "$?"
  if [ "$TC_STATE" = ok ]; then
    say 'toolchain: ok (rustup, wasm32-wasip2)'
    TC_OK=1
    return 0
  fi
  TC_OK=0
  if [ "$NO_TC" = 1 ]; then
    say 'toolchain: skipped (--no-toolchain)'
    print_commands
    return 0
  fi
  if ! ( : </dev/tty ) 2>/dev/null; then
    say 'no terminal to ask on: the toolchain was not installed'
    print_commands
    return 0
  fi
  ts_src=https://sh.rustup.rs
  if [ "$TC_STATE" = none_a ] && [ -n "${WATANE_RUSTUP_URL:-}" ]; then ts_src=$WATANE_RUSTUP_URL; fi
  case $TC_STATE in
    target)
      printf '%s\n%s [y/N] ' \
        'watane-dev needs the wasm32-wasip2 target to build mods, and it is missing.' \
        'Add the wasm32-wasip2 target with rustup?'
      ;;
    *)
      printf '%s\n%s [y/N] ' \
        'watane-dev needs the Rust toolchain with the wasm32-wasip2 target to build mods, and it is not installed.' \
        "Install it with rustup (a few hundred MB, from $ts_src, into ~/.rustup and ~/.cargo)?"
      ;;
  esac
  ts_ans=""
  read_answer </dev/tty || { check_signal "$?"; ANSWER=""; }
  ts_ans=$ANSWER
  case $ts_ans in
    [Yy] | [Yy][Ee][Ss]) ;;
    *)
      say 'toolchain: not installed'
      print_commands
      return 0
      ;;
  esac
  case $TC_STATE in
    none_a)
      if [ -n "${WATANE_RUSTUP_URL:-}" ]; then
        say "using WATANE_RUSTUP_URL=$WATANE_RUSTUP_URL"
        download "$WATANE_RUSTUP_URL" "$TMP/rustup-init.sh" plain
      else
        download https://sh.rustup.rs "$TMP/rustup-init.sh" secure
      fi
      rustup_step sh "$TMP/rustup-init.sh" -y --profile minimal --no-modify-path -t wasm32-wasip2 </dev/null
      say 'rustup put its programs in ~/.cargo/bin; watane-dev finds them there. To use cargo yourself, add that folder to your PATH.'
      ;;
    target)
      rustup_step "$RUSTUP" target add wasm32-wasip2
      ;;
    none_b)
      rustup_step "$RUSTUP" toolchain install stable --profile minimal -t wasm32-wasip2
      rustup_step "$RUSTUP" default stable
      ;;
  esac
  say 'toolchain: installed'
  detect_toolchain
  check_signal "$?"
  if [ "$TC_STATE" != ok ]; then
    check_signal
    say 'toolchain: still incomplete after the install'
    exit 6
  fi
  TC_OK=1
  return 0
}

# --- the end of an install (TECHNICAL 3.9, 3.10) ---

path_block() {
  # Print the home shorthand for the person.
  # shellcheck disable=SC2088
  say '~/.local/bin is not on your PATH, so a new terminal will not find watane-dev yet.'
  case ${SHELL:-} in
    */zsh | */bash)
      # Print the shell file name literally.
      # shellcheck disable=SC2088
      case $SHELL in */zsh) pb_f='~/.zshrc' ;; *) pb_f='~/.bash_profile' ;; esac
      say 'Add it, then open a new terminal:'
      say "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> $pb_f"
      ;;
    *)
      say "Add it to your shell's startup file, then open a new terminal:"
      say "  export PATH=\"\$HOME/.local/bin:\$PATH\""
      ;;
  esac
  return 0
}

final_output() {
  say "watane $RELEASE is installed."
  if [ "$UPGRADED" = 1 ]; then say 'If watane is open, quit it and open it again.'; fi
  say ''
  say 'If macOS says "watane" cannot be opened, or could not verify that it is free of malware:'
  say '  watane.app is not signed by an Apple developer, because there is no account behind watane.'
  say '  Open System Settings, Privacy & Security, find the line about "watane" and press "Open Anyway".'
  say '  Or run this once, then open the app again:'
  say '    xattr -dr com.apple.quarantine ~/Applications/watane.app'
  say ''
  case ":${PATH:-}:" in
    *":$PREFIX/bin:"*)
      fo_dev=watane-dev
      fo_fmt='  %-38s%s\n'
      ;;
    *)
      path_block
      say ''
      # Print the command with home shorthand.
      # shellcheck disable=SC2088
      fo_dev='~/.local/bin/watane-dev'
      fo_fmt='  %-44s%s\n'
      ;;
  esac
  say 'Next:'
  # shellcheck disable=SC2059
  {
    printf "$fo_fmt" 'open ~/Applications/watane.app' 'the app'
    printf "$fo_fmt" "$fo_dev doctor" 'checks the maker tools'
    printf "$fo_fmt" "$fo_dev new reading-list" 'makes a mod'
  }
  if [ "$TC_OK" != 1 ]; then say '  (install the toolchain first: see above)'; fi
  if xcode-select -p >/dev/null 2>&1; then
    check_signal "$?"
  else
    check_signal "$?"
    say "watane-dev needs the macOS command line tools (a C linker and git). Run this and follow macOS's own dialog:"
    say '  xcode-select --install'
  fi
  return 0
}

# --- the commands ---

do_install() {
  TMPBASE=${TMPDIR:-/tmp}
  TMP=$(mktemp -d "$TMPBASE/watane-install.XXXXXX") ||
    die 1 "cannot make a temporary folder in $TMPBASE" "Set TMPDIR to a folder you can write to, then run this again."
  check_signal "$?"
  ARCHIVE=watane-$RELEASE-$TARGET.tar.gz
  BASE=${WATANE_RELEASE_BASE:-https://github.com/civic-tech-pioneer/watane/releases/download/v$RELEASE}
  check_no_live_install
  check_foreign
  if [ -n "${WATANE_RELEASE_BASE:-}" ]; then
    say "using WATANE_RELEASE_BASE=$WATANE_RELEASE_BASE"
    DLMODE=plain
  else
    DLMODE=secure
  fi
  download "$BASE/SHA256SUMS" "$TMP/SHA256SUMS" "$DLMODE"
  say "downloading $ARCHIVE"
  download "$BASE/$ARCHIVE" "$TMP/$ARCHIVE" "$DLMODE"
  verify_archive
  extract_archive
  install_files
  xattr -dr com.apple.quarantine "$HOME/Applications/watane.app" >/dev/null 2>&1 || { check_signal "$?"; }
  check_signal "$?"
  finish_commit "$$" || { check_signal "$?"; }
  check_signal "$?"
  SWAP_ACTIVE=0
  say 'installed:'
  for di_r in $RELS; do
    dest_of "$di_r"
    case $di_r in bin/* | share/watane-dev/vendor | Applications/*) say "  $(tilde "$DEST")" ;; esac
  done
  toolchain_step
  final_output
  return 0
}

do_status() {
  read_receipt "$RECEIPT"
  [ "$R_OK" = 1 ] || die 7 "not installed: no receipt at $(tilde "$RECEIPT")"
  say "watane $R_VERSION ($R_TARGET)"
  while IFS= read -r ds_l; do
    case $ds_l in 'path '*) say "$(tilde "${ds_l#path }")" ;; esac
  done <<EOF
$R_LINES
EOF
  return 0
}

# home_deletable <home>: HOME_DEL is its resolved path when the home may be deleted
home_deletable() {
  plain_path "$1" || return 1
  case "$HOME/" in "$1"/*) return 1 ;; esac
  hd_r=$(cd "$1" 2>/dev/null && pwd -P) || { check_signal "$?"; return 1; }
  check_signal "$?"
  plain_path "$hd_r" || return 1
  case "$HOME_R/" in "$hd_r"/*) return 1 ;; esac
  HOME_DEL=$hd_r
  # a home that resolves outside the home is the person's to delete (8.4 step 5)
  if ! in_home "$hd_r"; then HOME_OUT=1; return 1; fi
  return 0
}

# shell_quote <text>: the text in single quotes, for a command the person runs
shell_quote() {
  sq_text=$(printf '%s' "$1" | sed "s/'/'\\\\''/g") || { check_signal "$?"; }
  check_signal "$?"
  printf "'%s'" "$sq_text"
}

do_uninstall() {
  check_no_live_install
  recover
  read_receipt "$RECEIPT"
  [ "$R_OK" = 1 ] || die 7 "nothing to uninstall: no receipt at $(tilde "$RECEIPT")"
  du_ver=$R_VERSION
  while IFS= read -r du_l; do
    case $du_l in 'path '*) ;; *) continue ;; esac
    du_p=${du_l#path }
    if allowed "$du_p"; then
      if exists "$du_p"; then rm_path "$du_p" || { check_signal "$?"; }; fi
    else
      warn "skipped $du_p: not a path that watane installs"
    fi
  done <<EOF
$R_LINES
EOF
  clean_stale || { check_signal "$?"; }
  if allowed "$RECEIPT"; then
    rm -f "$RECEIPT" || { check_signal "$?"; warn "could not remove $RECEIPT"; }
  else
    warn "skipped $RECEIPT: not a path that watane installs"
  fi
  for du_d in "$SHAREW" "$PREFIX/share/watane-dev" "$PREFIX/share" "$PREFIX/bin"; do
    if resolve_dir "$du_d" && below_home "$RES"; then rmdir "$du_d" 2>/dev/null || { check_signal "$?"; }; fi
  done
  say "uninstalled watane $du_ver"
  if exists "$HOME/Library/Caches/watane-dev"; then
    say 'kept: ~/Library/Caches/watane-dev (maker cache; safe to delete)'
  fi
  if exists "$HOME/.rustup" || exists "$HOME/.cargo"; then
    say 'kept: ~/.rustup and ~/.cargo (the Rust toolchain; remove with rustup self uninstall)'
  fi
  du_h=${WATANE_HOME:-$HOME/.watane}
  [ -e "$du_h" ] || return 0
  HOME_OUT=0
  if ! home_deletable "$du_h"; then
    if [ "$HOME_OUT" = 1 ]; then
      say "kept: $(tilde "$du_h") (not deleted: it resolves to $HOME_DEL, outside your home folder)"
      say 'To delete it yourself, run:'
      du_quoted=$(shell_quote "$HOME_DEL") || { check_signal "$?"; }
      check_signal "$?"
      say "  rm -rf $du_quoted"
      return 0
    fi
    say "kept: $(tilde "$du_h") (not deleted: not a folder that watane can delete)"
    return 0
  fi
  if ( : </dev/tty ) 2>/dev/null; then
    printf 'Delete the watane home %s? It holds your identity and your mods, and cannot be restored without a backup. Type delete to delete it, or press Enter to keep it: ' "$(tilde_r "$HOME_DEL")"
    du_a=""
    read_answer </dev/tty || { check_signal "$?"; ANSWER=""; }
    du_a=$ANSWER
    if [ "$du_a" = delete ]; then
      rm -rf -- "$HOME_DEL"
      check_signal "$?"
      if [ -L "$du_h" ]; then rm -f -- "$du_h"; check_signal "$?"; fi
      say "deleted: $(tilde "$du_h")"
      return 0
    fi
  fi
  say "kept: $(tilde "$du_h") (your home: identity, mods, channels)"
  return 0
}

main() {
  trap cleanup EXIT
  trap 'on_signal 129' HUP
  trap 'on_signal 130' INT
  trap 'on_signal 143' TERM
  trap 'on_signal 141' PIPE

  HOME=${HOME:-}
  MODE=install
  HAVE_REL=0
  REL_ARG=""
  NO_TC=0
  while [ $# -gt 0 ]; do
    case $1 in
      --release)
        [ $# -ge 2 ] || die 1 "--release needs a version" "a version looks like 0.1.0"
        HAVE_REL=1
        REL_ARG=$2
        shift
        ;;
      --no-toolchain) NO_TC=1 ;;
      --uninstall | --status)
        [ "$MODE" = install ] || bad_flag "$1"
        MODE=${1#--}
        ;;
      --help | -h)
        usage
        exit 0
        ;;
      *) bad_flag "$1" ;;
    esac
    shift
  done
  if [ "$NO_TC" = 1 ] && [ "$MODE" != install ]; then bad_flag --no-toolchain; fi
  if [ "$HAVE_REL" = 1 ]; then
    RELEASE=$REL_ARG
  elif [ -n "${WATANE_RELEASE:-}" ]; then
    RELEASE=$WATANE_RELEASE
  else
    RELEASE=$DEFAULT_RELEASE
  fi
  if [ "$MODE" = install ]; then check_version "$RELEASE"; fi

  # step 2: root
  main_uid=$(id -u 2>/dev/null) || die 2 "the tool id is missing" "watane install uses only the tools that stock macOS ships."
  check_signal
  [ "$main_uid" != 0 ] || die 1 "do not run this as root or with sudo" "watane installs into your own home folder and needs no password."
  # step 3: tools
  check_tools "uname id mkdir mv cp rm rmdir cat chmod grep awk tr readlink kill dd od mktemp"
  if [ "$MODE" = install ]; then
    check_tools "curl tar"
    command -v shasum >/dev/null 2>&1 || check_tools sha256sum
  fi
  # step 4: platform, home, folders
  if [ "$MODE" = install ]; then check_platform; fi
  check_home
  set_entries
  if [ "$MODE" = install ]; then check_folders; fi
  check_signal
  case $MODE in
    status) do_status ;;
    uninstall) do_uninstall ;;
    *)
      say "watane $RELEASE: installing for $TARGET"
      do_install
      ;;
  esac
}

main "$@"
