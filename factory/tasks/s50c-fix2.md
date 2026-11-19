Subsystem: install
Effort: low
Depends on: none
Slice: S50c — a person installs watane on a clean Mac (install stages 4 and 5, J7)

## Goal
The two red tests of S50c's user-sim pass (install/tests/regress_s50c_sim.rs, commit 751e35c):
`regress_s50c_relinked_folder_refusal_names_the_link` and
`regress_s50c_home_changes_ignores_macos_media_library_writes`. The foreman wrote the spec
(see `git diff 751e35c -- install/TECHNICAL.md` after this brief's commit):
1. install.sh, 3.4.2: a path foreign only by 8.5 rule 4 is not listed with an `mv` hint; per
   such install folder the run prints the two lines `watane install: ~/<folder> resolves to
   <now>, not to <R> as at install` / `Make ~/<folder> a real folder again (or point it where it
   was), then run this again.`, exit 8, before any download; ordinary foreign paths are listed
   as before (both blocks may appear in one run).
2. install/tests/clean_mac.sh `home_changes`: the guest rule of TECHNICAL 12.3 (new top-level
   names in the home fail except the allowed ones; a `watane` component outside .local,
   Applications and .watane fails; deeper new paths in folders that existed are skipped). The
   `--fresh-home` mode keeps its current strict rule if it differs, say which.

## Acceptance
- AC1 Both red tests pass, assertions unchanged.
- AC2 INST-S-37's upgrade case (tests/s50c.rs) still exits 8 before any download and keeps the
  marker; its stderr assertions follow the new text (update them; list them).
- AC3 New status-dropping sites go through check_signal; status_audit.rs, check_signals.rs pass
  unchanged. Every watane-install test passes (`-- --include-ignored`, skipping only
  `ac1_ac2_fresh_home_real_release` when target/release has no build); shellcheck, clippy, fmt.

## Spec
install/TECHNICAL 3.4.2, 8.5 rule 4, 12.3. install/FUNCTIONAL INST-S-30, 37.

## Interface
none.

## Files
`install/install.sh`, `install/tests/*`, `install/TECHNICAL.md`, `install/FUNCTIONAL.md`,
`install/NOTES.md`.

## Changed tests
tests/s50c.rs `ac1_upgrade_refuses_remapped_or_unrecorded_receipt_paths_before_download` (and
any other test asserting `found files that watane did not install` for a re-linked folder):
the stderr text for rule-4 paths. Searched install/tests for `did not install`.

## Out of scope
The "felt wrong" items of the user-sim report (empty folders after a late Ctrl-C, `/private`
paths in `skipped`, rustup's text): later slice.
