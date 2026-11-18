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
