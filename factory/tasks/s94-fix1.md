Subsystem: install
Effort: medium
Depends on: s94-1
Slice: S94 — a maker runs `cargo test -p watane-install` and it is done in under 2 minutes, proving the same rules

## Goal
The shell unit level proves what the real run does. The adversary (commit 0b20143,
`install/tests/it/adv_s94.rs`) showed that `support/unit.rs` sources `install.sh`'s functions under
`set +e`. The script runs under `set -eu`, so a unit case can pass while the real run exits early
(its red test 8: a `check_folders` mutant whose real run exits 1 with no text). Decision (foreman):
unit cases run under the script's own shell options. Each case runs in its own subshell under
`set -eu` and reports its status, so a `set -e` exit ends only that case and shows as its status.
The guard gaps the adversary found that were already open before S94 also get a default test, at
the unit level where it can show them.

## Acceptance
AC1. `support/unit.rs` runs every case under `set -eu`, the options `install.sh` sets. Each case
     runs in a subshell. A case that `set -e` ends early reports that status, and its output so
     far stays apart from the next case's. A case that relies on `set +e` says so in the case
     itself. All existing `unit::` and `adv_s94::` tests pass under the change. Any whose
     expectation changes is a finding: report it (it means the unit level and the real run
     differed).
AC2. A unit test of `check_folders` with a dangling install-folder link that resolves out of the
     home (as `adv_s94_a_dangling_install_folder_link_out_of_the_home_exits_2`) fails against the
     adversary's mutant 8 (`resolve_dir "$ch_f"; if ! below_home "$RES"; then`). Show it red with
     the mutant applied. `install.sh` is restored after.
AC3. Each default-set test below fails against its named mutant of `install.sh` (apply each mutant
     alone, show it red, then restore). Use the unit level wherever it can show the rule:
     a. `valid_entry` without `allowed`: a dead journal through a relinked `~/.local/bin` must not make
        `undo_swap` remove or move anything outside the home.
     b. `check_foreign` without `if exists "$cf_p.new.$cf_n"; then continue; fi` (3.4.2, INST-S-35: moved and no `.new.<n>`).
     c. `read_receipt` follows a receipt that is a link.
     d. `clean_stale` without `pid_gone`, and without `allowed`; `recover` without `pid_gone`; `undo_swap`
        without the `J_MOVED` check or without `allowed`.
     e. `check_home`: without `plain_path`, and with HOME `/` accepted.
     f. `do_uninstall` without `check_no_live_install`, or without `recover`.
     g. secure download mode: `curl` without `--proto =https --tlsv1.2`, or without `-f` (a fake
        `curl` that records its arguments, then defers to the real one or serves the file).
     h. `check_version` allows `/` in the suffix.
     If a rule needs a whole run, keep it to one run per behaviour.
AC4. The `slow:` reason of `script::inst_s27_an_upgrade_keeps_what_a_receipt_names_beyond_the_install`
     is true: the uninstall goes through `do_uninstall`'s own `receipt_allowed` call, and
     `finish_commit`'s is proven by `adv_s94::adv_s94_an_upgrade_commit_keeps_...`. Name that test.
AC5. Budget: the default set starts at most 2,500 processes and finishes in at most 40 s;
     `--include-ignored` starts at most 11,000 processes and passes (count with
     /tmp/claude-0/-home-user-watane/a44946d2-64a1-5cdb-8457-38d0cfe3594d/scratchpad/count_execs.sh,
     `all` for include-ignored; strace dies about one run in three, so retry). Report the numbers.
AC6. clippy, `cargo +nightly fmt --check` pass. `install.sh` is unchanged at the end.

## Spec
install/TECHNICAL.md 12.1 (the unit level: update it for `set -eu`), 3.4 to 3.8, 8.4, 8.5.
factory/README.md "Test pyramid".

## Interface
None outside install's tests.

## Files
`install/tests/support/unit.rs`, `install/tests/support/mod.rs`, `install/tests/it/unit.rs`,
`install/tests/it/adv_s94.rs`, `install/tests/it/script.rs` (AC4's reason), `install/tests/fakes/*`
(a recording `curl` if needed), `install/TECHNICAL.md` 12.1.

## Changed tests
`unit::*` and `adv_s94::*` run under `set -eu` (AC1). The expectations stay the same unless AC1 shows a real difference.
`script::inst_s27_an_upgrade_keeps_what_a_receipt_names_beyond_the_install`: reason text only.

## Out of scope
Any change to `install.sh`. The pty uninstall `delete` question stays slow-only (pty).
