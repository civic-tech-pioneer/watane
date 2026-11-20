Subsystem: install
Effort: medium
Depends on: none
Slice: S89 — Alice installs, interrupts, checks and uninstalls watane, and every line she reads is true and in her own terms

## Goal
The adversary's second red test passes: when the swap fails after the receipt's rename (the
receipt `mv` renamed the file but returned non-zero, e.g. a SIGKILLed `mv`, status 137), undo
finishes the commit, so the new version is installed; the run then must not say "Nothing was
changed". It exits 5 with a cause that is true and names what is installed, followed by the two
lines of the interruption hint (TECHNICAL 3.8/3.9, IN-45). Recommended text, to be written into
TECHNICAL 3.9 first: `watane install: a step after the install failed` then
`watane <version> is installed; the run was interrupted before it finished.` and
`Run watane-dev doctor to check the maker tools.` (reuse the hint's lines; do not print them twice).
Also: the "install it later" URL prints unquoted only when it starts with `https://` or `http://`
and holds only the characters of the current safe set; any other URL (a leading `~`, `=` or `-`,
or any odd byte) goes through `shell_quote` (TECHNICAL 3.7; IN-44 note). A leading `-` inside
quotes is still a curl option: write that down as a decided limit (only the person sets the
variable, AGENTS rule 4).

## Acceptance
AC1. `adv_s89::adv_s89_receipt_move_that_fails_after_the_rename_says_nothing_was_changed` passes,
     unchanged, and its expectations are met by the text you put in TECHNICAL 3.9 (if the test
     pins a text other than the recommended one, follow the test's assertions; read it first).
AC2. A failed swap before the receipt's rename still prints "Nothing was changed..." as today
     (existing tests).
AC3. `WATANE_RUSTUP_URL='~root/x'` and `'=x'` are printed single-quoted; `https://example.invalid/rustup-init.sh`
     and the default stay unquoted.
AC4. All install tests pass with `--include-ignored` apart from the known environment failures;
     shellcheck clean.

## Spec
install/TECHNICAL.md 3.6 step 4, 3.7, 3.8, 3.9, IN-44, IN-45.

## Interface
none.

## Files
install/install.sh, install/TECHNICAL.md, install/NOTES.md, install/tests/it/s89.rs, plus any
user-sim red test file the foreman names when starting you.

## Changed tests
none; adv_s89.rs must not change.

## Out of scope
macOS sh runs (PO).
