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
`HOME_OUT` for the outside-home case; the question and the `deleted:` line follow `tilde_r` of the resolved path.
