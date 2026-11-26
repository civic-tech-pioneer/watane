#!/bin/sh
# install.sh: installs, upgrades, reports and removes watane (install/TECHNICAL 3).
# POSIX sh, only the tools of TECHNICAL 3.3. The whole body is the function `main`,
# called on the last line, so that a download cut off half way runs nothing.
# A waiting shell can dispatch INT only as it exits on input EOF.
EARLY_INT=0
trap 'if [ "$?" = 130 ] || [ "$EARLY_INT" = 1 ]; then
  printf "\nwatane setup was interrupted. Run the same command to try again.\n" >&2
  exit 130
fi' EXIT
trap 'EARLY_INT=1' INT
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
R_RESOLVED=""
J_OLD=""
J_NEW=""
J_MOVED=""
JOURNALS=""
TC_STATE=""
TC_OK=0
RUSTUP=""
UPGRADED=0
SIGNAL=0
LOCK_OWNED=0
CLEANUP_ACTIVE=0
EXITING=0
COMMITTED=0
PROMPT=""
TC_DOWNLOADING=0
TC_RUNNING=0
CREATED=""

usage() {
  printf '%s\n' \
    'usage: install.sh [--release <version>] [--no-toolchain]' \
    '       install.sh --uninstall' \
    '       install.sh --status' \
    '       install.sh --help | -h'
}

# A reader that went away (`--status | head -1`) must not make printf print a write error:
# the PIPE trap ends the run with 141. Any other write error (a full disk, a closed stdout, a
# PIPE that is ignored) still ends the run, with exit 1 and no text of printf's (3.9).
say() { printf '%s\n' "$*" 2>/dev/null || { say_rc=$?; check_signal "$say_rc"; exit 1; }; }
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

# receipt_allowed <path>: 8.5, including the install-time folder identity.
receipt_allowed() {
  ra_folder=""
  if allowed "$1"; then :; else
    check_signal "$?"
    warn "skipped $(tilde "$1"): not a path that watane installs"
    return 1
  fi
  ra_folder=${al_root#"$HOME/"}
  ra_now=$al_rr
  ra_old='an unrecorded path'
  while IFS= read -r ra_l; do
    case $ra_l in
      "resolved $ra_folder "*) ra_old=${ra_l#"resolved $ra_folder "} ;;
    esac
  done <<EOF
$R_RESOLVED
EOF
  if [ "$ra_old" = "$ra_now" ]; then return 0; fi
  warn "skipped $(tilde "$1"): ~/$ra_folder resolves to $(tilde_r "$ra_now"), not to $(tilde_r "$ra_old") as at install"
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

# recovery_allowed <entry> <pid> [recorded locations]: require the expected physical
# folder for a past run, or the checked staging location for this active swap.
recovery_allowed() {
  allowed "$1" || return 1
  ra_relative=${al_root#"$HOME/"}
  if [ "$2" = "$$" ] && [ "$SWAP_ACTIVE" = 1 ]; then
    in_list "${FOLDERS_RESOLVED:-}" "resolved $ra_relative $al_rr"
    return
  fi
  ra_recorded=0
  while IFS= read -r ra_record; do
    case $ra_record in
      "resolved $ra_relative "*)
        ra_recorded=1
        [ "$ra_record" = "resolved $ra_relative $al_rr" ] || return 1
        ;;
    esac
  done <<EOF
${3:-}
EOF
  [ "$ra_recorded" = 1 ] || [ "$al_rr" = "$HOME_R/$ra_relative" ]
}

# recovery_allowed_both <entry> <pid> <locations> <staged ok> <staged locations>: recovery_allowed
# against the published receipt's locations and, when the staged receipt is readable, against
# its own too: the staged one may veto where the published one allows (3.6 step 4).
recovery_allowed_both() {
  recovery_allowed "$1" "$2" "$3" || return 1
  [ "$4" != 1 ] || recovery_allowed "$1" "$2" "$5"
}

# read_receipt <file>: R_OK, R_VERSION, R_TARGET and the raw path and resolved lines
read_receipt() {
  R_OK=0
  R_VERSION=""
  R_TARGET=""
  R_LINES=""
  R_RESOLVED=""
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
      'resolved '*) R_RESOLVED=$(add_item "$R_RESOLVED" "$rr_l") ;;
      'path '*) R_LINES=$(add_item "$R_LINES" "$rr_l") ;;
    esac
  done <"$1"
  return 0
}

# Use the staged receipt only if the published receipt is absent.
read_recovery_receipt() {
  if exists "$RECEIPT"; then
    read_receipt "$RECEIPT"
  else
    read_receipt "$RECEIPT.new.$1"
  fi
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
    if [ ! -f "$fj_f" ] || [ -L "$fj_f" ]; then continue; fi
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
      "If no install is running, that number now belongs to another program; remove $(tilde "$SHAREW/journal.$cn_n") and run this again."
  done <<EOF
$JOURNALS
EOF
  return 0
}

# lock_pid_suffix <lock>: show the pid only for a plain lock with a valid pid file.
lock_pid_suffix() {
  [ ! -L "$1" ] && [ -f "$1/pid" ] && [ ! -L "$1/pid" ] || return 0
  lp_pid=""
  lp_lines=0
  while IFS= read -r lp_line || [ -n "$lp_line" ]; do
    lp_lines=$((lp_lines + 1))
    lp_pid=$lp_line
  done <"$1/pid"
  case $lp_pid in
    '' | *[!0123456789]*) ;;
    *) if [ "$lp_lines" = 1 ]; then printf ' (pid %s)' "$lp_pid"; fi ;;
  esac
  return 0
}

# take_lock: never break a lock, even when its pid is dead (3.6.2).
take_lock() {
  LOCK=$HOME/.watane-install.lock
  if mkdir "$LOCK" 2>/dev/null; then
    LOCK_OWNED=1
    check_signal "$?"
    (set -C; printf '%s\n' "$$" >"$LOCK/pid") || { check_signal "$?"; die 5 "could not write install lock"; }
    check_signal "$?"
    return 0
  else
    check_signal "$?"
  fi
  tl_suffix=$(lock_pid_suffix "$LOCK")
  check_signal "$?"
  die 5 "another install may be running$tl_suffix" \
    "If no install is running, remove its lock and run this again:" \
    '  rm -rf ~/.watane-install.lock'
}

# rm_path <path>: rm -rf with a warning
rm_path() {
  rm -rf "$1" || { check_signal "$?"; warn "could not remove $(tilde "$1")"; return 1; }
  return 0
}

# undo_swap <pid>: put the old files back from the journal of that pid (3.6 step 4)
undo_swap() {
  (
    # The parent retains the signal code; checks inside the undo must let it finish.
    SIGNAL=0
    # The cleanup phase belongs to the parent, not this undo subshell.
    # shellcheck disable=SC2030
    CLEANUP_ACTIVE=0
    # shellcheck disable=SC2030
    EXITING=0
    set +e
    allowed "$RECEIPT" || exit 0
    read_recovery_receipt "$1"
    ub_locations=$R_RESOLVED
    ub_new_ok=0
    recovery_allowed "$RECEIPT" "$1" "$ub_locations" || exit 0
    ub_j=$SHAREW/journal.$1
    if [ -f "$ub_j" ] && [ ! -L "$ub_j" ]; then
      # the receipt moved into place: the commit happened, there is nothing to undo
      if ! exists "$RECEIPT.new.$1"; then finish_commit "$1" || exit "$?"; exit 0; fi
      read_receipt "$RECEIPT"
      ub_old_ok=$R_OK
      ub_old_paths=$R_LINES
      read_receipt "$RECEIPT.new.$1"
      ub_new_ok=$R_OK
      ub_new_paths=$R_LINES
      ub_new_locations=$R_RESOLVED
      read_journal "$ub_j"
      ub_blocked=0
      while IFS= read -r ub_p; do
        [ -n "$ub_p" ] || continue
        if [ "$ub_p" != "$RECEIPT" ]; then
          if [ "$ub_old_ok" = 1 ] || [ "$ub_new_ok" = 1 ]; then
            in_list "$ub_old_paths" "path $ub_p" || continue
          fi
        fi
        if ! exists "$ub_p.old.$1" && ! in_list "$J_MOVED" "$ub_p"; then continue; fi
        recovery_allowed "$ub_p" "$1" "$ub_locations" || { ub_blocked=1; continue; }
        exists "$ub_p.old.$1" || continue
        if [ "$ub_p" = "$RECEIPT" ]; then in_list "$J_MOVED" "$ub_p" || continue; fi
        if [ "$ub_p" != "$RECEIPT" ]; then rm_path "$ub_p" || exit "$?"; fi
        mv "$ub_p.old.$1" "$ub_p" || exit "$?"
      done <<EOF
$J_OLD
EOF
      while IFS= read -r ub_p; do
        [ -n "$ub_p" ] || continue
        in_list "$J_MOVED" "$ub_p" || continue
        if [ "$ub_new_ok" = 1 ]; then
          in_list "$ub_new_paths" "path $ub_p" || continue
        elif [ "$ub_old_ok" = 1 ]; then
          in_list "$ub_old_paths" "path $ub_p" || continue
        fi
        recovery_allowed_both "$ub_p" "$1" "$ub_locations" "$ub_new_ok" "$ub_new_locations" || { ub_blocked=1; continue; }
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
      [ "$ub_blocked" = 0 ] || exit 0
    fi
    # Retire the journal while every staging name still proves what did not move.
    # A retry after this point cleans staging only; it cannot claim a person's file.
    rm -f "$SHAREW/journal.$1" || exit "$?"
    while IFS= read -r ub_p; do
      recovery_allowed "$ub_p" "$1" "$ub_locations" || continue
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
  if ! allowed "$RECEIPT"; then return 0; fi
  read_receipt "$RECEIPT"
  if ! recovery_allowed "$RECEIPT" "$1" "$R_RESOLVED"; then
    receipt_allowed "$RECEIPT" || { check_signal "$?"; }
    return 0
  fi
  fc_old=$SHAREW/receipt.old.$1
  if [ "$R_OK" = 1 ]; then
    fc_cur=$R_LINES
    read_receipt "$fc_old"
    if [ "$R_OK" = 1 ]; then
      while IFS= read -r fc_l; do
        case $fc_l in 'path '*) ;; *) continue ;; esac
        fc_p=${fc_l#path }
        if in_list "$fc_cur" "$fc_l"; then continue; fi
        receipt_allowed "$fc_p" || { check_signal "$?"; continue; }
        recovery_allowed "$fc_p" "$1" "$R_RESOLVED" || continue
        if exists "$fc_p"; then rm_path "$fc_p" || return; fi
      done <<EOF
$R_LINES
EOF
    fi
  fi
  # Old-only paths use the old receipt; backups use the committed new receipt.
  read_receipt "$RECEIPT"
  while IFS= read -r fc_p; do
    receipt_allowed "$fc_p" || { check_signal "$?"; continue; }
    recovery_allowed "$fc_p" "$1" "$R_RESOLVED" || continue
    if exists "$fc_p.old.$1"; then rm_path "$fc_p.old.$1" || return; fi
  done <<EOF
$ENTRIES
EOF
  if receipt_allowed "$RECEIPT"; then
    if exists "$fc_old"; then rm_path "$fc_old" || return; fi
  else
    check_signal "$?"
  fi
  rm -f "$SHAREW/journal.$1" || { check_signal "$?"; return 1; }
  return 0
}

# clean_stale [report]: remove or report staging names of dead processes (3.8).
clean_stale() {
  while IFS= read -r cs_e; do
    allowed "$cs_e" || continue
    for cs_k in new old; do
      for cs_f in "$cs_e.$cs_k".*; do
        exists "$cs_f" || continue
        cs_n=${cs_f#"$cs_e.$cs_k."}
        case $cs_n in '' | *[!0-9]*) continue ;; esac
        pid_gone "$cs_n" || continue
        read_recovery_receipt "$cs_n"
        recovery_allowed "$RECEIPT" "" "$R_RESOLVED" || continue
        recovery_allowed "$cs_e" "" "$R_RESOLVED" || continue
        if [ "${1:-}" = report ]; then
          say "leftover: $(tilde "$cs_f")"
          CS_REPORTED=1
        else
          # A journal that recovery could not retire still needs its staging evidence.
          cs_j=$SHAREW/journal.$cs_n
          if [ -f "$cs_j" ] && [ ! -L "$cs_j" ]; then continue; fi
          rm_path "$cs_f" || return
          RECOVERED=1
        fi
      done
    done
  done <<EOF
$ENTRIES_ALL
EOF
  return 0
}

# prune_watane_dirs <locations>: rmdir the two Watane-specific folders when empty and when the
# recorded locations say where they are (8.4 steps 1 and 2; 8.5 rule 4).
prune_watane_dirs() {
  for pw_entry in "$SHAREW/VERSION" "$PREFIX/share/watane-dev/vendor"; do
    allowed "$pw_entry" || continue
    in_list "$1" "resolved ${al_root#"$HOME/"} $al_rr" || continue
    rmdir "${pw_entry%/*}" 2>/dev/null || { check_signal "$?"; }
  done
  return 0
}

# recover [prune]: undo or finish a dead swap, then clean its names (3.6 step 0).
recover() {
  RECOVERED=0
  RECOVERED_SAID=0
  rc_pending=0
  rc_prune=""
  # the journals and receipt.* lie beside the receipt: none is used through a link out (8.5)
  allowed "$RECEIPT" || return 0
  find_journals
  while IFS= read -r rc_n; do
    [ -n "$rc_n" ] || continue
    pid_gone "$rc_n" || continue
    if exists "$RECEIPT.new.$rc_n"; then
      rc_locations=""
      if [ "${1:-}" = prune ] && ! exists "$RECEIPT"; then
        read_receipt "$RECEIPT.new.$rc_n"
        if [ "$R_OK" = 1 ]; then rc_locations=$R_RESOLVED; fi
      fi
      undo_swap "$rc_n" || { check_signal "$?"; die 5 "could not recover interrupted install (pid $rc_n)" "Recovery stopped; fix the filesystem error and run this again."; }
      if [ -n "$rc_locations" ] && ! exists "$SHAREW/journal.$rc_n"; then
        rc_prune=$rc_locations
      fi
    else
      finish_commit "$rc_n" || { check_signal "$?"; die 5 "could not recover interrupted install (pid $rc_n)" "Recovery stopped; fix the filesystem error and run this again."; }
    fi
    if exists "$SHAREW/journal.$rc_n"; then rc_pending=1; else RECOVERED=1; fi
  done <<EOF
$JOURNALS
EOF
  clean_stale || { check_signal "$?"; die 5 "could not recover stale install files" "Recovery stopped; fix the filesystem error and run this again."; }
  if [ -n "$rc_prune" ]; then prune_watane_dirs "$rc_prune"; fi
  if [ "$RECOVERED" = 1 ] && [ "$rc_pending" = 0 ]; then
    say "recovered interrupted watane install"
    RECOVERED_SAID=1
  fi
  return 0
}

# rollback: undo the swap in progress (3.8). It counts as undone only when undo_swap ended well: a
# subshell that macOS sh forks while a trapped signal is pending dies of that signal, and the
# handler that runs next must then undo it again.
rollback() {
  if [ "$SWAP_ACTIVE" = 1 ]; then
    # Committed is what undo itself decides on: the journal is there and the receipt's
    # `.new.<pid>` is gone (3.8). Read it before undo removes either.
    if [ -f "$SHAREW/journal.$SWAP_PID" ] && [ ! -L "$SHAREW/journal.$SWAP_PID" ] && ! exists "$RECEIPT.new.$SWAP_PID"; then COMMITTED=1; fi
    undo_swap "$SWAP_PID" || return
    SWAP_ACTIVE=0
  fi
  return 0
}

on_signal() {
  SIGNAL=$1
  trap '' HUP INT TERM PIPE
  rollback || :
  check_signal "$1"
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
    # Undo and resource subshells deliberately clear their local cleanup phase.
    # shellcheck disable=SC2031
    if [ "$CLEANUP_ACTIVE" = 1 ]; then
      CLEANUP_ACTIVE=0
      release_files || { check_signal "$?"; }
    fi
    # Once EXIT handling has started, no later exit trap can report the signal.
    # shellcheck disable=SC2031
    if [ "$EXITING" = 1 ]; then signal_hint "$SIGNAL"; fi
    exit "$SIGNAL"
  fi
  return 0
}

# Release run-owned resources even when cleanup's rollback selects a signal exit.
# The parent keeps its signal; cleanup failures are checked without re-entering this helper.
release_files() {
  (
    # Keep the parent's selected exit signal unchanged.
    # shellcheck disable=SC2030
    SIGNAL=0
    CLEANUP_ACTIVE=0
    EXITING=0
    if [ "$LOCK_OWNED" = 1 ]; then
      rm -f "$LOCK/pid" || { check_signal "$?"; }
      rmdir "$LOCK" || { check_signal "$?"; }
    fi
    remove_created
    if [ -n "$TMP" ]; then rm -rf "$TMP" || { check_signal "$?"; }; fi
    exit 0
  )
}

# interrupted_hint: a signal after the commit left watane installed (3.8, 3.9)
interrupted_hint() {
  doctor_command
  if [ "${1:-}" = failed ]; then
    printf 'watane %s is installed.\n' "$RELEASE" >&2
  else
    printf 'watane %s is installed; the run was interrupted before it finished.\n' "$RELEASE" >&2
  fi
  printf 'Run %s to check the maker tools.\n' "$DOCTOR" >&2
  if [ "$PROMPT" = toolchain ] || [ "$TC_DOWNLOADING" = 1 ]; then print_commands >&2; fi
  return 0
}

signal_hint() {
  sh_signal=$1
  set +e
  # Formatting a quoted mirror URL performs its own signal checks.
  SIGNAL=0
  if [ "$sh_signal" = 130 ] || [ -n "$PROMPT" ] || { [ "$COMMITTED" = 1 ] && [ "$TC_RUNNING" = 1 ]; }; then
    printf '\n' >&2
  fi
  if [ "$COMMITTED" = 1 ]; then
    interrupted_hint
  elif [ "$PROMPT" = uninstall ]; then
    printf '%s; the watane home was kept.\n' "$du_outcome" >&2
  elif [ "$sh_signal" = 130 ]; then
    printf 'watane setup was interrupted. Run the same command to try again.\n' >&2
  fi
  # Keep the exit status selected before formatting the hint.
  SIGNAL=$sh_signal
}

cleanup() {
  cl_rc=$?
  trap - EXIT
  EXITING=1
  CLEANUP_ACTIVE=1
  ( : ) || true
  rollback || { check_signal "$?"; }
  # Resource cleanup is attempted once; it must never revisit a released lock.
  CLEANUP_ACTIVE=0
  release_files || { check_signal "$?"; }
  # release_files checks cleanup commands with a separate, local signal flag.
  # shellcheck disable=SC2031
  if [ "$SIGNAL" != 0 ]; then
    cl_rc=$SIGNAL
    signal_hint "$cl_rc"
  fi
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
    command -v "$ct_t" >/dev/null 2>&1 || { check_signal "$?"; die 2 "the tool $ct_t is missing" "watane install uses only the tools that stock macOS ships; $ct_t was not found on your PATH."; }
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
  HOME_R=$(cd "$HOME" 2>/dev/null && pwd -P) || { check_signal "$?"; die 2 "HOME is not a folder: $HOME" "Set HOME to your home folder, then run this again."; }
  check_signal "$?"
  [ "$HOME_R" != / ] || die 2 "HOME resolves to /: $HOME" "Set HOME to your home folder, then run this again."
  return 0
}

# check_folders: each install folder resolves inside the home (TECHNICAL 3.4.1)
check_folders() {
  FOLDERS_RESOLVED=""
  for ch_f in "$HOME/.local/bin" "$HOME/.local/share/watane" "$HOME/.local/share/watane-dev" "$HOME/Applications"; do
    if ! resolve_dir "$ch_f" || ! below_home "$RES"; then
      ch_m="resolves to $(tilde_r "$RES"), outside your home folder"
      [ "$RES" != "$HOME_R" ] || ch_m="resolves to your home folder itself"
      die 2 "$(tilde "$ch_f") $ch_m" \
        "watane writes only under your home folder. Make it a real folder there, then run this again."
    fi
    FOLDERS_RESOLVED=$(add_item "$FOLDERS_RESOLVED" "resolved ${ch_f#"$HOME/"} $RES")
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
  if [ "$TC_DOWNLOADING" = 1 ]; then
    warn "cannot download the rustup installer from $1"
  else
    warn "cannot download $1"
  fi
  cat "$TMP/curl.err" >&2
  if [ "$TC_DOWNLOADING" = 1 ]; then
    printf 'watane %s is installed.\n' "$RELEASE" >&2
    print_commands >&2
  fi
  exit 3
}

check_foreign() {
  read_receipt "$RECEIPT"
  cf_old_ok=$R_OK
  cf_old_ver=$R_VERSION
  cf_known=""
  cf_rule4=""
  cf_folders=""
  while IFS= read -r cf_l; do
    case $cf_l in 'path '*) ;; *) continue ;; esac
    cf_p=${cf_l#path }
    if receipt_allowed "$cf_p" 2>/dev/null; then
      cf_known=$(add_item "$cf_known" "$cf_p")
    else
      check_signal "$?"
      if [ -n "$ra_folder" ] && in_list "$ENTRIES" "$cf_p" && exists "$cf_p"; then
        cf_rule4=$(add_item "$cf_rule4" "$cf_p")
        if ! in_list "$cf_folders" "$ra_folder"; then
          cf_folders=$(add_item "$cf_folders" "$ra_folder")
          # shellcheck disable=SC2088 # Show the home as a literal tilde in the fix text.
          warn "~/$ra_folder resolves to $(tilde_r "$ra_now"), not to $(tilde_r "$ra_old") as at install"
          printf 'Make ~/%s a real folder again (or point it where it was), then run this again.\n' "$ra_folder" >&2
        fi
      fi
    fi
  done <<EOF
$R_LINES
EOF
  find_journals
  while IFS= read -r cf_n; do
    [ -n "$cf_n" ] || continue
    pid_gone "$cf_n" || continue
    allowed "$RECEIPT" || continue
    read_recovery_receipt "$cf_n"
    recovery_allowed "$RECEIPT" "$cf_n" "$R_RESOLVED" || continue
    cf_locations=$R_RESOLVED
    cf_ownership=$R_OK
    cf_paths=$R_LINES
    cf_staged=0
    cf_staged_locations=""
    if exists "$RECEIPT.new.$cf_n"; then
      read_receipt "$RECEIPT.new.$cf_n"
      if [ "$R_OK" = 1 ]; then
        cf_ownership=1
        cf_paths=$R_LINES
        cf_staged=1
        cf_staged_locations=$R_RESOLVED
      fi
    fi
    read_journal "$SHAREW/journal.$cf_n"
    while IFS= read -r cf_p; do
      [ -n "$cf_p" ] || continue
      recovery_allowed_both "$cf_p" "$cf_n" "$cf_locations" "$cf_staged" "$cf_staged_locations" || continue
      if [ "$cf_ownership" = 1 ]; then in_list "$cf_paths" "path $cf_p" || continue; fi
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
    if in_list "$cf_rule4" "$cf_p"; then continue; fi
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
  if [ -n "$cf_rule4" ]; then check_signal; exit 8; fi
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

# not_swapped_msg: sets NS_MSG, the last line of a failure before any swap (3.9):
# a receipt means a version was installed, but this runs before recover (3.6 step 0),
# so it cannot claim the version still works
not_swapped_msg() {
  NS_MSG='Nothing was installed.'
  if exists "${RECEIPT:-}"; then NS_MSG='Nothing was changed.'; fi
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
  not_swapped_msg
  if [ "$va_count" = 0 ]; then
    die 4 "SHA256SUMS has no line for $ARCHIVE" "$NS_MSG"
  elif [ "$va_count" != 1 ]; then
    die 4 "SHA256SUMS has more than one line for $ARCHIVE" "$NS_MSG"
  fi
  if command -v shasum >/dev/null 2>&1; then
    va_got=$(shasum -a 256 "$TMP/$ARCHIVE") || { check_signal "$?"; va_got=""; }
  else
    va_got=$(sha256sum "$TMP/$ARCHIVE") || { check_signal "$?"; va_got=""; }
  fi
  check_signal
  va_got=${va_got%% *}
  if [ "$va_want" != "$va_got" ]; then
    die 4 "checksum mismatch for $ARCHIVE" "expected $va_want" "got $va_got" "$NS_MSG"
  fi
  say "checked: sha256 matches SHA256SUMS of release v$RELEASE"
  return 0
}

extract_archive() {
  not_swapped_msg
  tar -tzf "$TMP/$ARCHIVE" >"$TMP/members" 2>/dev/null || { check_signal "$?"; die 5 "$ARCHIVE is not a watane archive" "$NS_MSG"; }
  check_signal
  if grep -Eq '(^/|(^|/)\.\.(/|$))' "$TMP/members"; then
    check_signal "$?"
    die 5 "$ARCHIVE holds a file name that leaves its folder" "$NS_MSG"
  else
    check_signal "$?"
  fi
  # a link member (type l or h in the listing) could make a later chmod or copy reach a file
  # outside the tree: refused before anything is unpacked (3.4 step 9)
  tar -tvzf "$TMP/$ARCHIVE" >"$TMP/members.v" 2>/dev/null || { check_signal "$?"; die 5 "$ARCHIVE is not a watane archive" "$NS_MSG"; }
  check_signal
  if grep -q '^[lh]' "$TMP/members.v"; then
    check_signal "$?"
    die 5 "$ARCHIVE holds a symbolic or hard link, which a watane archive never does" "$NS_MSG"
  else
    check_signal "$?"
  fi
  mkdir "$TMP/x" || { ea_rc=$?; check_signal "$ea_rc"; exit "$ea_rc"; }
  check_signal "$?"
  tar -xzf "$TMP/$ARCHIVE" -C "$TMP/x" || { check_signal "$?"; die 5 "$ARCHIVE could not be unpacked" "$NS_MSG"; }
  check_signal
  TOP=$TMP/x/watane-$RELEASE-$TARGET
  ea_v=""
  if [ -f "$TOP/VERSION" ]; then IFS= read -r ea_v <"$TOP/VERSION" || { check_signal "$?"; }; fi
  [ "$ea_v" = "$RELEASE" ] ||
    die 5 "$ARCHIVE is not a watane archive of version $RELEASE" "$NS_MSG"
  for ea_r in $RELS; do
    exists "$TOP/$ea_r" ||
      die 5 "$ARCHIVE is not a watane archive: $ea_r is missing" "$NS_MSG"
  done
  return 0
}

# note_created: CREATED lists the folders this run is about to create, shallowest first (3.6 step 1)
note_created() {
  CREATED=""
  for nc_f in "$HOME/.local" "$HOME/.local/bin" "$HOME/.local/share" "$HOME/.local/share/watane" "$HOME/.local/share/watane-dev" "$HOME/Applications"; do
    if exists "$nc_f"; then :; else CREATED=$(add_item "$CREATED" "$nc_f"); fi
  done
  return 0
}

# remove_created: an install that did not commit takes away the empty folders it made (3.6 step 4)
remove_created() {
  [ "$COMMITTED" != 1 ] || return 0
  for rc_f in "$HOME/Applications" "$HOME/.local/share/watane-dev" "$HOME/.local/share/watane" "$HOME/.local/share" "$HOME/.local/bin" "$HOME/.local"; do
    in_list "$CREATED" "$rc_f" || continue
    # A link swapped in since note_created must not be followed out of the home (3.4.1)
    rc_r=$(cd "$rc_f" 2>/dev/null && pwd -P) || { check_signal "$?"; continue; }
    check_signal "$?"
    [ ! -L "$rc_f" ] || continue
    [ "$rc_r" = "$HOME_R/${rc_f#"$HOME/"}" ] || continue
    rmdir "$rc_r" 2>/dev/null || { check_signal "$?"; }
  done
  return 0
}

stage_files() {
  note_created
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
    printf '%s\n' "$FOLDERS_RESOLVED"
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
  # Compare with step 4 before moving any entry (3.6 step 3).
  for sw_f in "$HOME/.local/bin" "$HOME/.local/share/watane" "$HOME/.local/share/watane-dev" "$HOME/Applications"; do
    resolve_dir "$sw_f" || { check_signal "$?"; }
    if ! in_list "$FOLDERS_RESOLVED" "resolved ${sw_f#"$HOME/"} $RES"; then
      sw_now=$(tilde_r "$RES")
      [ "$RES" != "$HOME_R" ] || sw_now="your home folder itself"
      SWAP_CHANGED="$(tilde "$sw_f") changed during the install: it now resolves to $sw_now"
      return 1
    fi
  done
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
  COMMITTED=1
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
  SWAP_CHANGED=""
  while IFS= read -r if_p; do
    for if_k in new old; do
      if exists "$if_p.$if_k.$$"; then
        die 5 "$(tilde "$if_p.$if_k.$$") is in the way" "Remove it, then run this again."
      fi
    done
  done <<EOF
$ENTRIES_ALL
EOF
  # fresh: no entry of an installed version exists before the swap (3.8)
  if_fresh=1
  while IFS= read -r if_p; do
    if exists "$if_p"; then if_fresh=0; fi
  done <<EOF
$ENTRIES_ALL
EOF
  if do_swap; then :; else
    if_rc=$?
    # Save the old paths before undo retires the journal. An old path that is
    # missing, or whose `.old.` name remains, means the previous version is not
    # whole. The receipt is copied, not moved, and stays old until the swap ends.
    if_old=""
    if_new=""
    if_moved=""
    if [ -f "$SHAREW/journal.$$" ]; then
      read_journal "$SHAREW/journal.$$"
      if_old=$J_OLD
      if_new=$J_NEW
      if_moved=$J_MOVED
    fi
    if_undo=0
    rollback || { if_undo=$?; check_signal "$if_undo"; }
    if [ "$if_undo" -ne 0 ]; then
      rollback || { check_signal "$?"; }
    fi
    check_signal "$if_rc"
    if [ "$COMMITTED" = 1 ]; then
      # the receipt moved before the failure: undo finished the commit (3.8, 3.9)
      printf 'watane install: a step after the install failed\n' >&2
      interrupted_hint failed
      exit 5
    fi
    # a new entry still at its path after undo: undo failed to take it out (3.8)
    if_left=0
    while IFS= read -r if_p; do
      [ -n "$if_p" ] || continue
      if in_list "$if_moved" "$if_p" && exists "$if_p"; then if_left=1; fi
    done <<EOF
$if_new
EOF
    if [ "$if_left" = 1 ]; then
      if [ "$if_fresh" = 1 ]; then
        die 5 "could not put the files in place" "The files could not be removed. Run this script again: it removes them and installs watane."
      fi
      die 5 "could not put the files in place" "The files could not be put back. Run this script again to restore the installed version."
    fi
    while IFS= read -r if_p; do
      [ -n "$if_p" ] || continue
      if ! exists "$if_p" || { [ "$if_p" != "$RECEIPT" ] && exists "$if_p.old.$$"; }; then
        die 5 "could not put the files in place" "The files could not be put back. Run this script again to restore the installed version."
      fi
    done <<EOF
$if_old
EOF
    if_msg="Nothing was changed."
    if [ "$if_fresh" = 1 ]; then if_msg="Nothing was installed."; fi
    if [ -n "$SWAP_CHANGED" ]; then
      die 5 "$SWAP_CHANGED" "$if_msg"
    fi
    die 5 "could not put the files in place" "$if_msg"
  fi
  return 0
}

# read_answer: one terminal line, preserving NUL as a rejected byte (3.7, 8.4).
# Encode before shell substitution: macOS sh's read silently drops NUL bytes.
read_answer() {
  ANSWER=""
  ANSWER_EOF=0
  ra_bad=0
  while :; do
    # Keep both statuses: a pipeline would hide dd's signal status behind od.
    # dd must read exactly one byte; od alone may read ahead past that byte.
    if [ -z "$TMP" ]; then
      TMP=$(mktemp -d "${TMPDIR:-/tmp}/watane-install.XXXXXX") || { check_signal "$?"; die 1 "cannot make a temporary folder in ${TMPDIR:-/tmp}"; }
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
      '') ANSWER=""; ANSWER_EOF=1; return 0 ;;
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

# rustup_src: RUSTUP_SRC is where offer A downloads from (3.7)
rustup_src() {
  RUSTUP_SRC=https://sh.rustup.rs
  if [ -n "${WATANE_RUSTUP_URL:-}" ]; then RUSTUP_SRC=$WATANE_RUSTUP_URL; fi
  return 0
}

doctor_command() {
  # shellcheck disable=SC2088 # Print the home shorthand.
  case ":${PATH:-}:" in
    *":$PREFIX/bin:"*) DOCTOR='watane-dev doctor' ;;
    *) DOCTOR='~/.local/bin/watane-dev doctor' ;;
  esac
}

print_commands() {
  say 'To install it later, run:'
  case $TC_STATE in
    none_a)
      rustup_src
      pc_url=$RUSTUP_SRC
      # unquoted only as an http(s) URL of the safe characters; any other is quoted (3.7)
      case $pc_url in
        '' | *[!A-Za-z0-9._/:@%+,=~-]*) pc_url=$(shell_quote "$pc_url") || { check_signal "$?"; } ;;
        https://* | http://*) ;;
        *) pc_url=$(shell_quote "$pc_url") || { check_signal "$?"; } ;;
      esac
      # the offer downloads without --proto when WATANE_RUSTUP_URL is set (3.2)
      pc_proto="--proto '=https' "
      if [ -n "${WATANE_RUSTUP_URL:-}" ]; then pc_proto=""; fi
      say "  curl ${pc_proto}--tlsv1.2 -sSf $pc_url | sh -s -- -y --profile minimal --no-modify-path -t wasm32-wasip2"
      ;;
    target) say '  rustup target add wasm32-wasip2' ;;
    none_b)
      say '  rustup toolchain install stable --profile minimal -t wasm32-wasip2'
      say '  rustup default stable'
      ;;
  esac
  doctor_command
  say "Then \`$DOCTOR\` checks it again."
  return 0
}

# rustup_step <step description> <command...>: run it, or end with exit 6
rustup_step() {
  rs_step=$1
  shift
  say "--- rustup's own output follows ---"
  TC_RUNNING=1
  : >"$TMP/rustup-status" || { check_signal "$?"; return 1; }
  # Stream both descriptors together. POSIX pipelines return the last command's
  # status, so the producer records rustup's status separately.
  (
    rs_rc=0
    ( cd "$HOME" 2>/dev/null || cd /; RUSTUP_AUTO_INSTALL=0; export RUSTUP_AUTO_INSTALL; "$@" ) || rs_rc=$?
    printf '%s\n' "$rs_rc" >"$TMP/rustup-status"
  ) 2>&1 | tee "$TMP/rustup-output" || { check_signal "$?"; return 1; }
  check_signal
  IFS= read -r rs_rc <"$TMP/rustup-status" || { check_signal "$?"; return 1; }
  check_signal "$rs_rc"
  rs_last=$(tail -c 1 "$TMP/rustup-output") || { check_signal "$?"; return 1; }
  check_signal "$?"
  if [ -n "$rs_last" ]; then say ''; fi
  say "--- end of rustup's output ---"
  TC_RUNNING=0
  if [ "$rs_rc" != 0 ]; then
    warn "the toolchain step failed: $rs_step (rustup exited $rs_rc)"
    printf 'watane %s is installed.\n' "$RELEASE" >&2
    print_commands >&2
    exit 6
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
  rustup_src
  PROMPT=toolchain
  case $TC_STATE in
    target)
      printf '%s\n%s [y/N] ' \
        'watane-dev needs the wasm32-wasip2 target to build mods, and it is missing.' \
        'Add the wasm32-wasip2 target with rustup?'
      ;;
    none_b)
      printf '%s\n%s [y/N] ' \
        'watane-dev needs the Rust toolchain with the wasm32-wasip2 target to build mods, and it is not installed.' \
        'Install it with rustup (a few hundred MB, into ~/.rustup and ~/.cargo)?'
      ;;
    *)
      printf '%s\n%s [y/N] ' \
        'watane-dev needs the Rust toolchain with the wasm32-wasip2 target to build mods, and it is not installed.' \
        "Install it with rustup (a few hundred MB, from $RUSTUP_SRC, into ~/.rustup and ~/.cargo)?"
      ;;
  esac
  ANSWER_EOF=1
  read_answer </dev/tty || { check_signal "$?"; ANSWER=""; }
  PROMPT=""
  if [ "$ANSWER_EOF" = 1 ]; then say ''; fi
  case $ANSWER in
    [Yy] | [Yy][Ee][Ss]) ;;
    *)
      say 'toolchain: not installed'
      print_commands
      return 0
      ;;
  esac
  case $TC_STATE in
    none_a)
      TC_DOWNLOADING=1
      if [ -n "${WATANE_RUSTUP_URL:-}" ]; then
        say "using WATANE_RUSTUP_URL=$WATANE_RUSTUP_URL"
        download "$RUSTUP_SRC" "$TMP/rustup-init.sh" plain
      else
        download "$RUSTUP_SRC" "$TMP/rustup-init.sh" secure
      fi
      TC_DOWNLOADING=0
      rustup_step 'install Rust with rustup' sh "$TMP/rustup-init.sh" -y --profile minimal --no-modify-path -t wasm32-wasip2 </dev/null
      say 'rustup put its programs in ~/.cargo/bin; watane-dev finds them there. To use cargo yourself, add that folder to your PATH.'
      ;;
    target)
      rustup_step 'rustup target add wasm32-wasip2' "$RUSTUP" target add wasm32-wasip2
      ;;
    none_b)
      rustup_step 'rustup toolchain install stable --profile minimal -t wasm32-wasip2' "$RUSTUP" toolchain install stable --profile minimal -t wasm32-wasip2
      rustup_step 'rustup default stable' "$RUSTUP" default stable
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
  take_lock
  check_folders
  say "watane $RELEASE: installing for $TARGET"
  TMPBASE=${TMPDIR:-/tmp}
  TMP=$(mktemp -d "$TMPBASE/watane-install.XXXXXX") || { check_signal "$?"; die 1 "cannot make a temporary folder in $TMPBASE" "Set TMPDIR to a folder you can write to, then run this again."; }
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
  ds_lock=$HOME/.watane-install.lock
  ds_has_lock=0
  ds_has_left=0
  ds_seen=""
  CS_REPORTED=0
  if exists "$ds_lock"; then
    ds_has_lock=1
    ds_suffix=$(lock_pid_suffix "$ds_lock")
    check_signal "$?"
    say "install lock: $(tilde "$ds_lock")$ds_suffix"
  fi
  read_receipt "$RECEIPT"
  ds_installed=$R_OK
  if [ "$ds_installed" = 1 ]; then
    say "watane $R_VERSION ($R_TARGET)"
    while IFS= read -r ds_l; do
      ds_p=${ds_l#path }
      if valid_entry "$ds_p"; then
        say "$(tilde "$ds_p")"
        ds_seen=$(add_item "$ds_seen" "$ds_p")
      else
        check_signal "$?"
      fi
    done <<EOF
$R_LINES
EOF
  fi
  if allowed "$RECEIPT"; then
    find_journals
    while IFS= read -r ds_n; do
      [ -n "$ds_n" ] || continue
      pid_gone "$ds_n" || continue
      read_recovery_receipt "$ds_n"
      recovery_allowed "$RECEIPT" "$ds_n" "$R_RESOLVED" || continue
      say "leftover: $(tilde "$SHAREW/journal.$ds_n")"
      ds_has_left=1
      if exists "$RECEIPT.new.$ds_n"; then
        ds_locations=$R_RESOLVED
        read_receipt "$RECEIPT.new.$ds_n"
        [ "$R_OK" = 1 ] || continue
        read_journal "$SHAREW/journal.$ds_n"
        while IFS= read -r ds_p; do
          in_list "$R_LINES" "path $ds_p" || continue
          recovery_allowed_both "$ds_p" "$ds_n" "$ds_locations" 1 "$R_RESOLVED" || continue
          exists "$ds_p.new.$ds_n" && continue
          exists "$ds_p" || continue
          if ! in_list "$J_NEW" "$ds_p" && ! exists "$ds_p.old.$ds_n"; then continue; fi
          # a path the receipt already listed above is not listed again
          if in_list "$ds_seen" "$ds_p"; then continue; fi
          ds_seen=$(add_item "$ds_seen" "$ds_p")
          say "leftover: $(tilde "$ds_p")"
        done <<EOF
$J_MOVED
EOF
      fi
    done <<EOF
$JOURNALS
EOF
    clean_stale report
    if [ "$CS_REPORTED" = 1 ]; then ds_has_left=1; fi
  fi
  # the script is named only where it is: a kill before its entry was swapped leaves none
  if [ -f "$SHAREW/install.sh" ] && [ ! -L "$SHAREW/install.sh" ]; then
    ds_run="run: sh $(tilde "$SHAREW/install.sh")"
  else
    ds_run="run the install command again"
  fi
  if [ "$ds_has_lock" = 1 ] && [ "$ds_has_left" = 1 ]; then
    say "Next: if no install is running, run: rm -rf $(tilde "$ds_lock"), then $ds_run"
  elif [ "$ds_has_lock" = 1 ]; then
    say "Next: if no install is running, run: rm -rf $(tilde "$ds_lock")"
  elif [ "$ds_has_left" = 1 ]; then
    say "Next: $ds_run (it recovers the interrupted install first)"
  fi
  [ "$ds_installed" = 1 ] || die 7 "not installed: no receipt at $(tilde "$RECEIPT")"
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
  take_lock
  check_no_live_install
  recover prune
  read_receipt "$RECEIPT"
  if [ "$R_OK" != 1 ]; then
    if [ "$RECOVERED_SAID" = 1 ]; then
      die 7 "nothing to uninstall: no receipt at $(tilde "$RECEIPT")" "The files of the interrupted install were removed."
    fi
    die 7 "nothing to uninstall: no receipt at $(tilde "$RECEIPT")"
  fi
  du_ver=$R_VERSION
  du_partial=0
  while IFS= read -r du_l; do
    case $du_l in 'path '*) ;; *) continue ;; esac
    du_p=${du_l#path }
    if receipt_allowed "$du_p"; then
      if exists "$du_p"; then rm_path "$du_p" || { check_signal "$?"; du_partial=1; }; fi
    else
      check_signal "$?"
      du_partial=1
    fi
  done <<EOF
$R_LINES
EOF
  if receipt_allowed "$RECEIPT"; then
    rm -f "$RECEIPT" || { check_signal "$?"; warn "could not remove $(tilde "$RECEIPT")"; du_partial=1; }
  else
    check_signal "$?"
    du_partial=1
  fi
  # Only the two Watane-specific directories are candidates for empty-directory cleanup.
  # Rule 4 must still match, including the location of the receipt itself.
  if receipt_allowed "$RECEIPT" 2>/dev/null; then
    prune_watane_dirs "$R_RESOLVED"
  else
    check_signal "$?"
  fi
  if [ "$du_partial" = 1 ]; then
    du_outcome="watane $du_ver was partly uninstalled"
    say "partly uninstalled watane $du_ver (some entries were skipped or could not be removed)"
  else
    du_outcome="watane $du_ver was uninstalled"
    say "uninstalled watane $du_ver"
  fi
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
    PROMPT=uninstall
    printf 'Delete the watane home %s? It holds your identity and your mods, and cannot be restored without a backup. Type delete to delete it, or press Enter to keep it: ' "$(tilde_r "$HOME_DEL")"
    ANSWER_EOF=1
    read_answer </dev/tty || { check_signal "$?"; ANSWER=""; }
    PROMPT=""
    if [ "$ANSWER_EOF" = 1 ]; then say ''; fi
    if [ "$ANSWER" = delete ]; then
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
  trap 'on_signal 130' INT
  if [ "$EARLY_INT" = 1 ]; then on_signal 130; fi
  trap 'on_signal 129' HUP
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
  main_uid=$(id -u 2>/dev/null) || { check_signal "$?"; die 2 "the tool id is missing" "watane install uses only the tools that stock macOS ships."; }
  check_signal "$?"
  [ "$main_uid" != 0 ] || die 1 "do not run this as root or with sudo" "watane installs into your own home folder and needs no password."
  # step 3: tools
  check_tools "uname id mkdir mv cp rm rmdir ls cat chmod grep awk tr readlink kill dd od mktemp"
  if [ "$MODE" = install ]; then
    check_tools "curl tar tail tee"
    command -v shasum >/dev/null 2>&1 || check_tools sha256sum
  fi
  # step 4: platform, home, folders
  if [ "$MODE" = install ]; then check_platform; fi
  check_home
  set_entries
  check_signal
  case $MODE in
    status) do_status ;;
    uninstall) do_uninstall ;;
    *) do_install ;;
  esac
}

main "$@"
