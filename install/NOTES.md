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
