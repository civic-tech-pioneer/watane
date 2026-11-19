//! Guard status-handling sites against lost signals (install/TECHNICAL 3.8).

// This is a conservative source lint for this script's syntax, not a POSIX parser.
// The scan trusts these helpers' statuses: adding one, or a program whose status
// a helper returns unchecked, needs the same review as an exception.
const CHECKED_HELPERS: &[&str] = &[
    "exists",
    "plain_path",
    "below_home",
    "in_home",
    "in_list",
    "add_item",
    "tilde",
    "tilde_r",
    "allowed",
    "pid_gone",
    "resolve_dir",
    "home_deletable",
];

const ALLOWED: &[(&str, &str)] = &[
    // A signal is already selected; rollback must finish before the handler exits.
    ("on_signal", r###"rollback || :"###),
    // These subshells dispatch pending traps; their own status must not stop dispatch.
    ("check_signal", r###"( : ) || true"###),
    ("cleanup", r###"( : ) || true"###),
    // Both branches check the unnegated substitution status before interpreting it.
    (
        "resolve_dir",
        r###"if rd_r=$(cd "$rd_d" 2>/dev/null && pwd -P); then"###,
    ),
    (
        "detect_toolchain",
        r###"if dt_rustc=$(cd "$HOME" 2>/dev/null || cd /; RUSTUP_AUTO_INSTALL=0; export RUSTUP_AUTO_INSTALL; "$RUSTUP" which rustc 2>/dev/null); then"###,
    ),
    // rs_rc is saved here and checked before the toolchain diagnosis.
    (
        "rustup_step",
        r###"( cd "$HOME" 2>/dev/null || cd /; RUSTUP_AUTO_INSTALL=0; export RUSTUP_AUTO_INSTALL; "$@" ) || rs_rc=$?"###,
    ),
    // Builtin printf encodes a byte; no external program runs in this substitution.
    ("read_answer", r###"ANSWER=$ANSWER$(printf '%b' "\\0$1")"###),
    // This negated probe runs only the ':' builtin with a tty redirection.
    (
        "toolchain_step",
        r###"if ! ( : </dev/tty ) 2>/dev/null; then"###,
    ),
];

// Keep quoted arguments opaque so operators inside them cannot authorize a fallback.
fn shell_tokens(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if escaped {
            word.push(c);
            escaped = false;
        } else if c == '\\' && quote != Some('\'') {
            word.push(c);
            escaped = true;
        } else if quote == Some(c) {
            word.push(c);
            quote = None;
        } else if quote.is_some() {
            word.push(c);
        } else if c == '\'' || c == '"' {
            word.push(c);
            quote = Some(c);
        } else if c.is_whitespace() || ";&|{}()".contains(c) {
            if !word.is_empty() {
                tokens.push(std::mem::take(&mut word));
            }
            if !c.is_whitespace() {
                let mut operator = c.to_string();
                if (c == '&' || c == '|') && chars.peek() == Some(&c) {
                    operator.push(chars.next().expect("peek matched a doubled operator"));
                }
                tokens.push(operator);
            }
        } else {
            word.push(c);
        }
    }
    if !word.is_empty() {
        tokens.push(word);
    }
    tokens
}

fn safe_command(tokens: &[String]) -> bool {
    let tokens = tokens
        .iter()
        .skip_while(|word| matches!(word.as_str(), "if" | "while" | "then" | "!"))
        .skip_while(|word| word.starts_with("IFS="))
        .collect::<Vec<_>>();
    match tokens.first().map(|word| word.as_str()) {
        Some("[") => tokens.iter().any(|word| word.as_str() == "]"),
        Some("read") => true,
        Some("command") => tokens.get(1).is_some_and(|word| word.as_str() == "-v"),
        Some(name) => CHECKED_HELPERS.contains(&name),
        None => false,
    }
}

// Remove only substitutions whose entire command is a known helper. An external
// substitution must instead use a checkpoint or one of the reviewed special sites.
fn helper_substitutions(line: &str) -> Option<String> {
    let mut result = String::new();
    let mut rest = line;
    while let Some(start) = rest.find("$(") {
        result.push_str(&rest[..start]);
        let body = &rest[start + 2..];
        if body.starts_with('(') {
            let end = body.find("))")?;
            if body[..end].contains("$(") {
                return None;
            }
            result.push_str("arithmetic");
            rest = &body[end + 2..];
            continue;
        }
        let mut quote = None;
        let mut escaped = false;
        let mut end = None;
        for (index, c) in body.char_indices() {
            if escaped {
                escaped = false;
            } else if c == '\\' && quote != Some('\'') {
                escaped = true;
            } else if quote == Some(c) {
                quote = None;
            } else if quote.is_none() {
                if c == '\'' || c == '"' {
                    quote = Some(c);
                } else if c == ')' {
                    end = Some(index);
                    break;
                }
            }
        }
        let end = end?;
        // Nested substitutions may run a different program even inside quoted helper args.
        // None are needed here: keep unfamiliar nesting subject to explicit review.
        if body[..end].contains("$(") {
            return None;
        }
        let command = shell_tokens(&body[..end]);
        if !command
            .first()
            .is_some_and(|name| CHECKED_HELPERS.contains(&name.as_str()))
            || command
                .iter()
                .any(|word| matches!(word.as_str(), ";" | "&&" | "||" | "|" | "("))
        {
            return None;
        }
        result.push_str("substitution");
        rest = &body[end + 1..];
    }
    result.push_str(rest);
    Some(result)
}

fn covered_by_rule(line: &str) -> bool {
    let Some(line) = helper_substitutions(line) else {
        return false;
    };
    let tokens = shell_tokens(&line);
    for (index, token) in tokens.iter().enumerate() {
        if token == "||" {
            let rhs = &tokens[index + 1..];
            // No constant: these forward the failed command's status to the caller.
            let preserves = rhs.first().is_some_and(|word| word == "return")
                && rhs
                    .get(1)
                    .is_none_or(|word| matches!(word.as_str(), ";" | "}"));
            let preserves = preserves
                || (rhs.first().is_some_and(|word| word == "exit")
                    && rhs.get(1).is_some_and(|word| word == "\"$?\"")
                    && rhs
                        .get(2)
                        .is_none_or(|word| matches!(word.as_str(), ";" | "}")));
            let start = tokens[..index]
                .iter()
                .rposition(|word| matches!(word.as_str(), ";" | "{" | "}"))
                .map_or(0, |position| position + 1);
            let builtin_left = tokens[start..index]
                .split(|word| matches!(word.as_str(), "&&" | "||" | "|"))
                .all(safe_command);
            if !preserves && !builtin_left {
                return false;
            }
        }
        // A negation anywhere in a condition list drops its pipeline's status.
        if token == "!" && index > 0 && matches!(tokens[index - 1].as_str(), "if" | "&&" | "||") {
            let end = tokens[index + 1..]
                .iter()
                .position(|word| matches!(word.as_str(), ";" | "&&" | "||"))
                .map_or(tokens.len(), |position| index + 1 + position);
            if !tokens[index + 1..end]
                .split(|word| word == "|")
                .all(safe_command)
            {
                return false;
            }
        }
    }
    true
}

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
        if !checked && !ALLOWED.contains(&(function, *line)) && !covered_by_rule(line) {
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

#[test]
fn ac3b_rules_cover_builtins_checked_helpers_and_preserved_statuses() {
    for site in [
        "[ -n \"$value\" ] && [ -f \"$file\" ] || return 1",
        "read -r value || value=\"\"",
        "command -v tool || return 1",
        "if ! allowed \"$different_path\"; then",
        "resolve_dir \"$different_path\" || return 1",
        "value=$(add_item \"$list\" \"$item\")",
        "say \"$(tilde \"$different_path\")\"",
        "tool || return",
        "tool || exit \"$?\"",
    ] {
        assert!(
            unchecked_sites(&format!("new_site() {{\n{site}\n")).is_empty(),
            "rule did not cover {site}"
        );
    }
    for helper in CHECKED_HELPERS {
        assert!(unchecked_sites(&format!("if ! {helper} value; then\n")).is_empty());
    }
    assert!(!unchecked_sites("if ! unknown_helper value; then\n").is_empty());
    assert!(!unchecked_sites("if ! [ -e x ] | tool; then\n").is_empty());
    assert!(!unchecked_sites("if ! [ -e x ] && ! tool; then\n").is_empty());
    assert!(!unchecked_sites("unknown_helper value || return 1\n").is_empty());
    assert!(!unchecked_sites(r#"value=$(add_item "$(external_tool)" item)"#).is_empty());
}

/// 3.8: "negated program predicates" must pass through check_signal and the scan "rejects new
/// unchecked spellings". Each of these negates an external program's status, so a signal it
/// died of is dropped, yet the scan accepts them: `elif` is not a negation context, and lines
/// without `||`, `if !` or `$(` are never read.
#[test]
fn adv_s50b_fix_scan_misses_negated_predicates() {
    let missed: Vec<_> = [
        "elif ! tool; then",
        "if [ -n \"$x\" ] && ! tool; then",
        "while ! tool; do",
        "if  ! tool; then",
        "if\t! tool; then",
        "! tool",
    ]
    .into_iter()
    .filter(|site| unchecked_sites(&format!("new_site() {{\n{site}\n:\n")).is_empty())
    .collect();
    assert!(
        missed.is_empty(),
        "scan accepts unchecked negations: {missed:?}"
    );
}
