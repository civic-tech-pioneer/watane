Subsystem: install
Effort: medium
Depends on: s50b-fix-2
Slice: S50b-fix — install's tests pass on macOS

## Goal
The two adversary breaches on the slice's fix commits are closed; their red tests pass unchanged.
1. A typed answer counts only when its bytes are exactly the accepted word. macOS `/bin/sh`
   (bash 3.2) `read` drops NUL bytes, so `y\0`, `\0yes`, `y\0\0\0` reach the `case` as `y`/`yes`
   and rustup runs. Decision (foreman): for both questions (3.7's toolchain offer: `y`/`yes` in
   any case; 8.4's home question: `delete`), an answer holding any byte outside the accepted word,
   a NUL included, is no / keep.
2. Undoing a swap twice never removes a file the run did not move in. `rollback` (f640d24)
   repeats `undo_swap` when a signal killed the first one; `undo_swap` deletes each
   `<entry>.new.<pid>` before `journal.<pid>`, so an undo that dies between the two leaves `new <p>`
   and `moved <p>` lines with no `<p>.new.<pid>`, and the repeat reads that as "moved in" and
   `rm -rf <p>`, deleting what the person put there since (a dragged-in watane.app). The same
   happens to the next run's `recover`. The repeat (and `recover`) must be safe at every point an
   undo can die; also a journal left behind once the last `.new` is gone must not outlive the run
   (adversary RISK: pid reuse then reads "another install is running").

## Acceptance
- AC1 `install/tests/script.rs` `adv_s50b_fix_toolchain_nul_in_answer_is_no` (ignored: pty; run
  with `--ignored`) passes unchanged.
- AC2 `install/tests/script.rs` `adv_s50b_fix_a_repeated_undo_removes_a_file_it_never_moved`
  passes unchanged.
- AC3 A pty test types `de\0lete` at the home question (8.4): the home is kept. (Model it on the
  existing `inst_s16_uninstall_home_deleted` and the AC1 test.)
- AC4 The undo's order (or the journal's record) makes a repeated undo and the next run's
  recovery idempotent: a test kills an undo at each of its removal steps in turn (the fake `rm`
  `term` mode of f640d24 can do this) and checks the home is the old version, nothing the run did
  not move is removed, and no `journal.<pid>` is left after the run ends or after the next run.
- AC5 Reading an answer stays POSIX with the tools of 3.3; if you need one more stock tool (for
  example `od` or `dd`), add it to 3.3's list. `shellcheck -s sh install.sh package.sh` exits 0.
  Every other test passes.

## Spec
install/TECHNICAL 3.6 step 4 (the journal), 3.7 (what counts as yes), 3.8 (the undo), 8.4 (the
home question), INST-S-35 (a `new` line alone does not show watane put the file there). Write
the decisions into those sections and install/NOTES.md.

## Interface
none.

## Files
`install/install.sh`, `install/tests/*`, `install/TECHNICAL.md`, `install/NOTES.md`.

## Changed tests
none: the two red tests are the acceptance and stay unchanged. Searched `install/tests/script.rs`
for the answer and undo tests (`inst_s2`, `s3`, `s5a`, `s15`, `s16`, `s28`, `s33_*`, `s35`,
`ac2_toolchain_*`): their assertions hold under this change.

## Out of scope
A swap `mv` across filesystems (adversary RISK, needs a second mount: S50c's clean-Mac run);
anything outside `install/`.
