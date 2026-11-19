//! Guard status-handling sites against lost signals (install/TECHNICAL 3.8).

// This is a conservative source lint, not a POSIX shell parser. Exact, function-scoped
// exceptions make a newly introduced spelling require review, even for builtin code.
const ALLOWED: &[(&str, &str)] = &[
    // Builtin predicates, EOF tests and formatting substitutions run no external programs.
    ("exists", r###"exists() { [ -e "$1" ] || [ -L "$1" ]; }"###),
    ("resolve_dir", r###"[ -n "$rd_d" ] || rd_d=/"###),
    ("resolve_dir", r###"[ -n "$RES" ] || RES=/"###),
    ("allowed", r###"plain_path "$1" || return 1"###),
    ("allowed", r###"below_home "$RES" || return 1"###),
    (
        "set_entries",
        r###"ENTRIES=$(add_item "$ENTRIES" "$DEST")"###,
    ),
    (
        "set_entries",
        r###"ENTRIES_ALL=$(add_item "$ENTRIES" "$RECEIPT")"###,
    ),
    (
        "read_receipt",
        r###"[ -f "$1" ] && [ ! -L "$1" ] || return 0"###,
    ),
    (
        "read_receipt",
        r###"[ "$rr_l" = 'watane-receipt 1' ] || return 0"###,
    ),
    (
        "read_receipt",
        r###"'path '*) R_LINES=$(add_item "$R_LINES" "$rr_l") ;;"###,
    ),
    (
        "read_journal",
        r###"[ -f "$1" ] && [ ! -L "$1" ] || return 0"###,
    ),
    (
        "read_journal",
        r###"old) J_OLD=$(add_item "$J_OLD" "$rj_p") ;;"###,
    ),
    (
        "read_journal",
        r###"new) J_NEW=$(add_item "$J_NEW" "$rj_p") ;;"###,
    ),
    (
        "read_journal",
        r###"moved) J_MOVED=$(add_item "$J_MOVED" "$rj_p") ;;"###,
    ),
    ("find_journals", r###"exists "$fj_f" || continue"###),
    (
        "find_journals",
        r###"[ -f "$fj_f" ] && [ ! -L "$fj_f" ] || continue"###,
    ),
    (
        "find_journals",
        r###"JOURNALS=$(add_item "$JOURNALS" "$fj_n")"###,
    ),
    ("check_no_live_install", r###"[ -n "$cn_n" ] || continue"###),
    ("clean_stale", r###"exists "$cs_f" || continue"###),
    ("recover", r###"[ -n "$rc_n" ] || continue"###),
    (
        "check_platform",
        r###"[ "$cp_s" = Darwin ] || unsupported "$cp_s $cp_m""###,
    ),
    (
        "check_platform",
        r###"[ "$cp_a" = 1 ] || unsupported "$cp_s $cp_m""###,
    ),
    (
        "check_platform",
        r###"[ "$cp_major" -ge 14 ] || unsupported "macOS $cp_v (needs 14.0 or later)""###,
    ),
    (
        "check_home",
        r###"if ! plain_path "$HOME" || [ "$HOME" = / ]; then"###,
    ),
    (
        "check_folders",
        r###"[ "$RES" != "$HOME_R" ] || ch_m="resolves to your home folder itself""###,
    ),
    ("check_folders", r###"die 2 "$(tilde "$ch_f") $ch_m" \"###),
    (
        "check_foreign",
        r###"if allowed "$cf_p"; then cf_known=$(add_item "$cf_known" "$cf_p"); fi"###,
    ),
    ("check_foreign", r###"[ -n "$cf_n" ] || continue"###),
    ("check_foreign", r###"[ -n "$cf_p" ] || continue"###),
    (
        "check_foreign",
        r###"in_list "$J_MOVED" "$cf_p" || continue"###,
    ),
    (
        "check_foreign",
        r###"cf_known=$(add_item "$cf_known" "$cf_p")"###,
    ),
    (
        "check_foreign",
        r###"if exists "$cf_p"; then cf_found=$(add_item "$cf_found" "$cf_p"); fi"###,
    ),
    (
        "check_foreign",
        r###"if [ "$cf_old_ok" != 1 ] && exists "$RECEIPT"; then cf_found=$(add_item "$cf_found" "$RECEIPT"); fi"###,
    ),
    (
        "check_foreign",
        r###"while IFS= read -r cf_p; do printf '  %s\n' "$(tilde "$cf_p")" >&2; done <<EOF"###,
    ),
    (
        "check_foreign",
        r###"while IFS= read -r cf_p; do printf '  mv %s %s.mine\n' "$(tilde "$cf_p")" "$(tilde "$cf_p")" >&2; done <<EOF"###,
    ),
    (
        "verify_archive",
        r###"while read -r va_hash va_name va_rest || [ -n "$va_hash" ]; do"###,
    ),
    (
        "verify_archive",
        r###"[ "$va_name" = "$ARCHIVE" ] || continue"###,
    ),
    (
        "stage_files",
        r###"! exists "$RECEIPT.new.$$" || return 1"###,
    ),
    (
        "write_journal",
        r###"! exists "$SHAREW/journal.$$" || return 1"###,
    ),
    ("read_answer", r###"ANSWER=$ANSWER$(printf '%b' "\\0$1")"###),
    (
        "toolchain_step",
        r###"if ! ( : </dev/tty ) 2>/dev/null; then"###,
    ),
    (
        "do_install",
        r###"case $di_r in bin/* | share/watane-dev/vendor | Applications/*) say "  $(tilde "$DEST")" ;; esac"###,
    ),
    (
        "do_status",
        r###"case $ds_l in 'path '*) say "$(tilde "${ds_l#path }")" ;; esac"###,
    ),
    ("home_deletable", r###"plain_path "$1" || return 1"###),
    ("home_deletable", r###"plain_path "$hd_r" || return 1"###),
    (
        "home_deletable",
        r###"if ! in_home "$hd_r"; then HOME_OUT=1; return 1; fi"###,
    ),
    (
        "do_uninstall",
        r###"if exists "$HOME/.rustup" || exists "$HOME/.cargo"; then"###,
    ),
    ("do_uninstall", r###"[ -e "$du_h" ] || return 0"###),
    (
        "do_uninstall",
        r###"say "kept: $(tilde "$du_h") (not deleted: it resolves to $HOME_DEL, outside your home folder)""###,
    ),
    (
        "do_uninstall",
        r###"say "kept: $(tilde "$du_h") (not deleted: not a folder that watane can delete)""###,
    ),
    (
        "do_uninstall",
        r###"printf 'Delete the watane home %s? It holds your identity and your mods, and cannot be restored without a backup. Type delete to delete it, or press Enter to keep it: ' "$(tilde_r "$HOME_DEL")""###,
    ),
    ("do_uninstall", r###"say "deleted: $(tilde "$du_h")""###),
    (
        "do_uninstall",
        r###"say "kept: $(tilde "$du_h") (your home: identity, mods, channels)""###,
    ),
    ("main", r###"[ "$MODE" = install ] || bad_flag "$1""###),
    (
        "main",
        r###"command -v shasum >/dev/null 2>&1 || check_tools sha256sum"###,
    ),
    // These predicates check their program statuses internally before returning a boolean.
    ("allowed", r###"resolve_dir "$al_root" || return 1"###),
    ("allowed", r###"resolve_dir "${1%/*}" || return 1"###),
    ("finish_commit", r###"allowed "$fc_p" || continue"###),
    ("clean_stale", r###"allowed "$cs_e" || continue"###),
    ("clean_stale", r###"pid_gone "$cs_n" || continue"###),
    ("recover", r###"if ! allowed "$RECEIPT"; then"###),
    ("recover", r###"pid_gone "$rc_n" || continue"###),
    (
        "check_folders",
        r###"if ! resolve_dir "$ch_f" || ! below_home "$RES"; then"###,
    ),
    ("check_foreign", r###"pid_gone "$cf_n" || continue"###),
    ("do_uninstall", r###"if ! home_deletable "$du_h"; then"###),
    // Undo retains SIGNAL=0; these exact calls propagate status to the parent checkpoint.
    (
        "undo_swap",
        r###"if ! exists "$RECEIPT.new.$1"; then finish_commit "$1" || exit "$?"; exit 0; fi"###,
    ),
    ("undo_swap", r###"[ -n "$ub_p" ] || continue"###),
    ("undo_swap", r###"exists "$ub_p.old.$1" || continue"###),
    (
        "undo_swap",
        r###"if [ "$ub_p" != "$RECEIPT" ]; then rm_path "$ub_p" || exit "$?"; fi"###,
    ),
    ("undo_swap", r###"mv "$ub_p.old.$1" "$ub_p" || exit "$?""###),
    ("undo_swap", r###"in_list "$J_MOVED" "$ub_p" || continue"###),
    ("undo_swap", r###"mv "$ub_p" "$ub_p.new.$1" || exit "$?""###),
    ("undo_swap", r###"exists "${ub_p%/*}" || continue"###),
    ("undo_swap", r###"mkdir "$ub_p.new.$1" || exit "$?""###),
    (
        "undo_swap",
        r###"rm -f "$SHAREW/journal.$1" || exit "$?""###,
    ),
    (
        "undo_swap",
        r###"if exists "$ub_p.new.$1"; then rm_path "$ub_p.new.$1" || exit "$?"; fi"###,
    ),
    (
        "undo_swap",
        r###"if exists "$RECEIPT.old.$1"; then rm_path "$RECEIPT.old.$1" || exit "$?"; fi"###,
    ),
    ("undo_swap", r###"allowed "$ub_p" || continue"###),
    // Swap and cleanup helpers return the original status for the caller to check.
    ("rollback", r###"undo_swap "$SWAP_PID" || return"###),
    ("stage_files", r###"mkdir -p "${DEST%/*}" || return"###),
    (
        "stage_files",
        r###"cp -R "$TOP/$sf_r" "$DEST.new.$$" || return"###,
    ),
    (
        "stage_files",
        r###"bin/* | share/watane/install.sh) chmod 755 "$DEST.new.$$" || return ;;"###,
    ),
    ("stage_files", r###"mkdir -p "$SHAREW" || return"###),
    (
        "swap_entries",
        r###"if exists "$sw_p"; then mv "$sw_p" "$sw_p.old.$$" || return; fi"###,
    ),
    (
        "swap_entries",
        r###"printf 'moved %s\n' "$sw_p" >>"$sw_j" || return"###,
    ),
    ("swap_entries", r###"mv "$sw_p.new.$$" "$sw_p" || return"###),
    (
        "swap_entries",
        r###"if exists "$RECEIPT"; then cp -p "$RECEIPT" "$RECEIPT.old.$$" || return; fi"###,
    ),
    (
        "swap_entries",
        r###"printf 'moved %s\n' "$RECEIPT" >>"$sw_j" || return"###,
    ),
    (
        "swap_entries",
        r###"mv "$RECEIPT.new.$$" "$RECEIPT" || return"###,
    ),
    ("do_swap", r###"stage_files || return"###),
    ("do_swap", r###"write_journal || return"###),
    ("do_swap", r###"swap_entries || return"###),
    (
        "finish_commit",
        r###"if exists "$fc_p"; then rm_path "$fc_p" || return; fi"###,
    ),
    (
        "finish_commit",
        r###"if exists "$fc_p.old.$1"; then rm_path "$fc_p.old.$1" || return; fi"###,
    ),
    (
        "finish_commit",
        r###"if exists "$fc_old"; then rm_path "$fc_old" || return; fi"###,
    ),
    ("clean_stale", r###"rm_path "$cs_f" || return"###),
    // Trap dispatch points and an already selected signal must not interrupt rollback.
    ("on_signal", r###"rollback || :"###),
    ("check_signal", r###"( : ) || true"###),
    ("cleanup", r###"( : ) || true"###),
    // resolve_dir checks both branches before interpreting cd/pwd's result.
    (
        "resolve_dir",
        r###"if rd_r=$(cd "$rd_d" 2>/dev/null && pwd -P); then"###,
    ),
    // Detection checks both branches; rustup_step saves and checks rs_rc before diagnosis.
    (
        "rustup_step",
        r###"( cd "$HOME" 2>/dev/null || cd /; RUSTUP_AUTO_INSTALL=0; export RUSTUP_AUTO_INSTALL; "$@" ) || rs_rc=$?"###,
    ),
    (
        "detect_toolchain",
        r###"if dt_rustc=$(cd "$HOME" 2>/dev/null || cd /; RUSTUP_AUTO_INSTALL=0; export RUSTUP_AUTO_INSTALL; "$RUSTUP" which rustc 2>/dev/null); then"###,
    ),
];

fn unchecked_sites(source: &str) -> Vec<String> {
    let lines: Vec<_> = source.lines().map(str::trim).collect();
    let mut function = "";
    let mut unchecked = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if let Some((name, _)) = line.split_once("() {") {
            function = name;
        }
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let candidate =
            line.contains("||") || line.contains("if !") || line.replace("$((", "").contains("$(");
        if !candidate {
            continue;
        }
        // Inline checks consume the status before fallback assignment or remapping.
        // die's first command must do the same (asserted below), including a continued line.
        let fallback = line.rsplit_once("||").map(|(_, rhs)| rhs.trim());
        let inline_check = fallback.is_some_and(|rhs| {
            if rhs.starts_with("{ check_signal \"$?\";") || rhs.starts_with("die ") {
                return true;
            }
            // A named status is safe only when captured first and checked next.
            rhs.strip_prefix("{ ").is_some_and(|body| {
                body.split_once("=$?;").is_some_and(|(name, rest)| {
                    !name.is_empty()
                        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                        && rest
                            .trim_start()
                            .starts_with(&format!("check_signal \"${name}\";"))
                })
            })
        });
        let checked = inline_check
            || (line.ends_with("||")
                && lines.get(index + 1).is_some_and(|next| next.starts_with("die ")))
            // Only a standalone assignment exposes its substitution's status unchanged.
            || (!line.contains("||") && !line.starts_with("if ")
                && line.split_once("$(").is_some_and(|(prefix, _)| prefix.ends_with('='))
                && lines.get(index + 1).is_some_and(|next| *next == "check_signal \"$?\""));
        if !checked && !ALLOWED.contains(&(function, *line)) {
            unchecked.push(format!("{} ({function}): {line}", index + 1));
        }
    }
    unchecked
}

#[test]
fn ac3b_status_remapping_requires_a_signal_check() {
    let source = include_str!("../install.sh");
    assert!(source.contains("die() {\n  check_signal \"$?\"\n"));
    assert!(source.contains("    SIGNAL=0\n"));
    let unchecked = unchecked_sites(source);
    assert!(
        unchecked.is_empty(),
        "unchecked status sites:\n{}",
        unchecked.join("\n")
    );

    // Prove the scan rejects each new unchecked form instead of just pinning today's text.
    for site in [
        "tool || exit 1",
        "tool || return 1",
        "tool || true",
        "tool || true; check_signal \"$?\"",
        "if ! tool; then check_signal \"$?\"; fi",
        "tool || value=\"\"",
        "if ! tool; then",
        "value=$(tool)",
        "say \"$(tool)\"",
    ] {
        assert!(
            !unchecked_sites(&format!("new_site() {{\n{site}\n:\n")).is_empty(),
            "missed {site}"
        );
    }
    // Exact exceptions must not hide a changed program or a new constant status.
    for (from, to) in [
        ("$(add_item", "$(external_tool"),
        ("|| exit \"$?\"", "|| exit 1"),
        (
            "read_answer </dev/tty || { check_signal \"$?\"; ANSWER=\"\"; }",
            "read_answer </dev/tty || ANSWER=\"\"",
        ),
    ] {
        assert!(
            !unchecked_sites(&source.replace(from, to)).is_empty(),
            "missed mutation {to}"
        );
    }
    // A check after `|| true` is too late: it would see true's zero.
    assert!(!unchecked_sites("tool || true\ncheck_signal \"$?\"\n").is_empty());
}
