Subsystem: install
Effort: low
Depends on: s50b-fix-1
Slice: S50b-fix — install's tests pass on macOS

## Goal
Only a typed `y` or `yes` (any case) installs the toolchain. Today `toolchain_step` lowercases
the answer with `tr '[:upper:]' '[:lower:]'` and ignores `tr`'s exit code: in a UTF-8 locale
`tr` stops at the first invalid byte with "Illegal byte sequence" but keeps what it wrote, so
`y\xffnope` becomes `y` and `yes\xc3` becomes `yes`, and both count as yes (adversary RISK,
pre-existing). Decision (foreman): match the raw answer with `case` patterns and drop `tr`;
anything else, an undecodable byte included, is no.

## Acceptance
- AC1 `install.sh` `toolchain_step` decides with `case $ts_ans in [Yy] | [Yy][Ee][Ss]) ...` on
  the answer as read; no `tr` on the answer. The home question of 8.4, if it parses its answer
  the same way, gets the same treatment.
- AC2 A pty test (in `tests/script.rs`, beside `inst_s2_toolchain_answered_y`) types the bytes
  `y\xffnope` and Enter at offer A: the toolchain is not installed and `toolchain: not
  installed` is printed. Another types `YeS`: installed, as `y` is. Run them with
  `LANG`/`LC_ALL` `en_US.UTF-8` in the env of the run.
- AC3 `shellcheck -s sh install.sh package.sh` still exits 0; every other test passes.

## Spec
install/TECHNICAL 3.7 (the text near line 399, "After `tr 'A-Z' 'a-z'`, only `y` and `yes`
count": rewrite it to "only `y` and `yes`, in any case, count; any other answer, an
undecodable byte included, is no"), and 8.4 if it says the same. Record in install/NOTES.md.

## Interface
none.

## Files
`install/install.sh`, `install/tests/script.rs`, `install/TECHNICAL.md`, `install/NOTES.md`.

## Changed tests
none: the existing answer tests (`inst_s2`, `inst_s3`, `inst_s5a`, `inst_s34_*`, `ac4_*`) keep
their assertions. Searched `install/tests/*.rs` for `tr ` and answer strings.

## Out of scope
Any other question or text; anything outside `install/`.
