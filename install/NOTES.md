# install: build notes (a record, not a contract)

S50b-1 built `install.sh` (stage 1 of TECHNICAL 11) and `tests/script.rs`. Where the first draft of
the specs did not fit, TECHNICAL.md was fixed in place; the changes:

- **No progress bar** (3.5): curl's stderr goes to a file so that the script's first stderr line is
  its own, so a bar had nowhere to show.
- **`--uninstall` and `--status` skip the folder check of 3.4.1** (3.2): INST-S-31's last case needs an
  uninstall to run with `~/.local/share/watane-dev` turned into a link; 8.5 rule 3 is what stops it
  from removing through that link. The validator found the receipt, the journals and their
  `.old`/`.new` names still removed through a link at `share/watane`; they now pass 8.5 too.
- **`<target>` of 3.4.1** for a folder that does not exist yet is its nearest parent's resolved path
  plus the missing part (`<second>/watane`), so the message names where a write would land.
- **`gzip` in the test `PATH`** (12.1): GNU tar runs `gzip` to read a `.tar.gz`; macOS's tar does not.
  The script does not name it. The symlink dir of the tests holds it for Linux only.
- **Fake `mv` and `rm`** (12.1) match a shell glob from `mv_fault` / `rm_fault`, not a bare word: the
  glob `*watane.app.new.*` hits the move into place and not the undo's `mv watane.app.old.<pid>`.
- **`hakari`**: `cargo hakari generate` was run (it reported no change); `install` depends on
  `workspace-hack` like the other crates, besides the two dev-dependencies of AC1.
- The swap, its undo, signals and recovery were also tried by hand with the fake `mv` (failure and
  sleep plus `KILL` at the receipt and at the app; `TERM` at the app); INST-S-32 to 35 are S50b-3's.
- Shellcheck is not installed in the cloud sandbox; the script passes `sh -n` and `dash -n`.

S50b-3 built `package.sh`, `tests/package.rs` and INST-S-32 to 35, and the hand-built bad archive of AC6. Notes:

- **`package.sh` finds its inputs from its own place** (4.2): `<checkout>/target/release/...` and
  `<checkout>/dev/scripts/vendor.sh`, so the test needs no variable or flag: it copies the script into
  a miniature checkout. A refusal says `package.sh: missing part: <what>`.
- The tests that were not in S-33's text but catch hand mutants of `install.sh`: a kill at the receipt
  of a fresh install (the journal part of the foreign-file check, 3.4.2); a kill while `rm` removes a
  path that only the old receipt held (`cp -p receipt receipt.old`, `finish_commit` in `recover`);
  `TERM` at the app (the undo in `on_signal`); the live pid of S-35 (`kill -0`). Each mutant turns
  one of them red.
- AC4 puts both versions' lines in one `SHA256SUMS`, since the fake release is one server for both.

S50b-fix1: the closing `rmdir` loop of `do_uninstall` resolves each folder (`resolve_dir`) and removes it only when it is inside
the resolved home. Toolchain detection and the rustup commands of offer B run in a subshell from `$HOME` with
`RUSTUP_AUTO_INSTALL=0` (3.7): rustup reads `rust-toolchain.toml` from the cwd. The fake rustup logs `pwd` and
`RUSTUP_AUTO_INSTALL` per call in `rustup.pwd` / `rustup.auto`.

S50b-fix2: the adversary's five RISK findings closed with the stricter option (IN-34 to IN-38). The pid of the
script in a test is the `Command` child (perl execs sh), and a fake `chmod` (`chmod_wait`/`chmod_waiting` in the
state folder) holds the staging so a test can plant a link at `journal.<pid>` first. `home_deletable` sets
`HOME_OUT` for the outside-home case; the question shows the resolved path through `tilde_r`; the `deleted:` and `kept:` lines keep the home as given.

S50b-fix3: install folders must resolve strictly below the resolved home (`below_home`; `in_home` stays for the home-to-delete check). A link to the home itself gave rule 3 the home's top level (IN-39). Links to other folders inside the home still work (INST-S-31 last case).
A folder that resolves to the home itself is refused with its own first line (`resolves to your home folder itself`); INST-S-31 last case now also upgrades and uninstalls through the dotfiles link.

## S50b fix4: SIGPIPE

A closed reader (`curl | sh | head`) kills the shell at its next write without the `EXIT` trap, leaving the temporary folder. `PIPE` is now trapped like `HUP`/`INT`/`TERM` (exit 141), and the handler ignores `PIPE` so its own writes cannot die. The "quit it and open it again" line is printed only for an upgrade (old version differs), not a reinstall (3.6 step 5).

## S50b-fix: macOS test load and shellcheck

Endpoint security (ManageEngine app control and Microsoft Defender) checks each exec:
the foreman measured 0.08 seconds for an idle shell, 5 seconds under 16 test
threads, and 25–35 seconds for fake uname. The old 20/30-second waits failed
with empty output under load, although the script was correct. Test support
now shares a 600-second deadline and a Mutex/Condvar gate admitting four live
plain, pty or package runs per binary. Admission happens before spawning and
before callers start their waits; a permit remains held through completion.

The archive expectation includes the four files added by codesign only when
codesign is on PATH (IN-40). Shellcheck findings were fixed by removing unused
HAD_RECEIPT, spelling empty CDPATH explicitly, and documenting literal printed
commands/home shorthand. Character classes in tr still map Y/YES to y/yes.

## S50b-fix: group KILL can race rollback on macOS

The missing journal after `kill_at` was caused by non-atomic signal delivery
to the process group, not a stale fake-tool marker or the run gate. Killing
the fake mv could wake the installer before KILL reached the installer. Its
failed move then ran the ordinary rollback, removing the journal and staged
files. Endpoint-security delays and scheduling under load exposed this window.

A small parent-shell/child-sleeper regression reproduced it under concurrent
`/bin/sh -c true` load: the child was reported as `Killed: 9`, but the parent
ran its EXIT cleanup and exited 1 instead of dying from KILL. Extra background
sleepers widen the delivery window; repeated rounds exercise the race.
`signal_group` now sends STOP to the leader PID before KILL to the group, in
one sender shell. The leader cannot run rollback when its child dies. Other
signals retain their ordinary group delivery so TERM/PIPE still test rollback.
No installer, fake-tool behavior, deadline, gate or existing assertion changes.

The four-worker exec-load run of the three original cases passed before the
fix (1080 seconds), so it did not reproduce the foreman's exact failures.
The smaller fixture did reproduce the underlying delivery race twice; one
captured run failed in 23.57 seconds with parent exit 1 and child `Killed: 9`.
With the fix, all 16 rounds passed under that same background exec load
(472.34 seconds). This distinguishes evidence of the race from the
intermittent higher-level symptom.

## S50b-fix: a test run that inherits an ignored INT

`inst_s11_interrupted_during_the_download` hung for over an hour in a check run, with `curl`
still connected to the stalled release server. A shell that starts a job in the background
ignores INT for it, the ignore survives `exec`, and a non-interactive `sh` cannot trap a
signal it was started with ignored: so the test binary, `perl`, `install.sh` and its `curl`
all ignored the test's INT and `finish` waited for ever. Reproduced by starting the test under
`perl -e '$SIG{INT}="IGNORE"; exec @ARGV'`. The script is right (a person's Ctrl-C arrives
with INT at its default). The launcher (`support::LAUNCH`) now sets HUP, INT, QUIT, TERM and
PIPE to their defaults after `setsid`, and `finish` kills the run's process group when the run
has not ended and closed its pipes within the 600-second deadline, so no left-behind process
can hold a test for ever.

## S50b-fix-2: match toolchain consent as raw bytes

The toolchain answer now uses shell case patterns for `y` and `yes` in any
case, without passing the answer through `tr`. Invalid UTF-8 cannot become
a yes prefix through partial conversion. The home deletion question already
matches the literal `delete` directly and needs no change.

The UTF-8-locale PTY regression typed `y\xffnope` and failed before the fix
with `tr: Illegal byte sequence` and exit 1 on macOS. A second PTY test types
`YeS` and checks the installer invocation. The PTY helper gained raw-byte
input while preserving the existing string-answer API and assertions.

## S50b-fix-2 follow-up: TERM rollback and macOS sh's pending traps

The foreman's `inst_s33_term_in_the_swap_undoes_it` exited 143 but left the
upgrade mid-swap. Eight full installer rounds under four concurrent
`/bin/sh -c true` workers passed locally (221.25 seconds), so those runs
alone did not reproduce the reported failure.

A smaller fixture prepares the old/new files and journal directly, then
runs the real `on_signal` and `undo_swap` functions at the same fake-mv
pause. Repeating that fixture under the same exec load reproduced an
unrestored swap in 37.33 seconds. stderr identified the failure:
`run_pending_traps: bad value in trap_list[15]: 0x0`, followed by
`signal handler is SIG_DFL, resending 15 (SIGTERM) to myself`.
The shell was resending TERM during rollback, rather than completing it.
`finish` did not hit its deadline: the test reached its snapshot assertion
with exit 143. The fake-mv sleep glob does not match the `.old` moves used
by rollback.

`on_signal` now registers no-op (`:`) handlers instead of replacing its
active handlers with ignored traps (`trap ''`). This keeps a handler registered while
rollback starts its subshells, avoiding the macOS sh pending-trap failure.
The original snapshot assertions remain; the TERM test now includes the
run output on a snapshot failure. A default prebuilt-journal regression
and an ignored 16-round exec-load regression exercise the actual rollback
functions without a release build or the full install pipeline.

With the no-op handlers, the checked-in 16-round load test passed in
227.71 seconds, and the original TERM and PIPE tests passed under the
concurrent exec load (45.43 and 45.59 seconds). A longer exploratory run
was stopped after that validation and is not counted as a pass.

Final checks: `cargo test -p watane-install` passed (63 passed, 15 ignored),
`cargo clippy --all-targets -- -D warnings`, `cargo +nightly fmt --check`
and `shellcheck -s sh install.sh package.sh` all exited 0. No existing test
assertion was removed or weakened.

## S50b-fix-2 validator: the real cause of the half-undone swap

The `:` handler of the follow-up above did not fix it; `inst_s33_pipe_in_the_swap_undoes_it`
failed the same way in the next check run. The cause is not the handler that `on_signal` sets.
macOS `sh` (bash 3.2) forks a subshell with a trapped signal still pending, resets the trap to
its default in the child, and the child's first trap check kills it ("bad value in
trap_list[15]: 0x0 ... resending 15 (SIGTERM) to myself"); a loop of `( : )` while a background
`kill -TERM $$` lands shows it in about one run of fifteen. The group signal kills the fake `mv`
first, so the swap's `mv` fails and `install_files` runs `undo_swap` in a subshell before the
script's own signal is handled; that subshell dies, and `install_files` had already set
`SWAP_ACTIVE=0`, so `on_signal` saw no swap and exited 141 or 143 with the home half swapped.
The same holds for the `cleanup` path.

`rollback` now clears `SWAP_ACTIVE` only when `undo_swap` ended well, and `install_files`,
`cleanup` and `on_signal` all use it, so the handler that runs after a dead undo undoes again
(`undo_swap` is safe to repeat: the next run's recovery repeats it the same way). `on_signal` is
back to `trap ''`: an ignored signal stays ignored in the undo's subshell, where a `:` handler
becomes the default again. `inst_s33_a_signal_that_kills_the_undo_is_undone_by_the_handler`
pins it without a race: the fake `rm` sends TERM to the undo's subshell and to the script at the
undo's first `rm`; it fails on the script before this fix with exit 143 and the home half swapped.
The ignored `ac1_term_rollback_under_exec_load` is gone: it only repeated the race, which the
test above now pins without one. `inst_s33_int_in_the_swap_undoes_it` joins the TERM and PIPE
tests; under exec load macOS `sh` lost the INT in about one run of three (bash 3.2 ignores an INT
that lands just after the child it waited on died of it), and those runs ended with exit 5 and
the swap undone, which 3.8 now names.

## S50b-fix-3: exact answer bytes and repeatable undo

Both supplied adversary tests failed unchanged before the fix: `y` followed
by NUL ran rustup, and a second undo removed the person's replacement app.
Answers now pass through stock POSIX `dd` and `od` one byte at a time before
entering shell variables. NUL rejects the whole line rather than disappearing;
no trimming or case folding changes the answer. Both questions share this
reader, drain through the newline, and reject EOF and extra bytes. Storage is
bounded to six shell characters, the length of `delete`. The two stock tools were added
to TECHNICAL 3.3 and the tests' restricted PATH; no crate dependency was added.
The required `de\0lete` pty regression keeps the home (this particular case
already passed on the baseline shell); the supplied trailing/prefix NUL
rustup cases demonstrated the byte-loss breach.

Undo now removes the journal after restoring entries and before deleting any
staging name. If journal removal fails, staging evidence remains and undo
reports failure for rollback to retry. A completed receipt move encountered
by rollback finishes commit cleanup instead of leaving a journal behind.
The removal matrix went red on a TERM at journal removal: all staging was
gone but the journal survived the run. It stages a journal directly, then
interrupts each of fifteen removals with TERM (rollback repeats) and KILL
(next run recovers), compares the old home bytes, adds a person's app, and
checks two recovery runs preserve it without journals or staging leftovers.
The first default suite had 51 script tests pass and one fail: the existing
signal-during-undo test expected 143 but got 5 because the sandbox blocks
fake rm's `ps`. The test launcher now supplies its own pid to fake rm, so
TERM still reaches both shells without ps. All existing assertions remain.

An additional prebuilt regression went red after undo had removed a fresh
entry but before journal retirement: recovery deleted a replacement at that
entry. Fresh entries now move back to their `.new` names before cleanup,
so a repeated undo sees durable evidence that they were taken out. An entry
already absent gets an empty staging directory for the same evidence.
The supplied red tests and all prior test assertions remain unchanged.

Final validation on macOS: `cargo test -p watane-install` passed (68 passed,
16 ignored); the supplied NUL adversary and the new home-question NUL test
also passed with `--ignored`. The unchanged repeated-undo adversary, both
AC4 regressions and all original default tests passed. `cargo clippy
--all-targets -- -D warnings`, `cargo +nightly fmt --check`, and
`shellcheck -s sh install.sh package.sh` exited 0. The existing manifest
warnings about workspace-hack remain. No other ignored or GUI tests ran.


## S50b-fix-3 validator return, round 1

Kept validator commit `1451e3b`, including lost-tty denial, the stronger
NUL/empty-marker regressions, and the reverted FUNCTIONAL mechanism text.
A staged fresh-entry journal with both the entry and `.local/bin` removed
reproduced the bare mkdir failure and exit 1. Undo now skips an absent
entry whose parent is also absent, as decided by the foreman: there is
nothing at that path to guard. The regression completes two real installs
from that state and checks no journal or staging names remain.

Other recovery failures remain fatal. Undo, committed cleanup and stale-name
cleanup now propagate failed operations; recovery supplies a clear message
and exit 5 instead of falling through or exposing an unlabelled exit 1.
A prebuilt-journal regression exercises failed marker creation, fresh-entry
rename, old-file restoration, staging removal, journal removal, committed
backup removal and stale-file removal. Every case stops before subsequent
work. Normal post-commit cleanup and uninstall retain their documented
warning behavior; a later recovery stops if the filesystem still fails.

The swap now remains active until post-commit cleanup returns, including
while xattr runs. A fake xattr sleep pauses the actual upgrade after the
receipt commit. The new TERM/INT regression went red with the new version
installed but its journal and old backups left behind; the signal handler
now reaches undo's committed branch to finish cleanup. It checks exit
143/130, the new version in place, and no journal, backups or temporary
folder left. The original adversary tests remain unchanged.

Round 1 validation: `cargo test -p watane-install` passed (71 passed,
16 ignored); the two NUL pty tests passed with `--ignored`, including the
validator's stronger home-question assertions. `cargo clippy --all-targets
-- -D warnings`, `cargo +nightly fmt --check` and `shellcheck -s sh
install.sh package.sh` exited 0. All cargo tests ran without shell
backgrounding. The existing workspace-hack manifest warnings remain.

## S50b-fix: early signals without false causes

The unchanged user-sim regression failed on this Mac before the fix: at
60, 75 and 90 ms it called `0.1.0` an invalid version (exit 1); at 105
and 135 ms it called `id` missing (exit 2). The version's `printf | grep`
and the checksum-line `awk` / `printf | grep` are now builtin checks.
Checksum calculation captures shasum/sha256sum directly, preserving the
program's status instead of losing it through the final awk.

On macOS `/bin/sh` (bash 3.2) and `/bin/dash`, the working combination is
to register traps before parsing, record the signal before staging, and
run `( : ) || true` at a checkpoint before diagnosing a failure. Forking
the empty subshell gives bash a pending-trap dispatch point; a subshell
that itself dies of the pending signal must not bypass the parent's flag
check. A failed program's 129/130/141/143 status is also preserved: an INT
lost by bash after the child's death cannot be recovered from a flag
alone. Cleanup preserves a recorded signal code. During an active swap
the existing immediate signal rollback stays in place.

The unchanged 40-interrupt regression passed three consecutive times on
this Mac (22.21, 24.68 and 24.83 seconds). New prebuilt function fixtures
cover all four signals during id, shasum and tar's list, verbose-list and
extract operations, on both shells; all passed (42.58 seconds). Other
fixtures check signal-status preservation without a delivered parent
signal, builtin-only version and checksum-line checks, and unchanged
genuine bad-version / checksum error lines and codes. The existing
missing-tool and other real-failure assertions remain unchanged. No
dependency or tool was added.

Final validation: `cargo test -p watane-install` passed (78 passed,
16 ignored, 601.43 seconds); `cargo clippy --all-targets -- -D warnings`,
`cargo +nightly fmt --check` and `shellcheck -s sh install.sh package.sh`
all exited 0. Clippy retained the existing workspace-hack manifest
warnings. No ignored or GUI tests ran.


## s50b-fix-5: immediate signals, pending traps before failure causes

All three unchanged adversary tests went red on this Mac: TERM during
SHA256SUMS still fetched the archive, TERM during mktemp printed the
foreign-file cause, and TERM between the swap checkpoint and clearing
deferral installed and reached the toolchain step. A new builtin-only
fixture also went red because a command after the signal still ran.

Removed SIGNAL_DEFER and its branches. The handler now exits immediately
at every step, with the existing active-swap rollback. Recovery is again
interruptible; its undo evidence remains safe to repeat as in s50b-fix-3.
The pending-trap dispatch and child-status mapping in check_signal remain
before die, checksum and unpack failures. Added dispatch before the
separate foreign-file, bad-flag and incomplete-toolchain causes. A new
fixture went red when a toolchain child's signal status became exit 6;
rustup_step now passes that status to check_signal before its diagnosis.
Removed the redundant partial signal-status mapping in download, which
already calls check_signal for all four signal statuses. No dependency
was added.

Changed tests: adversary test 3's SIGNAL_DEFER=0 anchor disappeared, so
only its injection strings now anchor on SWAP_PID=$$, at the same moment
between do_swap's check_signal and starting the swap. All its assertions
are unchanged: exit 143, no toolchain step, no installed message and no
receipt. Adversary tests 1 and 2 and the user-sim regression are unchanged.
No existing check_signals assertion changed; two comments no longer
refer to deferral. New fixtures cover immediate exit between builtins and
child signal statuses during download and toolchain failure handling.

The unchanged 40-interrupt user-sim regression passed three consecutive
runs on this Mac (21.42, 35.20 and 22.71 seconds). The full check_signals
file passed all 11 tests. Shellcheck, clippy and nightly fmt check exited
0; clippy retains the existing workspace-hack manifest warnings.

Final validation: the single default `cargo test` run passed (86 passed,
16 ignored), including all genuine-failure, rollback and repeated-recovery
tests. No ignored or GUI tests ran. Only the five listed install files
changed; no other subsystem or dependency changed.


## s50b-fix-6: retain signal statuses on ignored results

The two unchanged adversary Ctrl-C tests and three new AC2 tests first
failed on macOS: toolchain detection and xattr continued to success,
sysctl reported an unsupported platform, rmdir continued uninstall, and
the answer pipeline treated the interruption as a declined offer.

Signal checks now see statuses before platform and toolchain probes,
negative grep/xcode-select predicates, readlink fallbacks, ignored removal
failures, extraction mkdir and final cleanup can discard them. Both
post-rustup detection and initial detection stop before a misleading
result. Builtin-only formatting substitutions need no program-status recovery;
shell_quote and its caller now retain the sed pipeline status. Existing acted-on swap failures retain
their rollback behavior. Undo clears the already handled signal in its
subshell so internal probes let it finish; the parent retains the code. No dependency was added.

The answer read uses dd into a temporary byte file, then od, checking both
statuses. A trial with od -N 1 alone consumed the rest of a piped answer,
so dd still limits the actual read. Uninstall lazily creates the temporary
folder for its question and the existing exit trap removes it; mktemp is
therefore checked in every mode. The new pipeline regression interrupts
either original pipeline tool and preserves the raw-byte answer rules.

Foreman decisions recorded in TECHNICAL 3.8: rustup/rustup-init exiting
129/130/141/143 counts as that signal without a toolchain failure line;
final-cleanup interruption may leave the temporary folder; a signal while
a question waits acts when the read returns. Existing adversary tests
and assertions remain unchanged.

Final validation: `cargo test` passed (92 passed, 16 ignored; 620.82 s).
The five focused Ctrl-C tests passed after their observed red run.
`cargo clippy --all-targets -- -D warnings`, `cargo +nightly fmt --check`
and `shellcheck -s sh install.sh package.sh` exited 0. Clippy retained
the existing workspace-hack manifest warnings. No ignored or GUI tests ran.

Validator, round 1: the swap's `|| return 1` (stage_files, swap_entries,
do_swap) still replaced a program's status, so a Ctrl-C during the staging
`cp -R` ended with exit 5 and "could not put the files in place". Those
sites now return the program's own status and install_files passes it to
the signal check after the rollback.

Validator, round 1: `ac4_signals_during_xattr_leave_the_new_version_without_a_journal`
failed once in a full check run (exit 130, temporary folder left). With the
new check right after xattr, `check_signal` can exit while the INT trap is
still pending; the trap then ran at cleanup's dispatch point, after
`trap - EXIT`, and exited before `rm -rf`. `check_signal` now ignores
further signals before it exits, as `on_signal` does.

## s50b-fix-7: retain rollback interruption statuses

Both unchanged adversary undo tests reproduced exit 5 with a cause before
this fix and pass with exit 130 and no cause afterwards. undo_swap now
exits with each failed command's status; rollback returns it and clears
SWAP_ACTIVE only on success. install_files checks rollback's status before
its swap diagnosis, and recover checks undo's status before its diagnosis.
The undo subshell's SIGNAL=0 remains unchanged. Signal-handler rollback
failure cannot override the signal already being handled; EXIT cleanup
checks rollback failures as well.

Whole-script status audit: finish_commit's rm_path calls and clean_stale's
rm_path call now return the helper's status. The two swap journal appends
return printf's status, and stage_files/write_journal retain their output
block's failure status instead of assigning 1. ac3_journal_append_preserves_signal_status
failed with exit 5 before the append fix and checks exit 130 with no cause.
Other constant returns are builtin validation results or follow an explicit
check_signal (resolve_dir, rm_path, finish_commit's journal removal,
home_deletable). Direct `|| die` calls retain the status into die's first
check_signal; no extra check is needed there. No dependency was added.

Validation: the unchanged adversary Ctrl-C tests passed; the AC3 regression
passed after its observed red run. Focused script ac4 tests: 5 passed,
1 ignored. Default cargo test -p watane-install: 89 passed, 16 ignored,
1 failed. The failure is the unchanged
adv_s50b_fix_a_repeated_undo_removes_a_file_it_never_moved assertion at
script.rs:1352: it sends TERM to the undo subshell but expects exit 5;
retaining undo's signal status now correctly yields 143. This contradicts
the task's status-propagation requirement, so the test is left unchanged
for the owner to resolve. Clippy and nightly fmt check exited 0 (existing
workspace-hack manifest warnings only); shellcheck exited 0. No ignored
or GUI tests ran.

### s50b-fix-7 foreman follow-up: AC3b

The check log reproduced only the recorded repeated-undo assertion failure.
The foreman authorized changing that test's exit assertion from 5 to 143:
it sends TERM into undo, whose signal status is now retained. Only that
assertion and its explanatory comment changed; its replacement app and
marker assertions remain unchanged. The focused test passed.

AC3b adds tests/status_audit.rs: a conservative source scan for OR fallback,
negated predicates and command substitutions. It requires the original
status to reach check_signal, directly, via die's checked entry, or through
an exact function-scoped reviewed exception. The commented allow-list covers
builtin predicates/formatting, helpers that check statuses internally,
status-preserving returns (including undo's SIGNAL=0 path), saved rustup
status and signal dispatch points. Mutation checks show new unchecked
forms, changed builtin substitutions, constant undo exits and late checks
are rejected. This is a source lint for the script's current syntax, not a
complete POSIX shell parser; unfamiliar syntax requires review.

The scan first failed on both read_answer fallback assignments (toolchain
and uninstall). Both now check the function's status before clearing ANSWER;
the scan then passed. TECHNICAL 3.8 states the general status-remapping rule.
No dependency was added, and no ignored or GUI test was run.

Follow-up final validation: cargo test -p watane-install passed (97 passed,
16 ignored, 11 suites; 633.34 s). Clippy, nightly fmt check, shellcheck and
git diff --check passed. Clippy retains only the existing workspace-hack
manifest warnings. This supersedes the earlier unresolved assertion result.

### s50b-fix-7 validator return, round 1: rule classes

Kept validator commit f9fbc69's single undo-status paragraph. Replaced the
large exact-line allow-list in status_audit.rs with rules for builtin tests,
read/command -v, the twelve named builtin-only or internally checked helpers,
and status-preserving `|| return` / `|| exit "$?"`. Quoted arguments are
kept opaque while identifying shell operators, and helper substitutions
must contain only a named helper call. Nested substitutions remain subject
to review so a quoted external command cannot hide inside helper arguments.
Only eight exact special sites remain, each commented and function-scoped:
rollback in on_signal, the two dispatch subshells, the two conditional
substitutions, saved rustup status, byte formatting and the builtin tty probe.

The new rule-class test first failed on an ordinary edited builtin test
under the old allow-list, then passed with the rule matcher. It also rejects
an unknown helper (both negation and constant return), and accepts each
named helper without adding line entries. A nested external-substitution
regression failed before the nesting guard and passed afterwards. Kept all
previous mutation checks. TECHNICAL 3.8 describes the rule classes and the
remaining special exceptions. No installer behavior or dependency changed.

Round-1 final validation: default cargo test -p watane-install passed
(98 passed, 16 ignored), including both rule-scan tests and all unchanged
rollback tests. Focused scanner tests passed after the test-only clippy fix
(use expect with the established peek invariant instead of unwrap).
Clippy, nightly fmt check, shellcheck and git diff --check passed; only the
existing workspace-hack manifest warnings remain. No ignored or GUI tests ran.

### s50b-fix-8: negated predicates and failed undo

The unchanged adversary scan test first failed on all six reported spellings.
The scanner now tokenizes every line containing a negation token and checks
negations in every command position, regardless of whitespace. Added AC1
coverage for until, separators and pipelines as well as the adversary cases.
TECHNICAL 3.8 explicitly names the forms left to hand review.

The AC3 fake-mv test first reproduced the false "Nothing was changed" claim
when restoring *.old.* fails permanently. install_files now retains rollback's
status, checks it for signals, and prints recovery instructions on a non-signal
undo failure; successful undo keeps its original text. The test also verifies
missing entry paths and recovery/install on the next run. No existing test
assertion or dependency changed. TECHNICAL 3.8 and 3.9 own both exit-5 texts.

Final validation: cargo test -p watane-install passed (101 passed, 16 ignored,
11 suites; 646.69 s). Clippy with -D warnings, nightly fmt --check,
shellcheck -s sh install.sh package.sh and git diff --check passed. Clippy
reported only the existing workspace-hack manifest warnings. No ignored or
GUI tests ran.


### s50b-fix-9: scan command positions and truthful rollback reporting

Both unchanged adversary tests failed first: the scan missed case arms,
background separators, negated substitutions and continued quotes; staging
cleanup failure falsely claimed installed files were missing. The scanner now
recognizes those command positions, rejects negated substitutions before
checkpoint exemptions, and joins physical lines while quotes remain open.
TECHNICAL 3.8 records its regression-guard scope and remaining hand-review
forms; this is not a shell parser or security boundary.

Added ac2_transient_undo_failure_retries_and_restores_old_version, which failed
on the old recovery message before the fix. install_files saves old journal
paths, retries a failed rollback once, then chooses the exit-5 text by whether
an old path is missing. Staging cleanup failures alone keep the intact-version
text. The permanent restore failure test still passes and proves next-run
recovery. Added ac3_scan_scope_and_exit5_texts_are_documented, observed failing
before updating TECHNICAL 3.8 and 3.9. No existing test assertions or dependencies
changed.

Final validation: cargo test -p watane-install passed (105 passed, 16 ignored,
11 suites; 665.91 s). Clippy --all-targets with -D warnings, nightly fmt
--check, shellcheck -s sh install.sh package.sh (exit 0), and git diff --check
passed. Clippy emitted only existing workspace-hack manifest warnings.
No ignored or GUI tests ran.

Validator (s50b-fix-9): the missing-path check alone called a failed undo
whole when removing the new copy failed (the path existed, the old entry was
still under `.old.<pid>`). The diagnosis now also counts a remaining `.old.`
name, except the receipt's copy; validator_undo_stopped_before_restoring_reports_files_not_put_back pins it.


### S50b-fix2: early-check signals and portable test shutdown

The new ac2_substitution_fallback_checks_status_before_die failed first because
the scan exempted both inline `|| die` and a `die` on the next line. Removing
those exemptions exposed the root-id substitution, home resolution, recovery,
archive extraction and temporary-directory creation. Each now checks the failed
status in its caller before diagnosing it; die keeps its own dispatch check.
Steps 1 to 4 were reviewed: version validation is builtin-only, tool lookup is
checked explicitly, and platform substitutions already retain failed statuses.

Stale journals use 99999999, above Linux's maximum pid limit and macOS's pid
range. Contrary to the task brief's search result, script.rs also contained
42-based journal fixtures, including one entering main's live-pid check; those
fixtures and their matching paths, faults and expected texts use the same dead
pid. No assertions were weakened.

The pseudo-terminal runner drops its piped stdin before waiting for script(1),
so util-linux script can observe EOF. The three A && B || C checks are explicit
if statements with the same short-circuit behavior, avoiding shellcheck 0.9's
SC2015. TECHNICAL 3.4, 3.8 and 12.1 record the caller checks, scan rule, dead
pid and PTY close order. The 600-second waits and four-run gate are unchanged.
No dependencies were added.

Targeted validation: the new audit test was red on both the original scan and
the original script, then all seven status_audit tests passed. The unchanged
Mac Ctrl-C timing regression and stale-recovery Ctrl-C test passed.

Final local validation: cargo test -p watane-install passed (107 passed,
16 ignored, 10 suites; 419.85 s). Clippy --all-targets with -D warnings,
nightly fmt --check, shellcheck -s sh install.sh package.sh (0.11.0) and
git diff --check passed. waits.rs's ac3 shellcheck test also passed unchanged.
The task's Linux report supplies the prior PID, PTY and shellcheck 0.9
failures; Linux and shellcheck 0.9 were not available for local verification.
Per the task's sandbox instructions, no existing ignored or GUI tests and no
full --include-ignored run were started; the outside-sandbox validator must
check inst_s7_target_missing and the remaining ignored PTY cases.


### S50b-fix2 follow-up: preserve queued PTY answers on macOS

The outside-sandbox Mac check (s50b-fix2-1.check.log) exposed five lost-answer
failures: inst_s2_toolchain_answered_y, inst_s7_target_missing,
ac2_toolchain_accepts_mixed_case_yes, inst_s16_uninstall_home_deleted and
inst_s28_a_home_that_is_not_a_watane_home. Closing script(1)'s input before
waiting works for util-linux, but macOS forwards EOF before the installer's
/dev/tty read has consumed the queued answer.

Added ac1_pty_preserves_the_last_answer_until_consumed: a prebuilt shell fixture
prints a prompt, delays its tty read for one second, then confirms the consumed
answer. It failed first on the Mac with exit 11 and EOF before delete; after
the fix it passes and prints consumed: <delete>. It is ignored by default
because it requires script(1), and was run explicitly as this task's new test.

The runner now closes stdin before child.wait() only on Linux and after it
on macOS (and other non-Linux targets). This retains Linux's EOF-based shutdown
and macOS's old ordering. TECHNICAL 12.1 records the platform distinction and
regression. No existing assertions, waits, concurrency limits or dependencies
changed.

Follow-up validation on the Mac: the new delayed-reader test passed explicitly
after failing before the fix. Default cargo test -p watane-install passed
(107 passed, 17 ignored). Clippy --all-targets with -D warnings, nightly
fmt --check, shellcheck and git diff --check passed. Existing ignored PTY
tests remain for the outside-sandbox validator per the task instructions;
Linux execution also remains for its runner.


### S50c: receipt folder identity, lock and recovery evidence

The receipt keeps four `resolved` lines from the folder checks. Receipt-driven
uninstall and old-only post-commit removals require the same physical folder
now, including removal of the receipt itself. Missing resolution records skip
removals with the specified warning. A mkdir lock in the home serializes
install and uninstall; status remains read-only. Only the owner removes its
lock, after rollback. Dead, empty and malformed planted locks are never broken.

Undo now requires `moved <receipt>` before trusting the copied old receipt.
Recovery and uninstall's later stale cleanup leave staging evidence alone
when the receipt's path is disallowed. The new fake cp writes a partial
backup then fails; fake mv's wait mode lets the concurrency test release a
successful first install. No dependencies or installer test seams were added.

Red tests reproduced missing resolved lines, an ignored planted lock, a
32-byte receipt replacing the complete receipt, and deleted staging evidence
through a receipt directory linked outside the home. The kill tests first
failed at the new lock boundary, then passed with the helper checking the
refusal and executing its printed removal command before recovery. The
prebuilt TERM rollback fixture needed its completed receipt backup recorded
with `moved <receipt>`; its rollback assertions are unchanged and pass.

INST-S-37's old-only vendor cleanup uses a prebuilt committed swap because
the current archive requires vendor. FUNCTIONAL and TECHNICAL clarify that
fixture; the archive contract stays unchanged. TECHNICAL also describes the
new fake cp, mv wait mode and S50c tests.

Final validation on the Mac: default `cargo test` passed (116 passed,
17 ignored), including unchanged status_audit.rs and check_signals.rs.
Clippy --all-targets with -D warnings, nightly fmt --check, shellcheck
-s sh install.sh package.sh and git diff --check passed. The first default
run exposed only the incomplete prebuilt TERM fixture noted above; after
correcting its journal evidence, the focused test and final default run
passed. Existing ignored tests and --include-ignored remain for the
outside-sandbox validator, per the task instructions. Clean-Mac and VM
work remain out of scope for this slice.


### S50c validator round 1: failed signal undo and foreign folder identity

Built on validator 8198038 and foreman 2d288a2. The imported regression
`ac2_lock_is_removed_after_a_failed_undo_under_a_signal` first failed with
the lock left behind after exit 143. It now also checks that the temporary
folder is gone and the journal remains. Cleanup marks its phase, and a
signal exit selected by check_signal in that phase releases the run-owned
lock and temporary folder before exiting. Resource cleanup and undo use
local signal/phase state so the parent keeps its selected signal and only
the parent releases the lock after undo. Status-dropping fallbacks still
check_signal first; status_audit.rs and check_signals.rs are unchanged.

`ac1_upgrade_refuses_remapped_or_unrecorded_receipt_paths_before_download`
first reproduced a successful upgrade through the replaced vendor folder.
The foreign-file check now applies receipt_allowed, including rule 4,
without printing removal warnings. Both a changed folder resolution and
missing resolved lines make existing entries foreign: exit 8, vendor listed,
no download, marker and home snapshot unchanged. The separate prebuilt
committed-swap cleanup test remains in place. The test-plan paragraph moved
from TECHNICAL 8.1 to 12.1 as requested.

Reviewed hand-written receipts in script.rs, adv_s50b.rs and s50c.rs.
Successful install/upgrade/reinstall fixtures retain the real install's
resolved lines. Hand-built receipt.new files are pending journal evidence,
not the installed receipt; the other literal receipts test status or
outside-home uninstall refusal. No existing receipt fixture needed repair,
so no support helper or existing test was changed. The two named tests above
are the tests added in this round. No dependencies were added.

Round-one final validation on the Mac: default cargo test passed (120 passed,
17 ignored), including the unchanged status_audit and check_signals suites.
The final failed-undo signal regression also passed in isolation after the
cleanup refactor. Clippy --all-targets with -D warnings, nightly fmt --check,
shellcheck -s sh install.sh package.sh and git diff --check passed. Existing
ignored tests remain for the outside-sandbox validator as instructed.
### S50c-2: release workflow and install entry point

Added the two-job release workflow: a read-only macOS build with no cache,
five-file version checks and the tag's main-ancestry check; a separate,
environment-gated publisher that downloads only the artifact, creates a draft
with all three assets and publishes it last. Manual dispatch has no inputs
and never enters the publishing job. Hyphenated versions are prereleases.
The smoke command calls the S50c-3 interface without implementing its script.

The action v4 commit SHAs were resolved with `gh api` for checkout,
upload-artifact and download-artifact. Tauri CLI is pinned to 2.12.5, from
the same 2.12 series as Cargo.lock's Tauri 2.12.5; upstream release:
https://tauri.app/release/tauri-cli/v2.12.5/ . The runner's stable toolchain
overrides rust-toolchain.toml, so that file does not install its Wasm target.
The actual bundle build and workflow dry run remain the foreman's checks.

The workflow and README tests were observed red with the missing workflow
and install section, then green. Mutations remove each required rule or add
unsafe inputs/steps. An additional red/green mutation proves an unrelated
`echo -- --locked` cannot satisfy the app-build check. A fake-gh publisher
fixture first exposed Bash 3's empty-array/nounset behavior; a boolean
prerelease argument now works for both stable and prerelease versions, and
the fixture verifies both calls and the rendered notes without network access.

AC3 needed no code or test change: package.sh already bakes @RELEASE@ into
both copies, and package::ac1_the_archive_has_the_tree_of_4_2 passed unchanged,
checking the baked script, absence of the placeholder and identical asset bytes.
README now shows the install and --no-toolchain commands and the behavior link.
No dependency was added. No ignored, GUI or real-release test was run.

Actionlint was not run: `brew list actionlint` reports no installed keg.
YAML loading and `bash -n` of all embedded shell steps passed.

Final validation: `cargo test -p watane-install` passed (112 passed,
17 ignored, 11 suites; 1157.71 s). Clippy --all-targets with -D warnings,
nightly fmt --check, shellcheck -s sh install.sh package.sh and git diff
--check passed. The version/origin step was also exercised locally with
temporary manifests and a fake git: manual/matching tags passed; a version
mismatch and off-main tag failed before any build. The publisher fixture
passed for stable and prerelease notes/arguments. No real release was created.

Validator (S50c-2, round 1): the Tauri CLI pin moved from 2.12.5 to 2.12.4.
`tauri-cli` and `tauri` number their versions apart; crates.io shows
`tauri` 2.12.5 and `tauri-cli` 2.12.4 published two minutes apart on
2026-10-21, both on `tauri-utils` ~2.10.2 (the lock's 2.10.2), while
`tauri-cli` 2.12.5 (2026-11-16) needs `tauri-utils` ~2.10.3. The three
action SHAs were re-checked with `gh api` against the `v4` tags.
actionlint 1.7.12 reports nothing on `release.yml`.
## S50c-3: clean guest and host VM harness

Added clean_mac.sh with an env-cleared preflight before any release access,
real file:// or one-line curl installs, no-terminal smoke, prompt-driven
script(1) answers, the clean and CLT scenario groups, plutil JSON/plist
checks, version and native-library checks. The clean guest tests n and
Enter separately, then keep-home and delete-home uninstalls; a reinstall
between those uninstalls is necessary because no receipt means exit 7.
A home-path snapshot allows the guest's pre-existing Desktop/Library
folders while rejecting new paths outside the install/toolchain folders.

Added vm_mac.sh with missing-Tart exit 2 before any mutation, per-run
clone/headless boot/IP/SSH/scp, forced SSH_ASKPASS password authentication,
a shared 600-second readiness deadline, and stop/delete traps. Preparation
makes the two templates, installs CLT only in a disposable guest, then
runs both parts. No host directory is shared. Fake commands cover success,
guest failure, preparation and local release copying (including host
paths with spaces); the real Tart program and image were not installed.

Tests first failed for missing scripts, missing preflight refusal, missing
VM lifecycle, and the missing native dependency/home comparison helpers.
The real fresh-home test then caught the distinct no-terminal text;
it now distinguishes that from a typed n. The delayed tty-reader fixture
passes from a process with no controlling terminal. The native dependency
fixture rejects Homebrew/rpath dependencies and truncated Mach-O data.
No dependency was added to the workspace.

Spec correction: /usr/bin/otool is a CLT shim (the same system shim as cc
and git). The no-CLT part must read thin arm64 Mach-O dylib load commands
with stock Perl instead. It enforces the same /usr/lib or /System path
assertion; CLT/fresh-home runs still use otool -L. The reader also passed
on the three real release programs and the real app executable.

Real local build: cargo build --release --locked -p watane-host -p
watane-dev -p watane-node passed (4m44s). Installed tauri-cli =2.12.5
--locked, matching Cargo.lock's tauri version, then cargo tauri build
--bundles app with the 7.3 config override and -- --locked passed (2m15s,
71.02 MiB bundle). Restored the CLI's incidental features=[] manifest
formatting; no sibling file remains changed. package.sh
 aarch64-apple-darwin 0.1.0 /tmp/s50c-3-release passed; clean_mac.sh
--from /tmp/s50c-3-release --fresh-home printed CLEAN-MAC PASS, with no
app launch or toolchain installation. The ignored real-release test also
passed, with CARGO_NET_OFFLINE=true for packaging so cargo test cannot
reach the network.

Tart flags/credentials were checked using gh api at openai/tart
 a8d03d4a1ae34705e153a376437cee3b4f6c4f63 (quick-start and the Clone,
Run, IP, Stop and Delete command sources). The task's golden-gate image
is not in that quick-start's image list; availability awaits the approved
real VM preparation. No existing test assertion was changed or ignored
suite/GUI test run. Only the two newly written ignored tests were run.

Review also found that a relative --from path was being resolved after
entering H. A fast installer-sentinel fixture reproduced the lost path;
resolution now happens first, under the cleared environment, while
release access still waits until after the clean assertions. The final
home snapshot fixture and relative-path fixture each passed after red.

Final validation: the single default cargo test run passed (115 passed,
19 ignored, 11 suites; about 1250 seconds on this Mac). Both new ignored
tests were run explicitly and passed: the real prebuilt release smoke
and delayed PTY answer. No existing ignored or GUI test ran. Clippy
--all-targets with -D warnings, nightly fmt --check, shellcheck of all
four shell scripts, and git diff --check passed. The final direct smoke
log is /tmp/s50c-3-smoke.log and its last line is CLEAN-MAC PASS.

## S89-1 (install.sh tells the truth)

Decided in TECHNICAL 3.6 to 3.9, 8.1, 8.5 and IN-44 to IN-48: the post-commit and
question-time interruption lines (printed by the exit trap, flag `COMMITTED` (see S89-fix1) set right
after the receipt's `mv`), `say` swallowing printf's write error so `--status | head -1`
ends by the PIPE trap (141, or 0), `~` for resolved paths through `tilde_r`, the offer's
URL in "install it later", empty folders the run created removed in `release_files`
(only when the run did not commit, only with `rmdir`), rustup's output framed per
rustup command (offer B prints two frames). Limits: a signal landing between the receipt's
`mv` and `COMMITTED=1` gets no hint (the install is whole; the swap's undo finishes the
commit); a lost INT on macOS sh reaches the same trap through `check_signal`. Tests: `tests/it/s89.rs`
(run on Linux; macOS-only sh behaviours are for the PO's Mac run).

## S89-fix1

"Committed" for the hint and `remove_created` is read in `rollback` from the journal plus the
receipt's `.new.<pid>` being gone (undo's own test); the `COMMITTED=1` after the `mv` stays only for
the normal success path. `say` exits 1 on a write error that no signal ended (pinned by
`fix1_say_to_a_full_disk_...`). `remove_created` re-resolves and skips links/outside-home folders.
The odd-URL quote applies when the URL has characters outside a plain-word set, not for every URL
other than the default, because s89 `ac5` (unchanged) expects an ordinary URL unquoted.

## S89-2: Node 24 actions and the release-build skip

Pins now: checkout v7.0.1 (3d3c42e5...), upload-artifact v7.0.2 (cf430e03...),
download-artifact v8.0.2 (9000827c...); all lightweight tags, so the tag object is
the commit; each `action.yml` was fetched and says `using: node24`. READMEs checked:
`path` of download-artifact still extracts a single named artifact directly into
`path`; upload-artifact `include-hidden-files` still defaults to false and
`dist-out/` has none. Earlier notes about "v4 SHAs" are history.

S89-fix2: a swap that fails after the receipt's rename now exits 5 with `a step after the install failed`
and the hint; fresh-install failure says `Nothing was installed.`; "install it later" quotes any URL not
http(s)-plain and drops `--proto` when WATANE_RUSTUP_URL is set. Not done: the hint's doctor line with the
full path when <prefix>/bin is off PATH, because adv_s89's signal test (unchangeable) pins the bare line
with a PATH that lacks the bin folder. Edited besides: tests/fakes/chmod (real chmod when FAKE_REAL is
unset, for the pasted line of sim_s89) and the tests that pinned `--proto` with a set URL.

S89-fix4: a failure before any swap (verify_archive, extract_archive) runs before `recover`, so with a receipt
it says only `Nothing was changed.`; "still works" stays for a failed swap after undo. `rollback` reads the
journal only as a regular file, not a link. The fresh-install undo failure now says a rerun "removes them and
installs watane". Left over: the post-undo "still works" line (INST-F-18).
