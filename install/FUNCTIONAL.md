# install: functional specification

What `install` does, free of implementation. "The script" is `install.sh`
(`TECHNICAL.md` 3). "The person" is whoever runs it, Alice or an agent
for her. "The release" is one version published on the GitHub release
page of the watane repository. "The toolchain" is `rustup`, a Rust
compiler and the `wasm32-wasip2` target. "The home" is the watane home
(`$WATANE_HOME`, else `~/.watane`). Ids of `spec/` (F-..., J...) and of
`dev/FUNCTIONAL.md` (DEV-F-n) are cited in `Traces:` lines only. File
names, paths, flags and the full texts are in `TECHNICAL.md`; this
document quotes the lines that a test checks.

Step 7 (the real world), the first of its slices. macOS (Apple silicon)
only; Linux is out of this subsystem's first version (`TECHNICAL.md` 9).

## 1. Terms of this subsystem

| Term | Meaning |
|---|---|
| prefix | The folder that holds `bin/` and `share/`: `~/.local`. |
| receipt | The file that the script writes last, listing every path it installed, so that an upgrade and an uninstall remove exactly those. |
| archive | The one file of a release that holds the programs, the app and the files they need beside them, for one platform. |
| checksums | The file `SHA256SUMS` of the same release, with the SHA-256 of each archive. |
| offer | The question whether to install the toolchain. |

## 2. Requirements

### 2.1 Install

- **INST-F-1** When the person runs the one-line command of the release, the script shall install `watane`, `watane-dev` and `watane-node` in `<prefix>/bin` and `watane.app` in `~/Applications`, with no other action of the person.
  Traces: F-GET-1, F-QUAL-2, J7.
- **INST-F-2** The script shall install, beside the programs, every file they need at run time, so that `watane-dev` finds its crate allowlist and `watane` with no setting and no checkout.
  Traces: F-MAKE-4, DEV-F-13.
- **INST-F-3** Before it writes any file outside its temporary folder, the script shall verify the SHA-256 of the archive against the checksums of the same release.
  Traces: F-GET-2, F-QUAL-3.
- **INST-F-4** If a download fails or a checksum does not match, then the script shall install nothing, remove its temporary folder, name the cause, and exit with a status that is not 0.
  Traces: F-QUAL-3.
- **INST-F-5** The script shall write only under `<prefix>`, `~/Applications` and its own temporary folder (and, after a yes to the offer, rustup in `~/.rustup` and `~/.cargo`), shall never ask for a password or run `sudo`, and shall refuse to run as root.
  Traces: TC-6, V5.
- **INST-F-6** The script shall need no account, key or token, shall contact only the release page and, after a yes to the offer, `sh.rustup.rs` and what `rustup` then fetches, and shall send nothing about the person or the machine.
  Traces: F-MAKE-6, TC-6.
- **INST-F-7** If the platform is not macOS on Apple silicon at the minimum version, or a tool that the script relies on is missing, then the script shall stop before it downloads anything, name the cause, and exit with status 2.
  Traces: TC-7, F-QUAL-3.

### 2.2 Toolchain

- **INST-F-8** After the install, if the toolchain is not complete, the script shall offer to install what is missing with a question that names what it installs, from where and into which folders, and that is answered by `y` or `N`, with `N` the default.
  Traces: F-MAKE-2, DEV-F-13.
- **INST-F-9** The script shall install the toolchain only after the answer `y` (or `yes`) typed on the terminal; in every other case it shall install none. No flag, variable or file answers for the person: the script asks the terminal (`/dev/tty`) and nothing else.
  Traces: F-GET-3, V5, F-AGENT-4.
- **INST-F-10** When the script has no terminal to ask on, it shall ask nothing, treat the answer as `N`, and print the commands that install the toolchain.
  Traces: F-MAKE-5.
- **INST-F-11** When the answer is `N`, the script shall print the same commands, finish with status 0, and leave the installed programs as they are.
  Traces: F-MAKE-5.
- **INST-F-12** When the toolchain is complete, the script shall ask nothing about it and say so.
  Traces: F-QUAL-3.
- **INST-F-13** The script shall not change what `watane-dev` does: `watane-dev` shall keep installing nothing and printing the command that adds a missing target.
  Traces: DEV-F-13.
- **INST-F-26** If the macOS command line tools are not installed (`xcode-select -p` fails), then the script shall print the one command `xcode-select --install` and why, shall never run it, and shall print nothing about the tools when they are installed.
  Traces: F-GET-3, F-MAKE-2, F-MAKE-5.

### 2.3 After the install

- **INST-F-14** The script shall print where each program and the app were put, and the next command to run.
  Traces: F-MAKE-5, J7.
- **INST-F-15** If `<prefix>/bin` is not on the `PATH`, then the script shall print the line that adds it and the file to put it in, and shall edit no shell file.
  Traces: V5.
- **INST-F-16** The script shall remove the macOS quarantine mark from the installed `watane.app`, and shall print what to do if macOS still asks whether to open it: the words of the Gatekeeper step and the one command.
  Traces: F-GET-5, J7, V1.
- **INST-F-17** Each failure message shall give the cause on its first line and, on the next lines, what the person can do; it shall carry no stack of the script's own steps.
  Traces: F-QUAL-3, F-MAKE-5.

### 2.4 Version, upgrade, uninstall

- **INST-F-18** Each program shall print its version on `--version`, the version of the release; `install.sh --status` shall print the version, the platform and the paths that the receipt holds.
  Traces: F-QUAL-3.
- **INST-F-19** When the script runs over an installed version, it shall replace exactly the paths of the receipt with those of the new release, say `upgrading watane <old> to <new>`, and leave every other file, the home and the toolchain alone.
  Traces: F-GET-4, V1, F-QUAL-3.
- **INST-F-20** If an upgrade fails or is interrupted after it began and before its receipt is in place, then the script shall put back the files it replaced and the old receipt, so that the version that was installed still works and a later run installs without meeting its own files as foreign; the receipt shall move into place last, with the files, and nothing the old receipt held shall be removed before it.
  Traces: F-QUAL-3.
- **INST-F-21** When the person runs `install.sh --uninstall`, the script shall remove exactly the paths of the receipt and the receipt, and name what it kept: the home, the toolchain and the maker cache.
  Traces: F-GET-4, V1.
- **INST-F-22** The script shall delete the home only when the person types the word `delete` to a question that names the home and says that it holds the identity and cannot be restored without a backup; with no terminal, or any other answer, it shall keep the home. Only a terminal answers this question: no variable, flag or file does.
  Traces: F-APP-6, V1.
- **INST-F-23** Nothing shall run after the script ends: no background process, no scheduled job, no login item, no check for a new version.
  Traces: V4.
- **INST-F-24** If an install folder (`~/.local/bin`, `~/.local/share`, `~/Applications`) resolves, with symbolic links followed, to a place outside the person's home folder, then the script shall stop before it downloads anything, name the folder and where it resolves, and exit with status 2.
  Traces: TC-6, V5, INST-F-5.
- **INST-F-25** If no receipt lists a path that the script would install and that path already exists, then the script shall change nothing, name each such path, say how to move it aside, and exit with status 8.
  Traces: F-GET-4, V1, INST-F-19.

## 3. Scenarios

Each scenario is one test, named `s<n>_...` (`TECHNICAL.md` 12). Unless a
scenario says otherwise: `HOME` is an empty folder, `PATH` holds the fake
tools of `TECHNICAL.md` 12.1 and `~/.local/bin`, the release is a fake
one served by `WATANE_RELEASE_BASE` (version `0.1.0`), and "the files"
are `~/.local/bin/watane`, `~/.local/bin/watane-dev`,
`~/.local/bin/watane-node`, `~/.local/share/watane-dev/vendor/`,
`~/.local/share/watane/install.sh`, `~/.local/share/watane/VERSION`,
`~/.local/share/watane/receipt` and `~/Applications/watane.app/Contents/Info.plist`.
S1 to S22 and S26 to S35 run in `cargo test`, but those that type an answer on a pseudo-terminal (S2, S3, S7, S15, S16, S28, and S5 (a)) are `#[ignore = "slow: ..."]` and run in the slow set (`TECHNICAL.md` 12); S23 to S25 run only on
the clean Mac.

| # | Scenario | Oracle |
|---|---|---|
| INST-S-1 | **Fresh install on a clean Mac**, no terminal, so the toolchain is not installed (INST-S-4). Traces: INST-F-1, 2, 3, 14. | Exit 0. All the files exist and the three programs are executable. Stdout holds, in this order, the lines `watane 0.1.0: installing for aarch64-apple-darwin`, `downloading watane-0.1.0-aarch64-apple-darwin.tar.gz`, `checked: sha256 matches SHA256SUMS of release v0.1.0`, `watane 0.1.0 is installed.`. `~/.watane` does not exist. No file outside `~/.local`, `~/Applications` and the temporary folder was written, and the temporary folder is gone. |
| INST-S-2 | **Toolchain answered `y`**: no `rustup` anywhere; on a pseudo-terminal the test types `y` and Enter. Traces: INST-F-8, 9. | Exit 0. Stdout holds the question once, ending `[y/N]`. The fake rustup installer ran once with the arguments `-y --profile minimal --no-modify-path -t wasm32-wasip2`. Stdout holds `toolchain: installed` before `watane 0.1.0 is installed.` No file of `$HOME` outside the three folders of INST-S-1 and the fake installer's own marker changed. |
| INST-S-3 | **Toolchain answered `N`**: same start, on a pseudo-terminal; the test types `n` and Enter (and, in a second run, only Enter). Traces: INST-F-8, 9, 11. | Exit 0. The question was printed once. The fake installer did not run. Stdout holds `toolchain: not installed` and the line `  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh -s -- -y --profile minimal --no-modify-path -t wasm32-wasip2`. |
| INST-S-4 | **No terminal**: stdin is a pipe and there is no controlling terminal (the test starts the script in a new session with no controlling terminal, `TECHNICAL.md` 12.1). Traces: INST-F-10. | Exit 0. Stdout does not hold `[y/N]`. It holds `no terminal to ask on: the toolchain was not installed` and the same command line as INST-S-3. The fake installer did not run. |
| INST-S-5 | **Nothing but the terminal says yes**: (a) `--no-toolchain` on a pseudo-terminal where the test types `y` and Enter after the script started; (b) `--toolchain`, which does not exist; (c) no terminal, and `WATANE_INSTALL_TTY` set to a file that holds `y`. Traces: INST-F-9. | (a) Exit 0. No question printed; stdout holds `toolchain: skipped (--no-toolchain)` and the command line of INST-S-3; the fake installer did not run. (b) Exit 1; first line `watane install: unknown or conflicting flag --toolchain`; `$HOME` is empty. (c) Exit 0; stdout holds `no terminal to ask on: the toolchain was not installed`; the fake installer did not run (the variable is not read). |
| INST-S-6 | **Toolchain complete**: a fake `rustup` and `rustc` whose sysroot holds `lib/rustlib/wasm32-wasip2/lib`. Traces: INST-F-12. | Exit 0. Stdout holds `toolchain: ok (rustup, wasm32-wasip2)`. No question. No installer ran. |
| INST-S-7 | **rustup present, target missing**, on a pseudo-terminal the test types `y` and Enter. Traces: INST-F-8, 9. | Exit 0. The question names the target and not rustup itself: it ends `Add the wasm32-wasip2 target with rustup? [y/N]`. The fake rustup was called once with `target add wasm32-wasip2`. |
| INST-S-8 | **Checksum mismatch**: the archive differs from its line of `SHA256SUMS`. Traces: INST-F-3, 4. | Exit 4. Stderr's first line is `watane install: checksum mismatch for watane-0.1.0-aarch64-apple-darwin.tar.gz`, then `expected` and `got` lines, then `Nothing was installed.` None of the files exists. The temporary folder (including the partial download) is gone. |
| INST-S-9 | **No checksum line**: `SHA256SUMS` has no line for the archive. Traces: INST-F-3, 4. | Exit 4. First line `watane install: SHA256SUMS has no line for watane-0.1.0-aarch64-apple-darwin.tar.gz`. Nothing installed; temporary folder gone. |
| INST-S-10 | **Download fails**: the archive answers 404. Traces: INST-F-4. | Exit 3. First line `watane install: cannot download <url>`. Nothing installed; temporary folder gone. |
| INST-S-11 | **Interrupted**: the script's process group gets `INT` during the download. Traces: INST-F-4. | Exit 130. Nothing installed, temporary folder gone. |
| INST-S-12 | **Upgrade**: `0.1.0` is installed; a file `~/.local/bin/mine` and a file `~/.watane/marker` exist; the run installs `0.2.0`. Traces: INST-F-19. | Exit 0. Stdout holds `upgrading watane 0.1.0 to 0.2.0`. `~/.local/share/watane/VERSION` holds `0.2.0`; `watane --version` of the new fake prints `watane 0.2.0`. A file that only `0.1.0` held (`vendor/old-crate-1.0.0/`) is gone. `mine` and `marker` are unchanged. |
| INST-S-13 | **Same version again**. Traces: INST-F-19. | Exit 0. Stdout holds `reinstalling watane 0.1.0`. The files exist, as after INST-S-1. |
| INST-S-14 | **Failed upgrade**: `0.1.0` is installed; the run installs `0.2.0` and the fake `mv` fails on the app. Traces: INST-F-20. | Exit 5. `VERSION` still holds `0.1.0`, `watane --version` still prints `watane 0.1.0`, the app of `0.1.0` is in place, the receipt still holds `version 0.1.0`, no `.new`, `.old` or journal file is left. |
| INST-S-15 | **Uninstall, home kept**: after INST-S-1 and a file `~/.watane/marker`; the script runs on a real pseudo-terminal (`TECHNICAL.md` 12.1) and the test types `n` and Enter. Traces: INST-F-21, 22. | Exit 0. None of the files exists; `~/.local/share/watane` and `~/.local/share/watane-dev` are removed when empty. `~/.watane/marker` exists. Stdout holds `kept: ~/.watane (your home: identity, mods, channels)`. |
| INST-S-16 | **Uninstall, home deleted**: on a pseudo-terminal, the test types `delete` and Enter. Traces: INST-F-22. | Exit 0. `~/.watane` does not exist. Only the exact word `delete` counts: with `DELETE`, `yes` or `y` the home stays. |
| INST-S-17 | **Uninstall with no terminal**, and with `WATANE_INSTALL_TTY` set to a file that holds `delete` (the script does not read that variable). Traces: INST-F-22. | Exit 0. No question; `~/.watane` stays; stdout holds the kept line of INST-S-15. |
| INST-S-18 | **Uninstall of nothing**: no receipt. Traces: INST-F-21. | Exit 7. First line `watane install: nothing to uninstall: no receipt at ~/.local/share/watane/receipt`. Nothing is removed. |
| INST-S-19 | **`~/.local/bin` not on `PATH`**. Traces: INST-F-15. | Exit 0. Stdout holds `~/.local/bin is not on your PATH, so a new terminal will not find watane-dev yet.` and, for `SHELL=/bin/zsh`, the line `  echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc`. `~/.zshrc`, `~/.bash_profile` and `~/.profile` do not exist or are byte for byte as before. With the folder on `PATH`, stdout holds no `is not on your PATH`. |
| INST-S-20 | **Unsupported platform**: the fake `uname` says `Linux`, or `Darwin` with `x86_64` and no Rosetta, or `sw_vers` says `12.7`. Traces: INST-F-7. | Exit 2. First line `watane install: unsupported platform: Linux x86_64` (or `Darwin x86_64`, or `macOS 12.7 (needs 14.0 or later)`), then the line `watane installs on macOS 14.0 or later, on Apple silicon.` No download was tried (the fake release server logged no request) and `$HOME` is empty. |
| INST-S-21 | **Root and missing tools**: the fake `id -u` prints 0; or `curl` is missing from `PATH`. Traces: INST-F-5, 7. | Exit 1 for root with first line `watane install: do not run this as root or with sudo`; exit 2 for the tool with first line `watane install: the tool curl is missing`. Nothing downloaded or written. |
| INST-S-22 | **Gatekeeper step**: the fake `xattr` records its calls; the app carries a quarantine mark. Traces: INST-F-16. | Exit 0. The fake `xattr` was called once with `-dr com.apple.quarantine <home>/Applications/watane.app`. Stdout holds the Gatekeeper text of `TECHNICAL.md` 6.2, whose first line is `If macOS says "watane" cannot be opened, or could not verify that it is free of malware:`. |
| INST-S-23 | **The real quarantine mark** (clean Mac only): the installed app after INST-S-1 on a real macOS. Traces: INST-F-16. | `xattr -p com.apple.quarantine ~/Applications/watane.app` exits non-zero; `open ~/Applications/watane.app` exits 0 and, 5 s later, `pgrep -f` finds the app's executable (`TECHNICAL.md` 12.3) running; the test then quits it. If it is not running (macOS showed a prompt instead), the test fails with `the app did not start: see the screen` and the human records what macOS showed (`TECHNICAL.md` Q1). |
| INST-S-24 | **`watane-dev new` straight after** (clean Mac only): INST-S-2 answered `y`, then in a new empty folder, with `~/.local/bin` added to `PATH` (`watane-dev` finds rustup in `~/.cargo/bin` itself) and no `WATANE_*` variable. Traces: INST-F-2, 13. | `watane-dev --version` prints `watane-dev 0.1.0`. `watane-dev new reading-list` exits 0 and `reading-list/mod.toml` exists. `watane-dev doctor --json` exits 0 and its `result` has `toolchain.wasm32_wasip2` true, `allowlist.vendor` equal to `<home>/.local/share/watane-dev/vendor` and `watane` equal to `<home>/.local/bin/watane` (`<home>` the physical path of `$HOME`). `watane-dev test` in the new mod exits 0. |
| INST-S-25 | **`watane-dev` with the answer `N`** (clean Mac only): INST-S-3, then `watane-dev doctor`. Traces: INST-F-13. | `watane-dev doctor --json` exits non-zero; its `errors` hold one with `code` `env.toolchain` and `fix` `Install rustup with your package manager, then run: rustup default stable` (dev's text, unchanged by INST-F-13). No `~/.cargo` or `~/.rustup` was created by watane-dev or by the script. |
| INST-S-26 | **A version that is a path**: `--release '1/../../../../../evil/watane/releases/download/v1'`, then again with `WATANE_RELEASE` set to that value and no flag; also `0.1.0/x` and an empty value, both with `--release`. The default base (no `WATANE_RELEASE_BASE`) is used with a fake `curl` first on `PATH` that logs its arguments. Traces: INST-F-3, 6. | Exit 1 each time. Stderr's first line is `watane install: not a version: <the value>`, the second `a version looks like 0.1.0`. The fake `curl` logged no call, and `$HOME` is empty. `--release 0.2.0-rc1` passes this check. |
| INST-S-27 | **A receipt that names other files**: after INST-S-1, the receipt gains the lines `path <home>/.local/../Documents`, `path <home>/.local/share/fish`, `path <home>/.local/bin/mine`, `path <home>/.local/share/watane`, `path <other>/x` (`<other>` a second temporary folder) and `path ` (empty); `mine` is a file, and each other named path is a folder that holds a file `marker`; the files `~/.local/bin/watane.old.mine` and `~/.local/bin/watane.old.99999999` exist. Run `--uninstall` (no terminal); in a second run from the same start, install `0.2.0` instead. Traces: INST-F-19, 21. | Both runs: every `marker` file and `mine` still exist, and so does `~/.local/bin/watane.old.mine`; `~/.local/bin/watane.old.99999999` is gone. The uninstall exits 0, removes the files of INST-S-1, and its stderr holds one line `watane install: skipped <path>: not a path that watane installs` for each of the six added paths. The upgrade exits 0 and `VERSION` holds `0.2.0`. |
| INST-S-28 | **A home that is not a watane home**: after INST-S-1, `WATANE_HOME` set to `$HOME`, then to `/`, then to `<home>/x/..`, then to `<home>/link` and to `<home>/link/` where `link` is a symbolic link to `$HOME`, with a file `$HOME/keep`; on a pseudo-terminal the test types `delete` and Enter. Traces: INST-F-22. | Exit 0 each time. Stdout holds no `Type delete`, and holds `kept: <the value as shown> (not deleted: not a folder that watane can delete)`. `$HOME/keep` exists. With `WATANE_HOME=<home>/elsewhere` (a folder that exists) the question is asked and `delete` removes that folder only. |
| INST-S-29 | **The release workflow's rules**, read from `.github/workflows/release.yml` as text (a YAML parse if the crate already has one, else line rules). Traces: INST-F-3, 6. | The top-level `permissions` grants nothing. The job `package-macos` has `contents: read` and no `write`; every `actions/checkout` step has `persist-credentials: false`; no step uses `actions/cache`, `Swatinem/rust-cache` or a `cache:` input; every `uses:` ends in `@` and 40 hex digits; `cargo install tauri-cli` has `--version '=`. The `workflow_dispatch` trigger has no `inputs`. Only the job `release` has `contents: write`; it has `needs: package-macos`, an `if:` that requires a tag ref, no `actions/checkout`, and no step that runs `cargo` or `sh`. The build job runs `git merge-base --is-ancestor`. The command that holds `cargo tauri build` (its `\`-continued lines joined) holds ` -- --locked`, and the command that builds the three programs holds `--locked`. The job `release` names `environment: release` (Q16). |
| INST-S-30 | **Files that watane did not install**: no receipt; `~/.local/bin/watane` is a file, `~/Applications/watane.app` is a folder, `~/.local/bin/watane-dev` is a symbolic link to a missing target; a second run with a receipt of `0.1.0` and a `~/.local/share/watane-dev/vendor` that the receipt does not list. Traces: INST-F-25. | Exit 8 each time, before any download (the fake release server logged no request). Stderr's first line is `watane install: found files that watane did not install:`, then one line `  <path>` for each such path (`~/.local/bin/watane`, `~/.local/bin/watane-dev`, `~/Applications/watane.app` in the first run), then `Move them aside, then run this again, for example:` and one `  mv <path> <path>.mine` line for each. Every one of those paths is byte for byte as before and no other file was created. A third run: a recognised receipt of `0.1.0`, and `~/.local/bin/watane.new.<pid of the run>` exists as a folder holding `marker` (the test starts the script with `exec` from a shell whose pid it knows): exit 0, `~/.local/bin/watane` is a regular file that prints `watane 0.1.0`, and no `watane.new.*` is left (the stale name was removed, not copied into). A fourth run: no receipt file but a folder at `~/.local/share/watane/receipt`: exit 8, and that path is listed. |
| INST-S-31 | **An install folder outside the home**: `~/.local` is a symbolic link to a second temporary folder; then `~/Applications` is a symbolic link to it (and `~/.local` is a real folder); then `~/.local/bin` is a real folder whose `~/.local/share` is such a link; then `~/.local/share` is a real folder and `~/.local/share/watane-dev` is such a link; then `~/.local/bin` is a broken link to `<second>/bin`, which does not exist; then each of `~/.local/bin`, `~/Applications`, `~/.local/share/watane`, `~/.local/share/watane-dev` in turn is a link to `$HOME` itself (the target is the home, the folder is refused). Traces: INST-F-24. | Exit 2 each time. Stderr's first line is `watane install: <folder> resolves to <target>, outside your home folder <home>` (for a link to `$HOME` itself, `watane install: <folder> resolves to your home folder itself`), then `watane writes only under your home folder. Make it a real folder there, then run this again.` No download was tried and the second temporary folder is empty (in the broken-link case, `<second>/bin` still does not exist). With a recognised receipt and `~/.local/share/watane-dev` turned into a link to `<second>` that holds `vendor/marker`, `--uninstall` leaves `<second>/vendor/marker` in place and prints the `skipped` line of INST-S-27 for `vendor`. With `~/.local` a link to a folder inside `$HOME`, the install, an upgrade to `0.2.0` and `--uninstall` succeed (exit 0, the files land in and leave that folder). |
| INST-S-32 | **A failure at the receipt**: `0.1.0` is installed; the run installs `0.2.0` and the fake `mv` fails on the move of `receipt.new.<pid>` (the last move of the swap, so every other entry was already replaced); a second run with the fake `mv` working. Traces: INST-F-20. | First run: exit 5. `~/.local/bin/watane --version` still prints `watane 0.1.0`, the receipt still holds `version 0.1.0` and its seven `path` lines, the app, the vendor folder, `VERSION` and `install.sh` are those of `0.1.0` (bytes compared with a copy made before), and no `.new.*`, `.old.*`, `receipt.new.*` or `journal.*` file exists. Second run: exit 0, `upgrading watane 0.1.0 to 0.2.0`, never `found files that watane did not install`. |
| INST-S-33 | **A kill in the swap**: as INST-S-32, but the fake `mv` stops (sleeps) at the move of `receipt.new.<pid>` and the test sends `KILL` to the script's process group; then a run with the fake `mv` working. Traces: INST-F-20. | After the kill: the receipt still holds `version 0.1.0`, and `journal.<pid>`, `receipt.new.<pid>` and `.old.<pid>` names exist. The next run exits 0 (not 8), says `upgrading watane 0.1.0 to 0.2.0`, and afterwards no `.new.*`, `.old.*` or `journal.*` file exists and every path is that of `0.2.0`. A second kill test sends `KILL` after the receipt move (a fake `rm` sleeps on its first `.old.` removal): the next run exits 0 and removes the leftovers. A third kill test sends `KILL` while the fake `mv` sleeps on the app (the entries before it in the order of `TECHNICAL.md` 3.6, `install.sh` and `VERSION` among them, swapped; the app and the receipt not): the next run's undo puts `0.1.0` back whole, then installs `0.2.0`, as above. |
| INST-S-34 | **Command line tools**: a fake `xcode-select` whose `-p` fails, then one whose `-p` prints a path; a fake that records any other call. Traces: INST-F-26. | Missing: exit 0; stdout ends with the two lines `watane-dev needs the macOS command line tools (a C linker and git). Run this and follow macOS's own dialog:` and `  xcode-select --install`, for each answer to the offer and with `--no-toolchain`. Present: stdout holds no `xcode-select` and no `command line tools`. In both runs the fake recorded no call with `--install`. |
| INST-S-35 | **A journal that names more than it moved**: `0.1.0` is installed (receipt in place); the files of a killed run are planted with a dead pid `<n>`: `receipt.new.<n>` (a receipt of `0.2.0`), and `journal.<n>` holding `old <each entry of 0.1.0>`, `new <home>/Applications/watane.app` with no `moved` line, `old <home>/Documents/d`, `moved <home>/Documents/e`, and a last line `new <home>/.local/bin/watane-x` with no newline. `~/Documents/d.old.<n>` and `~/Documents/e` exist, each holding `marker`, and `~/Documents/d` does not; `~/.local/bin/watane-x` exists. Then the same start with a live process's pid in place of `<n>`. Traces: INST-F-19, 20, 25. | Dead pid: exit 0; the run installs `0.1.0` over the receipt's paths as in INST-S-13; `~/Documents/d.old.<n>/marker`, `~/Documents/e/marker` and `~/.local/bin/watane-x` exist unchanged, and `~/Documents/d` does not exist; no `journal.<n>` or `receipt.new.<n>` is left. Live pid: exit 5 with first line `watane install: another install is running (pid <n>)`, no request to the fake release server, every planted file unchanged. A `new` line alone never shows that watane put a file there. Undo retires the journal before deleting any staging evidence, so a repeated undo or recovery keeps a replacement app the person put there and leaves no journal after staging cleanup. With no receipt and only the journal's `new <home>/Applications/watane.app` (no `moved` line) and a foreign `~/Applications/watane.app`: exit 8 listing `~/Applications/watane.app`, which is unchanged. |
| INST-S-36 | **Fix2 refusals** (tests `ac1_`..`ac5_` of `script.rs`, `package.rs`): an archive with a link member is refused (exit 5, nothing installed, the outside file unchanged); a link planted at `journal.<pid>` or `receipt.new.<pid>` is not written through (exit 5); a home that resolves outside `$HOME` is kept and `rm -rf '<R>'` printed (pty); the toolchain question names `WATANE_RUSTUP_URL`; `bin/watane.mine` and a path through a link inside `share/watane` are skipped. Traces: IN-34 to IN-38. | As listed. |

The proof that a machine is clean is in `TECHNICAL.md` 12.4, and runs
before S23 to S25: no `cargo`, `rustc` or `rustup` on `PATH` or in
`$HOME`, no `wasm32-wasip2` anywhere, no checkout above the working
folder, and a `PATH` and `HOME` that hold none of them.

## 4. Not in this version

- Windows; Intel Macs; Linux (`TECHNICAL.md` 9).
- Signing and notarisation of the programs or the app (no Apple Developer
  account, decided 2026-11-15).
- An updater, a package manager entry (Homebrew), an installer package.
- Installing `git` or the Xcode command line tools.
