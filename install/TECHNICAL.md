# install: technical specification

How `install` does what `FUNCTIONAL.md` says. Requirements are cited as
`INST-F-<n>`, scenarios as `INST-S-<n>`; a sibling's sections as `dev 6.1`,
`app 12`. Section 15 lists the decisions made here, section 14 the
questions left open. Names that this document fixes (the script, the
archives, the paths, the flags, the variables, the exit codes, the texts)
are cited elsewhere and defined only here.

Stage 1 of section 11 is built (S50b-1: `install.sh` and `tests/script.rs`); `NOTES.md` records where the build differed from the first draft.

## 1. What `install` owns

| Item | Value |
|---|---|
| A script | `install/install.sh`: POSIX `sh`, the one file that a person runs (section 3). |
| A packer | `install/package.sh`: assembles one archive from built programs (section 4); the workflow and a person on a Mac run it. |
| A workflow | `.github/workflows/release.yml` (section 7). |
| Documents | this folder's `VISION.md`, `FUNCTIONAL.md`, `TECHNICAL.md`, `AGENTS.md`. |
| A crate | `watane-install` at `install/`: **tests only**, no library code beyond a `//!` line. It exists because the script's tests must run in `cargo test` with `tiny_http` and the repository's gate (`factory/check.sh`, AGENTS.md), and a folder of shell tests would not. It adds no dependency to any other crate and nothing depends on it. |

```text
install/
  VISION.md  FUNCTIONAL.md  TECHNICAL.md  AGENTS.md
  Cargo.toml          package watane-install; [dev-dependencies] tempfile, tiny_http
  src/lib.rs          one `//!` line; no code
  install.sh          the script served to people (section 3)
  package.sh          the packer (section 4)
  tests/
    script.rs         INST-S-1 to INST-S-22, INST-S-26 to 28, 30 to 35: runs install.sh against a fake release (section 12)
    workflow.rs       INST-S-29: the rules of 7.1 and 7.2 read from .github/workflows/release.yml
    package.rs        the tree package.sh gives (section 4.2), with fake programs
    support/mod.rs    the fake release, the fake tools
    support/pty.rs    runs a command on a real pseudo-terminal and types an answer; and runs one with no controlling terminal (12.1)
    clean_mac.sh      INST-S-23 to INST-S-25 and the clean-machine proof (12.3); only on a Mac
    fakes/            the fake tools that script.rs puts first on PATH (12.1)
.github/workflows/release.yml
```

`install` depends on no other subsystem's code. It packs the programs of
`host` (`watane`), `dev` (`watane-dev`, its allowlist vendor folder),
`node` (`watane-node`) and `app` (the app).

## 2. Where things are

The paths are fixed (the human's decision, 2026-11-18); a test moves them by moving `HOME`. `~` is `$HOME`.

| What | Path | Why there |
|---|---|---|
| the three programs | `~/.local/bin/watane`, `watane-dev`, `watane-node` | per-user, no root; `watane-dev` finds `watane` beside itself (`dev/src/preview.rs` `find_watane`: `current_exe().with_file_name("watane")`) |
| the crate allowlist | `~/.local/share/watane-dev/vendor/` | `dev/src/allowlist.rs` `vendor_dir`: `<folder of executable>/../share/watane-dev/vendor`; this is why `bin/` and `share/` are siblings |
| the script, version and receipt | `~/.local/share/watane/install.sh`, `VERSION`, `receipt` | so a person uninstalls and asks the status without the network |
| the app | `~/Applications/watane.app` | per-user Applications folder, which Launchpad and Spotlight read; no root |
| the home | `$WATANE_HOME`, else `~/.watane` | `watane_host::default_home()`; never touched by an install |
| the maker cache | `~/Library/Caches/watane-dev` | `dev/TECHNICAL.md` env; rebuildable; kept by an uninstall |
| the toolchain | `~/.rustup`, `~/.cargo` (`$CARGO_HOME`, `$RUSTUP_HOME`) | rustup's own; `watane-dev` finds `rustup` in `~/.cargo/bin` with no `PATH` entry (`dev 6.1` step 2) |

The prefix is `~/.local`; `bin/` and `share/` are under it.

Run-time needs of each program, from the code: `watane` needs nothing
beside it. `watane-dev` needs the vendor folder (above), `watane` beside
it or on `PATH` or `WATANE_BIN`, and the toolchain (`dev 6.1`); its
template is compiled in (`include_str!`). `watane-node` needs nothing.
`watane.app` runs the host in process (it depends on `watane-host`), and
needs nothing beside it. Of the system, the programs use only what macOS
ships (`otool -L` lists only `/usr/lib` and `/System`, asserted in 12.4).
`git` is needed by `publish` and `new --from`, and the allowlist's build
needs a host linker (`cc`): both come with the macOS command line tools.
The script checks only whether they are installed (`xcode-select -p`) and
prints the command that installs them (3.9); it never runs it (IN-30, Q10).

## 3. The script `install.sh`

### 3.1 Where it is served

Each release holds `install.sh` as an asset, with the release's version
written into it by the workflow (the line `DEFAULT_RELEASE="@RELEASE@"`
becomes `DEFAULT_RELEASE="0.1.0"`). Two URLs:

```text
latest   https://github.com/civic-tech-pioneer/watane/releases/latest/download/install.sh
pinned   https://github.com/civic-tech-pioneer/watane/releases/download/v0.1.0/install.sh
```

The one-line command, which the README and the app's first screen show
and which the release notes repeat:

```sh
curl -fsSL https://github.com/civic-tech-pioneer/watane/releases/latest/download/install.sh | sh
```

With a flag: `... | sh -s -- --no-toolchain`. The pinned script installs its
own version; the latest script installs the latest published release
(GitHub redirects `latest`, so the script needs no API call and parses no
JSON). A script that still holds `@RELEASE@` (a checkout) refuses to run
without `--release` or `WATANE_RELEASE`: `@RELEASE@` fails the version check of
3.2 (exit 1, `not a version: @RELEASE@`).

The script file is the same file as the copy in the archive
(`share/watane/install.sh`), so `sh ~/.local/share/watane/install.sh
--uninstall` works offline.

### 3.2 Flags and environment

```text
install.sh [--release <version>] [--no-toolchain]
install.sh --uninstall
install.sh --status
install.sh --help | -h
```

| Flag | Meaning |
|---|---|
| `--release <v>` | install version `<v>` (`0.1.0`, no `v`) instead of the script's own |
| `--no-toolchain` | answer no without asking; still prints the commands. There is no flag that answers yes (IN-24) |
| `--uninstall` | remove what the receipt lists (8.4) |
| `--status` | print the receipt's version, target and paths (8.1); with no receipt, exit 7 and `watane install: not installed: no receipt at ~/.local/share/watane/receipt` |
| `--help`, `-h` | print the usage above on stdout, exit 0 |

`--uninstall` and `--status` run steps 2 and 3 of 3.4 and the `HOME` checks of
step 4 (not the folder check of 3.4.1: an uninstall must still work when a folder
became a link, and 8.5 rule 3 keeps it from removing through one: the receipt, the
journals and their `.new`/`.old` names too, which 3.6 step 0 leaves alone when the
receipt's folder resolves outside the home), then 8.4 or 8.1; they check no version and no platform, and download
nothing.

`--no-toolchain` with `--uninstall` or `--status`, and any unknown flag
(`--toolchain` among them), exit 1 with `watane install: unknown or conflicting
flag <flag>` and the usage on stderr. A flag beats its variable.

**The version is checked before it is used.** The version, from
`--release`, `WATANE_RELEASE` or the baked value, goes into a URL path
(`.../download/v<v>/...`), file names and the receipt, and curl collapses
`..` segments of a URL path; so a value such as
`1/../../../../../other/repo/releases/download/v1` would fetch another
repository's archive and its own `SHA256SUMS`, and print `checked`. In step
1 the version must match the extended regular expression
`^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z]+(\.[0-9A-Za-z]+)*)?$` (checked with
`grep -Eq` on a value already refused if it holds a newline); otherwise exit
1 with `watane install: not a version: <v>` and, on the next line, `a
version looks like 0.1.0`, before any download (INST-S-26).

| Variable | Meaning | Default |
|---|---|---|
| `WATANE_RELEASE` | as `--release`; set to the empty text it counts as unset, as `WATANE_HOME` (an empty `--release ''` is refused by the version check) | the baked version |
| `WATANE_RELEASE_BASE` | the folder URL that holds the release's files, without a trailing slash; any scheme curl knows (`https://`, `http://`, `file://`) | `https://github.com/civic-tech-pioneer/watane/releases/download/v<release>` |
| `WATANE_RUSTUP_URL` | where the rustup installer script comes from | `https://sh.rustup.rs` |
| `WATANE_HOME` | the home, read only to name it in the uninstall question | `$HOME/.watane` |
| `HOME`, `PATH`, `SHELL`, `TMPDIR`, `CARGO_HOME`, `RUSTUP_HOME` | as everywhere | |

No variable answers a question: the offer of 3.7 and the question of 8.4 read the terminal (`/dev/tty`) and nothing else (IN-25).
`WATANE_RELEASE_BASE` and `WATANE_RUSTUP_URL` exist for the tests and for
a mirror. When either is set, the script prints `using <name>=<value>`
before it downloads, and does not add `--proto '=https'`.

### 3.3 Tools it may rely on

Stock macOS 14 and later, found on the default `PATH`
(`/usr/bin:/bin:/usr/sbin:/sbin`): `sh` (POSIX; macOS's `/bin/sh` is bash
in POSIX mode or dash-like; the script uses only POSIX), `curl`, `tar`,
`shasum` (else `sha256sum`), `uname`, `sw_vers`, `sysctl`, `id`, `mktemp`,
`mkdir`, `mv`, `cp`, `rm`, `rmdir`, `ls`, `cat`, `chmod`, `grep`, `sed`,
`awk`, `tr`, `head`, `dirname`, `readlink`, `xattr`, `xcode-select`, `kill`, `printf`, `test`. It uses no
`python`, `perl`, `jq`, `brew`, `git`, `openssl`, `wget`, `gh` or
`codesign`, and no bash-only syntax (no `[[`, arrays, `local`, `echo -e`,
`$'..'`, process substitution, `read -p`). It starts `set -eu`, `umask
022`, and its whole body is one function `main` called on the last line
(`main "$@"`), so that a download cut off half way runs nothing. Step 3 checks each tool with `command -v`; a missing one
is exit 2 (INST-S-21). `sw_vers`, `sysctl`, `xattr` and `xcode-select` exist only on macOS, so step 3
leaves them out and step 4 checks them once `uname -s` printed `Darwin`: on Linux the
person reads `unsupported platform: Linux <arch>` (9), not that `sw_vers` is missing.

### 3.4 The steps of an install

In this order. Each step either succeeds or ends the script as the last
column says; a step after step 5 that fails runs the cleanup first (3.8).

| # | Step | On failure |
|---|---|---|
| 1 | Parse flags and variables (3.2); check the version (3.2). Print `watane <v>: installing for <target>` once the target is known (step 4). | exit 1 |
| 2 | Refuse root: `id -u` is 0. | exit 1 |
| 3 | The tools of 3.3. | exit 2 |
| 4 | Platform: `uname -s` is `Darwin`; `uname -m` is `arm64`, or `x86_64` while `sysctl -n hw.optional.arm64` prints `1` (a shell under Rosetta); `sw_vers -productVersion` is 14.0 or later; `HOME` is absolute, is not `/`, and holds no newline and no `.` or `..` component (8.5). The target is `aarch64-apple-darwin`. Each install folder resolves inside the home (3.4.1). | exit 2 |
| 5 | Make `$TMPDIR/watane-install.XXXXXX` (`mktemp -d`; `/tmp` when `TMPDIR` is unset) and set the trap (3.8). | exit 1 |
| 6 | Read the receipt, if any: the installed version, for the `upgrading` message. Then the foreign-file check (3.4.2). Nothing is removed or written in this step. | exit 8 |
| 7 | Download `SHA256SUMS` into the temporary folder. Print `downloading watane-<v>-<target>.tar.gz`, then download the archive (3.5). | exit 3 |
| 8 | Verify (5): print `checked: sha256 matches SHA256SUMS of release v<v>`. | exit 4 |
| 9 | Extract: refuse a member name that is absolute or holds `..`; `tar -xzf` into the temporary folder; the top folder must be `watane-<v>-<target>` and hold `VERSION` with the text `<v>` and the three programs. | exit 5 |
| 10 | Replace the paths and the receipt in one swap (3.6 steps 0 to 4); the receipt is its last entry and the commit point. | exit 5 |
| 11 | Remove the quarantine mark (6.1). | ignored |
| 12 | Clean up after the commit (3.6 step 5): the `.old.<pid>` names, the journal, the paths that only the old receipt held. | a warning, ignored |
| 13 | The toolchain (3.7). | exit 6 |
| 14 | Print `watane <v> is installed.`, then the next-step block (3.9). | |

#### 3.4.1 Install folders stay inside the home

Step 4 resolves, with `pwd -P` (symbolic links followed), the home and each
folder that holds an entry of 3.6 or the receipt: `$HOME/.local/bin`,
`$HOME/.local/share/watane`, `$HOME/.local/share/watane-dev`,
`$HOME/Applications` (checking only `$HOME/.local/share` would let a link at
`share/watane-dev` lead out). A folder that does not exist yet is resolved
through its nearest existing parent (`<target>` is that parent's resolved path followed by the missing part); a symbolic link counts as existing even
when its target is missing, so a broken link is never stepped over (`d=$folder;
while [ ! -d "$d" ] && [ ! -L "$d" ]; do d=$(dirname "$d"); done; (cd "$d" &&
pwd -P)`): a link that `cd` cannot enter (broken, or to a file) is exit 2 with
the same text and `<target>` as `readlink` gives it. Otherwise a broken link
to `/Volumes/Off/bin` would pass as its parent and be written through once the
volume, or a folder another user creates under `/tmp`, appears. The resolved
home must not be `/`. Each must equal the resolved home or start with it
followed by `/`; otherwise exit 2 with

```text
watane install: ~/.local/bin resolves to /Volumes/Data/bin, outside your home folder /Users/alice
watane writes only under your home folder. Make it a real folder there, then run this again.
```

before any download (INST-S-31). A person whose folders live on another disk
makes the link point inside the home, or installs by hand; the script does not
follow a link out, because a link can be put there by another program of the
same user and the script would then write, as the person, where the person did
not look.

#### 3.4.2 Files that watane did not install

The paths to install are the entries of 3.6. Step 6 reads the receipt, if
there is one and it is a regular file whose first line is `watane-receipt 1`,
as the set of paths that watane installed: its `path` lines that the rule of
8.5 allows (a refused line is ignored here, so the INST-S-27 receipt still
upgrades); a missing, unreadable or unrecognised receipt is the empty set. A path of a journal left by a dead process (3.6) is added to the set only when the journal's rules of 3.6.1 say the swap moved it in: a valid `new <path>` line, a valid `moved <path>` line, and no `<path>.new.<n>`. A `new` line alone is not enough: it says that the path did not exist when the killed run began, not that watane put it there, and a file that the person put at that path afterwards (a `watane.app` dragged from a browser) must stay foreign (INST-S-35).
The receipt's own path counts as a foreign file when it exists (`-e` or `-L`)
and is not a recognised receipt, since step 10's `mv` onto a folder would put
the receipt inside it. For each entry that is not in that set, if `[ -e <path> ] || [ -L <path> ]`
(a file, a folder or a link, even a broken one) the script collects it. If any
was found it prints, to stderr, and exits 8 before it downloads anything:

```text
watane install: found files that watane did not install:
  ~/.local/bin/watane
  ~/Applications/watane.app
Move them aside, then run this again, for example:
  mv ~/.local/bin/watane ~/.local/bin/watane.mine
  mv ~/Applications/watane.app ~/Applications/watane.app.mine
```

One `  <path>` and one `  mv <path> <path>.mine` line for each (INST-S-30). It
overwrites nothing and there is no flag that forces it (IN-26). An upgrade
(INST-F-19) is unaffected: its paths are in the receipt. A person who
deleted the receipt must move the files aside, or uninstall by hand first.

A line `upgrading watane <old> to <new>` or `reinstalling watane <v>` is
printed at step 6 when a receipt exists; the paths of the old receipt
that the new archive no longer holds are removed in step 12, after the
new receipt is in place. Nothing outside the temporary folder is touched
before step 10.

### 3.5 Downloads

`curl -fsSL --proto '=https' --tlsv1.2 --retry 2 --connect-timeout 15
-o <file> <url>` (without `--proto` when 3.2 says so). `-f` makes a 404 an
error. No progress bar: curl's stderr goes to a file (below). URL of a file: `<base>/<name>`; names:
`SHA256SUMS`, `watane-<v>-<target>.tar.gz`. A curl error is exit 3 with
`watane install: cannot download <url>` and curl's own reason on the next
line: curl's stderr goes to a file of the temporary folder and is printed
after that first line, so the first line of stderr is the script's.

### 3.6 Replacing the paths

The paths are the entries of 8.1: `<prefix>/bin/watane`, `watane-dev`,
`watane-node`; `<prefix>/share/watane-dev/vendor`;
`<prefix>/share/watane/install.sh`, `VERSION`; `~/Applications/watane.app`;
and, as the last entry, the receipt `<prefix>/share/watane/receipt`. The swap
is one transaction: the receipt moves into place with the files, last, and
that move is the commit point. Before it, the old version and the old
receipt can be put back whole; after it, the new ones are in place and only
cleanup is left. So a failure or a kill at any step before the commit leaves
a state that the next run takes for the old install, never for foreign
files (INST-F-20, INST-S-14, 32, 33).

0. **Recover and clean.** For each file `<prefix>/share/watane/journal.<n>`
   (`<n>` digits only, a regular file) whose process is dead (`kill -0 <n>`
   fails) or is this script's own pid: if `receipt.new.<n>` still exists,
   undo it as in step 4 (below), with `<n>` in place of `<pid>`; if not, the
   commit happened, so do step 5 with `<n>` in place of `<pid>` and the old
   receipt at `receipt.old.<n>` (its paths that the receipt does not hold, if
   it still exists, then its `.old.<n>` names); then remove the journal. Then remove the stale names of 3.8. If `<path>.new.<pid>` or
   `<path>.old.<pid>` (this run's own pid) still exists (`-e` or `-L`), exit
   5 before any copy: `cp -R` onto an existing folder or link copies into
   it, and `mv` onto one moves into it.
1. **Stage.** For each entry except the receipt, copy it from the extracted
   tree to `<path>.new.<pid>` (same folder, so the move is a rename; `cp -R`
   keeps links inside the app). Create parent folders with `mkdir -p`.
   `chmod 755` the three programs and `install.sh`. Write the new receipt
   (8.1) to `<prefix>/share/watane/receipt.new.<pid>`, in the same folder
   as the receipt.
2. **Journal.** Write `<prefix>/share/watane/journal.<pid>`, one line for
   each entry, the receipt included: `old <path>` if `<path>` exists now
   (`-e` or `-L`), else `new <path>`. It is written directly, with no
   temporary name: no entry is swapped before it is complete, and the undo
   of step 4 touches only entries the swap reached, so a journal cut short
   by a kill undoes nothing.
3. **Swap.** For each entry in the order of the list above, the receipt
   last: if `<path>` exists, `mv <path> <path>.old.<pid>`; then append the
   line `moved <path>` to the journal; then
   `mv <path>.new.<pid> <path>`. The receipt alone is not moved aside: it is
   copied (`cp -p receipt receipt.old.<pid>`, when it exists), and
   `mv receipt.new.<pid> receipt` replaces it in one rename. So until that
   rename, the commit point, the file `receipt` is the old receipt; had it
   been moved aside, a kill just before the commit would leave no receipt,
   and the next run's step 6, which comes before step 0, would find every
   swapped path foreign (exit 8, against INST-S-33; S50a user-sim, dry run).
4. **Undo.** Any failure in steps 1 to 3, and `HUP`, `INT` or `TERM` while
   they run (the trap, 3.8), undoes the swap from the journal, or, if the
   failure came before the journal was written, from the `.new` names alone:
   for each `old <path>` line, only if `<path>.old.<pid>` exists (the swap
   reached this entry): `rm -rf <path>` if it exists, then
   `mv <path>.old.<pid> <path>` (for the receipt, only the `mv`, which
   replaces the file in one rename, so that no moment leaves it missing); an `old` entry with no `.old.<pid>` was not
   swapped and its `<path>` is the old file, which stays. For each
   `new <path>` line, only if the journal also holds `moved <path>` and
   `<path>.new.<pid>` no longer exists (it was moved in): `rm -rf <path>`.
   Only lines valid by 3.6.1 are used. Each removal only where the rule of 8.5
   allows; then remove each
   `.new.<pid>`, `receipt.new.<pid>` and the journal; exit 5 (or 129, 130,
   143). A running `watane` keeps its open file; the next start uses the new
   one. The old version and the old receipt are then as before the run
   (INST-S-14, 32).
5. **After the commit.** Remove the paths of the old receipt that are not
   in the new list, each only if the rule of 8.5 allows it (INST-S-27); then
   `rm -rf` each `.old.<pid>`, `receipt.old.<pid>` last, since step 0 reads
   it if this step is cut short; remove the journal last. A removal that fails here
   prints `watane install: could not remove <path>` on stderr and does not
   change the exit status: the new version is installed, and the next run
   removes a leftover `.old.<n>` of a dead pid (3.8). After an upgrade the
   script prints `If watane is open, quit it and open it again.`

A process that is killed by `SIGKILL` (or loses power) between steps 2 and
3 leaves its journal, its `.old.<pid>` and its `.new.<pid>` names. The old
receipt is still in place until the commit, so the paths that the swap
already replaced are in the old receipt or in the journal's `new` lines,
and the next run's foreign-file check (3.4.2) does not count them. The
next run's step 0 undoes the journal, which puts the old version back, and
then installs as if the killed run never happened (INST-S-33). After the
commit, a kill leaves only `.old.<n>` names and the journal. Step 0 tells
the two cases apart by `receipt.new.<n>`: while it exists the commit has not
happened and the journal is undone; once it is gone the receipt is the new
one, and step 0 finishes step 5 for it (the paths only the old receipt
held, the `.old.<n>` names, the journal).

#### 3.6.1 Which journal lines count

A journal is read back by another process, maybe after a kill, a power
loss or an edit, so it is input, like the receipt (8.5). Step 0, the undo of
step 4 and the check of 3.4.2 use a journal only when it is a regular file
named `journal.<n>` with `<n>` digits only, and use a line of it only when:

1. the line ends with a newline (a line cut short by a kill is ignored);
2. it is `old <path>`, `new <path>` or `moved <path>`, nothing else; and
3. `<path>` is, as text, exactly one of the entries of 3.6 for the current
   `HOME` (the seven paths and the receipt), which the rule of 8.5 allows.

Any other line is ignored, so a journal cannot name a path outside the
entries, and `mv <path>.old.<n> <path>` and `rm -rf <path>` act on entries
only. An `old` line is used only while `<path>.old.<n>` exists; a `new` line
removes or counts `<path>` only with its `moved` line and no
`<path>.new.<n>` (INST-S-35). Two runs at once are not supported: a run
that finds, at step 6, a journal of a live process other than itself stops
with exit 5 and `watane install: another install is running (pid <n>)`,
followed by `If no install is running, that number now belongs to another
program; remove <home>/.local/share/watane/journal.<n> and run this again.`,
before it downloads or changes anything,
so it never treats the other run's half-swapped files as foreign or as its
own.

### 3.7 The toolchain

**Detect**, with the checks of `dev 6.1` steps 2 and 4, so the script and
`watane-dev doctor` agree: `rustup` is the first of `command -v rustup`,
`${CARGO_HOME:-$HOME/.cargo}/bin/rustup`, `$HOME/.cargo/bin/rustup`; then
`<rustup> which rustc` gives the compiler, and `<rustc> --print sysroot`
a folder that must hold `lib/rustlib/wasm32-wasip2/lib`. Three states:

| State | Meaning | Offer | Question (printed on stdout, then `[y/N] `) |
|---|---|---|---|
| ok | rustc found and the target present | none | none; prints `toolchain: ok (rustup, wasm32-wasip2)` |
| no target | rustup and rustc, no target | `rustup target add wasm32-wasip2` | `watane-dev needs the wasm32-wasip2 target to build mods, and it is missing.` newline `Add the wasm32-wasip2 target with rustup? [y/N]` |
| none | no rustup, or rustup with no toolchain | A: run the rustup installer; B (rustup, no toolchain): `rustup toolchain install stable --profile minimal -t wasm32-wasip2`, then `rustup default stable` | `watane-dev needs the Rust toolchain with the wasm32-wasip2 target to build mods, and it is not installed.` newline `Install it with rustup (a few hundred MB, from https://sh.rustup.rs, into ~/.rustup and ~/.cargo)? [y/N]` |

**Answer.** Read one line from `/dev/tty`, when `( : < /dev/tty ) 2>/dev/null`
succeeds; a script piped from `curl` has the pipe as stdin, so it never reads
stdin. After `tr 'A-Z' 'a-z'`, only `y` and `yes` count; an empty line, end of
file, any other text, and no terminal are all `N` (INST-F-9, as `node` NO-10).
`--no-toolchain` is `N` with no question. No flag, variable or file means `y`:
with no terminal (an agent, a pipe, a CI job) the toolchain is not installed,
and the commands are printed for the person or the agent to run (F-GET-3,
IN-24, IN-25).

**On `y`**, in the order of the table's offer:
- A: download `$WATANE_RUSTUP_URL` to `<temporary folder>/rustup-init.sh`
  (`curl -fsSL --proto '=https' --tlsv1.2`), run `sh
  <temporary folder>/rustup-init.sh -y --profile minimal --no-modify-path
  -t wasm32-wasip2 < /dev/null`. `--no-modify-path` keeps rustup from
  editing the person's shell files; it still prints its own lines, which
  the script lets through. Then print the note `rustup put its programs in
  ~/.cargo/bin; watane-dev finds them there. To use cargo yourself, add
  that folder to your PATH.`
- no target, and B: run the rustup commands shown, in order.
- Print `toolchain: installed` and re-run the detection once; if it is
  not `ok`, print `toolchain: still incomplete after the install` and exit
  6.
A rustup failure is exit 6 with `watane install: the toolchain step
failed (rustup exited <n>)`; the programs stay installed.

**On `N`** (or no terminal) print, for state `none` A:

```text
toolchain: not installed
To install it later, run:
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --no-modify-path -t wasm32-wasip2
Then `watane-dev doctor` checks it again.
```

(INST-S-3 checks line 3 with its `\|` unescaped; the pipe is part of the
command.) For `no target`: `  rustup target add wasm32-wasip2`; for B:
`  rustup toolchain install stable --profile minimal -t wasm32-wasip2` and
`  rustup default stable`.
With no terminal the first line is `no terminal to ask on: the toolchain
was not installed`; with `--no-toolchain` it is `toolchain: skipped
(--no-toolchain)`.

`watane-dev` itself is unchanged (INST-F-13, DEV-F-13).

### 3.8 Cleanup

The trap runs `rm -rf <temporary folder>` on `EXIT`, and on `HUP`, `INT`
and `TERM` first undoes a swap in progress (3.6 step 4), then exits with 129, 130 and 143. A `.new.<pid>` or `.old.<pid>`
left by a kill between 3.6 steps 2 and 3 is found and removed (after its
journal, if there is one, was undone) by the next run, at 3.6 step 0 (after the checksum matched, so nothing outside the
temporary folder changes before step 10): a name `<path>.new.<n>` or
`<path>.old.<n>` where `<path>` is an entry of 3.6 that the rule of 8.5
allows, `<n>` is one or more digits and nothing else, and `kill -0 <n>`
fails or `<n>` is the script's own pid (a name with its own pid can only be
left by an earlier process that had the same pid). Any other name (`watane.old.mine`) is left alone.

### 3.9 What it prints

Stdout carries the progress and the result, stderr the errors. No colour,
no cursor movement, no emoji; each line is a fact or a command, so an
agent can read it (F-MAKE-5). `$HOME` is shown as `~`. The success output
of INST-S-1 (answer `N`, `PATH` ok), exactly:

```text
watane 0.1.0: installing for aarch64-apple-darwin
downloading watane-0.1.0-aarch64-apple-darwin.tar.gz
checked: sha256 matches SHA256SUMS of release v0.1.0
installed:
  ~/.local/bin/watane
  ~/.local/bin/watane-dev
  ~/.local/bin/watane-node
  ~/.local/share/watane-dev/vendor
  ~/Applications/watane.app
<the toolchain block of 3.7>
watane 0.1.0 is installed.

If macOS says "watane" cannot be opened, or could not verify that it is free of malware:
  watane.app is not signed by an Apple developer, because there is no account behind watane.
  Open System Settings, Privacy & Security, find the line about "watane" and press "Open Anyway".
  Or run this once, then open the app again:
    xattr -dr com.apple.quarantine ~/Applications/watane.app

Next:
  open ~/Applications/watane.app        the app
  watane-dev doctor                     checks the maker tools
  watane-dev new reading-list           makes a mod
```

Before `Next:` and after the Gatekeeper block, when `<prefix>/bin` is not
on `PATH` (a colon-wrapped match of `:<prefix>/bin:` in `:$PATH:`), the
block of 3.10. When the toolchain is not `ok` at the end of step 13 (state `none`
or `no target`, not installed), one more line follows the three `Next:` lines:
`  (install the toolchain first: see above)`.
`watane-dev new` is the real command (`dev/src/cli.rs`).

**Command line tools.** At the end of step 13 the script runs
`xcode-select -p >/dev/null 2>&1`. If it fails (no tools), whatever the answer
to the offer and whether or not the toolchain is `ok`, the script prints after
the `Next:` lines, on stdout:

```text
watane-dev needs the macOS command line tools (a C linker and git). Run this and follow macOS's own dialog:
  xcode-select --install
```

If it succeeds, the script prints nothing about the tools. The script never
runs `xcode-select --install` itself, since macOS opens its own dialog and
the install is the person's to start (IN-30, INST-S-34). The check says
nothing about whether `cc` works, only whether macOS has a developer
directory.

Errors: first line `watane install: <cause>`, then lines that say what to
do. Exit codes:

| Code | Meaning |
|---|---|
| 0 | done, or `--help`, or `--status` with a receipt, or uninstall done; includes the answer `N` |
| 1 | usage, root, no temporary folder |
| 2 | unsupported platform, a missing tool, or an install folder that resolves outside the home (3.4.1) |
| 3 | a download failed |
| 4 | the checksum does not match, or `SHA256SUMS` has no line for the archive |
| 5 | the archive is not a watane archive, the files could not be put in place (rolled back), or another install is running (3.6.1) |
| 6 | the toolchain step failed (watane itself is installed) |
| 7 | no receipt (`--status`, `--uninstall`) |
| 8 | files that watane did not install are where it would install (3.4.2); nothing was changed |
| 129, 130, 143 | `HUP`, `INT`, `TERM` |

### 3.10 `PATH`

Printed when `~/.local/bin` is not on `PATH`:

```text
~/.local/bin is not on your PATH, so a new terminal will not find watane-dev yet.
Add it, then open a new terminal:
  echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
```

The file is `~/.zshrc` for `SHELL` ending in `/zsh`, `~/.bash_profile`
for `/bash`, and for any other shell the line `export PATH=...` alone with
`add it to your shell's startup file`. The script edits no file (INST-F-15,
Q5). In this run, the lines of `Next:` give the full path
(`~/.local/bin/watane-dev doctor`).

## 4. The archive

### 4.1 Names

One archive for each platform of a release, named
`watane-<version>-<target>.tar.gz`, `<version>` without `v`, `<target>` a
Rust target triple. S50 ships one: `watane-0.1.0-aarch64-apple-darwin.tar.gz`.
The release's files, all assets of the GitHub release `v<version>`:

| File | What |
|---|---|
| `watane-<version>-aarch64-apple-darwin.tar.gz` | the archive |
| `SHA256SUMS` | one line per archive: `<64 hex>␣␣<file name>`, the format of `shasum -a 256` |
| `install.sh` | the script of 3.1, version baked in |

### 4.2 The tree

`tar -czf` of one top folder, made with `COPYFILE_DISABLE=1` so that
no `._` files enter it:

```text
watane-0.1.0-aarch64-apple-darwin/
  VERSION                         "0.1.0" and a newline
  bin/
    watane                        host/src/bin/watane.rs (release build)
    watane-dev                    dev (release build)
    watane-node                   node (release build)
  share/
    watane-dev/vendor/            the output of dev/scripts/vendor.sh: one folder <crate>-<version> for each crate of dev/allowlist/Cargo.lock
    watane/
      install.sh                  the script of 3.1
      VERSION                     "0.1.0"
  Applications/
    watane.app/                   the bundle of 7.3
```

`share/watane/VERSION` is in the tree so that the receipt can list it;
the top `VERSION` is what step 9 of 3.4 checks. `bin/` and `share/` keep
the sibling layout that `vendor_dir` needs. The vendor folder is about
26 MB and is part of the archive because the maker tools build offline
(F-QUAL-1, `dev 6.2`): a first `watane-dev test` must not fetch crates
(Q9). The three programs are one executable each, statically linked
Rust, linked only against system libraries.

`package.sh <target> <version> <out-dir>` assembles it from
`target/release/{watane,watane-dev,watane-node}`, the bundle at
`target/release/bundle/macos/watane.app`, `dev/scripts/vendor.sh` and
`install/install.sh` (with `@RELEASE@` replaced), checks the layout
(`VERSION`, the three programs executable, the vendor folder non-empty,
`Info.plist`), runs `codesign --force --sign -` on the programs and
`codesign --force --deep --sign -` on the app, then `codesign --verify
--deep --strict` on each (ad hoc: Apple silicon will not run an unsigned
binary; this is no identity, section 6), writes the archive, the baked
`install.sh` (the third asset of 4.1, the same bytes as
`share/watane/install.sh`) and `SHA256SUMS` into `<out-dir>`. It needs no network except what
`vendor.sh` needs. It exits non-zero and writes no archive when a check
fails.

## 5. Checksums

`SHA256SUMS` is made by `package.sh` (4.2), in the job that makes the
archive, with `cd <out-dir> && shasum -a 256 <archive file name> > SHA256SUMS`
(the bare name, and no `-b`: a path such as `dist-out/watane-...` or a
`*` before the name would leave the script's `awk` with no line, exit 4 for
every person), and uploaded in the same release. The script (step 8) takes the line whose second field is
exactly the archive name (`awk -v f=<name> '$2 == f { print $1 }'`; zero or
more than one match is exit 4), computes `shasum -a 256 <archive>` (else
`sha256sum`) and compares the two 64-character strings as text.

**What the check protects against:** a truncated or corrupted download; a
cache or mirror that serves a different archive than the release holds; a
mismatch between the script and the archive version; a mistake in the
release (an archive replaced and `SHA256SUMS` not).

**What it does not protect against** (AC5): anyone who can change both
files of the release, which means a person with write access to the
repository's releases, a stolen maintainer token, or a malicious workflow
change; the script itself, which arrives over TLS from the same place and
is not covered by `SHA256SUMS` (a person who pipes `curl` to `sh` trusts
GitHub and the repository's owner at that moment); a certificate authority
that the person's Mac trusts and that is misused; a compromised Mac. It is
not a signature: nobody vouches for the release's author. The trust is
stated in `spec/TECHNICAL.md` 6 ("Install" row and the not-protected list).
Q2 lists the stronger options and what each costs.

## 6. Gatekeeper

### 6.1 What happens and what the script does

Facts that the design rests on (checked on a clean Mac in INST-S-23, Q1):
macOS Gatekeeper checks a file that carries the extended attribute
`com.apple.quarantine`; `curl` and `tar` do not set it, and a browser
download does. The programs are ad-hoc signed (4.2), which Apple silicon
requires to execute them at all; an ad-hoc signature is not an
identity, and Gatekeeper's notarisation check does not apply to a file with
no quarantine mark. The app is therefore expected to open with no prompt
when installed by the script. The script still runs, as the person's own
user and with no `sudo`:

```sh
xattr -dr com.apple.quarantine "$HOME/Applications/watane.app"
```

so that an app copied by a browser or AirDrop in an earlier attempt, or an
archive that was quarantined on the way (a mirror behind a browser),
opens. A failure of this command is ignored (the app carries no mark, or
`xattr` found nothing to delete).

### 6.2 The text

Printed in the next-step block (3.9), always, because it costs a few lines
and spares a person a search if macOS does prompt:

```text
If macOS says "watane" cannot be opened, or could not verify that it is free of malware:
  watane.app is not signed by an Apple developer, because there is no account behind watane.
  Open System Settings, Privacy & Security, find the line about "watane" and press "Open Anyway".
  Or run this once, then open the app again:
    xattr -dr com.apple.quarantine ~/Applications/watane.app
```

Why this and not more steps: "Open Anyway" in Privacy & Security is the
route that macOS 15 and later leaves (a control-click "Open" no longer
bypasses), and the `xattr` line is one command that does the same for the
file. Both are the person's own act on a file they chose to download. The
text says the app is unsigned and why, so no one is led to think it is
notarised. It is the human's decision (2026-11-15) and S50a's reading of
"as few and as calm steps as macOS allows".

What removing the quarantine mark gives up: macOS's check that Apple's
notary service has seen the app, which watane cannot have. What it keeps:
the SHA-256 check of 5, and the person's own decision to run the line.

## 7. The release workflow

`.github/workflows/release.yml`. S50b writes it; this is its design.

### 7.1 Trigger and runner

```yaml
on:
  push:
    tags: ['v[0-9]+.[0-9]+.[0-9]+*']
  workflow_dispatch: {}      # no inputs: a hand run is always a dry run
permissions: {}              # nothing at the top; each job names its own
```

Two jobs, so that no code that the build runs holds a token that can
write to the repository:

| Job | Runs on | Permissions | Does |
|---|---|---|---|
| `package-macos` | `macos-14` (Apple silicon) | `contents: read` | steps 1 to 7 of 7.2; uploads the archive, `SHA256SUMS` and `install.sh` as a workflow artifact |
| `release` | `ubuntu-latest` | `contents: write`, `environment: release` (Q16) | only on a tag push, `needs: package-macos`; no checkout, no `cargo`, no script of the repository: it downloads the artifact and runs step 8 with `gh` |

Why: the build compiles every dependency's build script, `tauri-cli` and
the vendored crates; any of them that is hostile runs with the job's token
and, by default, the checkout's persisted credentials. With
`contents: write` there, a crate author could push to a branch, move a tag
or replace release assets. So the build job's checkout sets
`persist-credentials: false`, and its token can only read.

A `workflow_dispatch` run is the first job only (a dry run: no release);
it has no input that turns the release on. Actions are pinned to full
commit SHAs. A tag with a `-` (`v0.2.0-rc1`) makes a prerelease, which
`releases/latest` never serves. A tag whose commit is not on `main` is
refused (7.2 step 2), so a tag pushed on an unreviewed branch publishes
nothing.

### 7.2 Steps

1. Checkout with `persist-credentials: false` and `fetch-depth: 0` (step
   2 needs `main`'s history).
2. **Version and origin check**: the tag without `v` equals `version` in
   `host/Cargo.toml`, `dev/Cargo.toml`, `node/Cargo.toml`, `app/Cargo.toml`
   and `app/tauri.conf.json` (on a hand run, which has no tag, the version is
   `host/Cargo.toml`'s and the five must still be equal), and on a tag run `git merge-base
   --is-ancestor "$GITHUB_SHA" origin/main` succeeds; otherwise the job
   fails before it builds (Q7).
3. Rust: the runner's stable toolchain (at least the `rust-version` of
   `host/Cargo.toml` and `dev/Cargo.toml`); no Wasm target, since the job
   builds and tests no mod. **No cache** (`actions/cache`,
   `Swatinem/rust-cache` or `setup-*` caching): a release restores nothing
   that another workflow run wrote, because a cache entry from another run
   (`target`, or the unpacked sources under `~/.cargo/registry/src`, which
   cargo does not re-hash) would enter the archive unchecked. A release
   build is slower; that is its price.
4. `MACOSX_DEPLOYMENT_TARGET=14.0 cargo build --release --locked -p
   watane-host -p watane-dev -p watane-node`.
5. The app (7.3).
6. `install/package.sh aarch64-apple-darwin <version> dist-out/`.
7. **Smoke**: `sh install/tests/clean_mac.sh --from "$PWD/dist-out" --fresh-home`
   (12.3): the install into a temporary `HOME` with `file://` as the
   release base, the assertions of 12.4, `watane --version`,
   `watane-dev --version`, `watane-node --version`, `otool -L`. The
   runner has no terminal, so the toolchain is not installed; the runner's own rustup is hidden by the
   `PATH` of the test.
8. Release (the `release` job, tag runs only): `gh release create v<version>
   --draft --title "watane <version>" --notes <text> <archive>
   SHA256SUMS install.sh` (`GITHUB_TOKEN`, `--repo "$GITHUB_REPOSITORY"` since
   there is no checkout), where `<text>` is the one-line command of 3.1, the
   pinned line for this version, and a link to `install/TECHNICAL.md` 6.2 for
   the Gatekeeper step, then `gh release edit
   v<version> --draft=false` as the last step, so that `latest/download`
   never serves a release with missing files. A failure before that leaves
   a draft, which no URL serves.

No job writes a cache or a secret into the archive, uses a secret but
`GITHUB_TOKEN`, or runs a step from a pull request (it triggers only on
tags and by hand). INST-S-29 checks the file `release.yml` for these rules
in `cargo test`.

### 7.3 Bundling `watane.app`

`app/tauri.conf.json` has `bundle.active: false`, so a plain
`cargo build` makes a binary, not a bundle. The workflow does not edit
that file; it asks the bundler for the app with a config override:

```sh
cargo install tauri-cli --version '=<x.y.z>' --locked   # an exact version, not cached
cd app && cargo tauri build --bundles app \
  --config '{"bundle":{"active":true,"targets":["app"],"macOS":{"minimumSystemVersion":"14.0"}}}' \
  -- --locked
```

`tauri-cli` passes what follows `--` to `cargo build`, so the app is built
with the lock like the three programs of step 4; without it cargo would
resolve the dependencies of `app` anew and the release could carry crates
that nobody reviewed. S50b checks it as INST-S-29 checks the rest of the
file: the command of `release.yml` that holds `cargo tauri build`, its
`\`-continued lines joined, must hold a word `--` followed by `--locked`,
and the command of step 4 must hold `--locked`.

The version of `tauri-cli` is exact (S50b picks it and a person bumps it
by hand): a range would let a new upload to crates.io run in the release
build. The command writes `target/release/bundle/macos/watane.app` (the product name
is `watane`, `tauri.conf.json`) with `Info.plist` merged from
`app/Info.plist` (the `NSAppTransportSecurity` entries without which no
mod page loads, `app 12`), the icon, and the version of
`tauri.conf.json`. `cargo tauri build` also builds the binary with
Tauri's `custom-protocol` feature, which a plain `cargo build --release`
does not. `package.sh` then re-signs ad hoc (4.2). The bundle's
executable name is whatever Tauri gives; no document or script depends on
it. If this route fails on the runner, the fallback (Q6) is to assemble
`Contents/MacOS`, `Contents/Info.plist` and `Contents/Resources` by hand
in `package.sh`; the tree of 4.2 does not change.

## 8. Version, upgrade, uninstall

### 8.1 Version and the receipt

A person learns the version with:

| Command | Prints |
|---|---|
| `watane --version` | `watane 0.1.0` (exists: `host/src/bin/watane.rs`) |
| `watane-dev --version` | `watane-dev 0.1.0` (exists: clap `version`) |
| `watane-node --version` | `watane-node 0.1.0` (stage 2 adds it) |
| the app | `Info.plist` `CFBundleShortVersionString` = `0.1.0`; Finder's Get Info shows it |
| `sh ~/.local/share/watane/install.sh --status` | the receipt, below |

`--status` prints, for the receipt of INST-S-1 (exit 0):

```text
watane 0.1.0 (aarch64-apple-darwin)
~/.local/bin/watane
~/.local/bin/watane-dev
~/.local/bin/watane-node
~/.local/share/watane-dev/vendor
~/.local/share/watane/install.sh
~/.local/share/watane/VERSION
~/Applications/watane.app
```

The receipt, `<prefix>/share/watane/receipt`, text, staged as
`receipt.new.<pid>` in step 10 and moved into place as the last entry of the
swap (3.6), so it is never half written and never ahead of its files:

```text
watane-receipt 1
version 0.1.0
target aarch64-apple-darwin
path /Users/alice/.local/bin/watane
path /Users/alice/.local/bin/watane-dev
path /Users/alice/.local/bin/watane-node
path /Users/alice/.local/share/watane-dev/vendor
path /Users/alice/.local/share/watane/install.sh
path /Users/alice/.local/share/watane/VERSION
path /Users/alice/Applications/watane.app
```

Absolute paths, one for each entry of 3.6; a path is the rest of its
line, so it may hold a space but not a newline (a `HOME` with a newline
is refused in step 4, exit 2). The receipt names itself
nowhere; it is removed by the uninstall as the last file.

### 8.2 Upgrade

Run the line again (`latest` gives the newest published release) or the
pinned line. Step 6 reads the receipt; step 10 replaces the paths of the
new archive, then removes the paths that only the old receipt holds; the
receipt is rewritten. Kept: the home, the toolchain, the cache, any file
the person put in `~/.local/bin`. Not checked: whether the new version
needs a newer home format; each program handles its own home (`host`,
`dist`, `log` own that), and `install` adds no migration. Downgrade
(`--release` older) is the same procedure and prints `upgrading watane
<new> to <old>` as written, with the numbers as they are.

### 8.3 Check for a new version

None. watane never checks (VISION). The release page is the notice; a
person who wants to know runs `--status` and compares. An in-app notice
is a feature of `app`, not of `install` (Q11).

### 8.4 Uninstall

`install.sh --uninstall` (INST-F-21, 22):

1. Stop, as an install does, on a journal of a live process other than
   itself (3.6.1, exit 5). Recover first, as 3.6 step 0 (a journal of a dead process is undone or
   finished), so that a killed upgrade leaves neither a missing receipt nor
   `.old.<n>` names behind. Then read the receipt; none is exit 7
   (INST-S-18).
2. `rm -rf` each `path` of the receipt that the rule of 8.5 allows; a path
   it refuses is left in place, with `watane install: skipped <path>: not a
   path that watane installs` on stderr (INST-S-27); then the stale names of
   3.8; then the receipt, when 8.5 allows it (else the same `skipped` line); then `rmdir` (not `rm -r`)
   `<prefix>/share/watane`, `<prefix>/share/watane-dev`, `<prefix>/share`
   and `<prefix>/bin`, each only if empty.
3. Print `uninstalled watane 0.1.0`.
4. Print what it kept, one line each (the home's line comes after step 5,
   and only when the home exists: `deleted: <home>`, or
   `kept: ~/.watane (your home: identity, mods, channels)`, or the kept line
   of a home that may not be deleted, below):
   `kept: ~/Library/Caches/watane-dev (maker cache; safe to delete)` when
   it exists, `kept: ~/.rustup and ~/.cargo (the Rust toolchain; remove
   with rustup self uninstall)` when they exist.
5. The home question, only when the home exists, a real terminal answers
   (`/dev/tty` opens), and
   the home is a folder that may be deleted: an absolute path with no `.`
   or `..` component and no trailing `/`, not `/`, not `$HOME`, and not a
   folder that holds `$HOME` (`$HOME/` does not start with `<home>/`); and
   the same holds for its resolved path `R` (`cd <home> && pwd -P`) against
   the resolved `$HOME`, so that `WATANE_HOME=~/link` with `link` pointing
   at `~` or `/` is refused. The deletion is `rm -rf -- "$R"`, then `rm`
   of `<home>` when it is a link. Otherwise no question
   is asked and the kept line reads `kept: <home> (not deleted: not a
   folder that watane can delete)` (INST-S-28), so that a `WATANE_HOME` set
   to `~` by mistake cannot turn one typed word into the loss of every file
   of the person. The question:
   `Delete the watane home ~/.watane? It holds your identity and your mods,
   and cannot be restored without a backup. Type delete to delete it, or
   press Enter to keep it: `. Only the exact text `delete` deletes it
   (`rm -rf`); then print `deleted: ~/.watane`. Anything else, an end of
   file and no terminal keep it (INST-S-15 to 17). No flag, variable or file answers this
   question (Q12). Tests type the answer on a pseudo-terminal (12.1).

The home is `$WATANE_HOME` if set and not empty, else `~/.watane`, as
`watane_host::default_home()`.

### 8.5 Which paths the script may remove

A receipt is a file in the person's folders, read back by a later run; an
edit, a corruption, a receipt written under another `HOME`, or a bug of a
later version must not make the script delete more than watane installs.
So every removal (an uninstall, the paths that only an old receipt holds,
the stale names of 3.8) first checks the path as text, against the current
`HOME` (which step 4 requires to be absolute, not `/`, with no newline and
no `.` or `..` component). A path is allowed only when:

1. it holds no newline and no `//`, does not end in `/` (`rm -rf
   link/` deletes what the link points to), and no component is `.` or
   `..` (no `/./`, `/../`, and it does not end in `/.` or `/..`); and
2. it is exactly `$HOME/Applications/watane.app`; or it is
   `$HOME/.local/bin/<name>` where `<name>` starts with `watane` and holds
   no `/`; or it lies strictly below `$HOME/.local/share/watane/` or
   `$HOME/.local/share/watane-dev/` (not the folder itself); and
3. the folder that holds it resolves inside the resolved home, as in
   3.4.1 (`--uninstall` too checks this, so a link put at
   `share/watane-dev` after the install does not make it delete `vendor`
   elsewhere).

A prefix test alone is not the rule: `$HOME/.local/../Documents` starts
with `$HOME/.local/` and is refused by 1; `$HOME/.local/share/fish` is
under `$HOME/.local/` and is refused by 2. The rule names folders, not a
list of today's seven entries, so a later version may add an entry below
them and an older script still removes it on a downgrade. `rm -rf` of an
allowed path that is a symbolic link removes the link, not what it points
to.

## 9. Linux (TC-7, second)

**Out of scope for this version, by decision (IN-9).** The programs would
build for `x86_64-unknown-linux-gnu`, and the script's shape (archive,
checksums, `~/.local`) would carry over, but:
- the app on Linux is only tested in an arm64 Docker image under `xvfb`,
  with the front tests not run (`app 12` stage F, `app/NOTES.md`); a Linux
  app that nobody has seen on a desktop is not something to hand a person;
- it needs system packages (GTK 3, WebKitGTK 4.1) that the script cannot
  install without root, which it never uses;
- there is no Linux CI runner plan or clean-machine test in this slice, and
  an install with no test is not shipped;
- the first journey is Alice on a Mac (VISION).

The script stops on Linux with exit 2 and `unsupported platform: Linux
<arch>` and, on the next line, `watane installs on macOS 14.0 or later, on
Apple silicon.` (the same second line for every refusal of step 4's platform
check, INST-S-20). The design leaves room: a target in
4.1, a platform case in step 4, a second runner in 7.1. Stage 6 of the
build plan is the CLI-only Linux archive (`watane`, `watane-dev`,
`watane-node`, vendor), with no app, on `ubuntu-22.04` for glibc 2.35;
Q14.

## 10. Security

| Property | Enforced by | Test |
|---|---|---|
| Nothing is written before the download is verified against the checksums of the same release | steps 7 to 10 of 3.4 | INST-S-8, 9 |
| A failed or interrupted install leaves nothing, and a failed upgrade leaves the old version | the temporary folder and trap (3.8), the replace and undo (3.6) | INST-S-8, 11, 14 |
| No root, no `sudo`, no password | step 2 refuses root; every path is under `$HOME`, links followed (3.4.1; INST-F-5, 24) | INST-S-21, 31; the test runs as a normal user and asserts no write outside `$HOME` |
| The toolchain is installed only after a typed yes (no flag says yes) | 3.7 | INST-S-2 to 5 |
| An install never overwrites a file that watane did not install | 3.4.2 | INST-S-30 |
| An install writes only inside the home, links followed | 3.4.1 | INST-S-31 |
| No account, no telemetry, no update check, no background process | the script has no other URL than 3.2 lists and starts nothing that outlives it | INST-S-1 (the fake server's request log holds exactly `SHA256SUMS` and the archive); `ps` after the run |
| The home is never deleted without a typed `delete`, and never when it is `/`, `$HOME` or a folder above it | 8.4 | INST-S-15 to 17, 28 |
| The version cannot steer the download to another path or repository | the version check of 3.2 | INST-S-26 |
| An uninstall or upgrade removes only what watane installs, whatever the receipt says | the rule of 8.5 | INST-S-27 |
| No code of the build holds a token that can write to the repository; a release restores no cache; a hand run publishes nothing; a tag off `main` publishes nothing | two jobs, `persist-credentials: false`, no cache, no dispatch input, the ancestor check (7.1, 7.2) | INST-S-29 |
| The programs run only what is in the person's own folders | the archive's tree (4.2) | package test; 12.4 `otool` |

**Trust in the download.** TLS to `github.com` (certificate checked by
macOS's curl against the system roots, `--proto '=https' --tlsv1.2`; a
redirect to `objects.githubusercontent.com` stays on https) delivers the
three files of one release; the SHA-256 in `SHA256SUMS` of the same
release ties the archive to the checksums file (5). The person trusts
GitHub, the owner of the repository and that release's workflow. This adds
no boundary of the running system: it is a boundary of the act of
installing, and `spec/TECHNICAL.md` 6 names it in one row and one
not-protected line. Rustup, if the person says yes, is rustup's own trust
(TLS to `sh.rustup.rs`); `install` adds none.

**Not protected:**
- a compromised repository, maintainer account or release workflow: it
  replaces the archive and the checksums together (5);
- the script itself, which is not covered by a checksum when it is piped
  (a person who wants to read it first runs `curl -fsSLO <url>`, reads it,
  then `sh install.sh`; the README says so);
- a misused certificate authority, or a compromised Mac;
- the executable code of the programs: they are built by the workflow from
  the repository and are not reproducible builds; nobody has audited the
  archive against the source;
- another program of the same user, which can replace the installed files
  afterwards: the same-user limit of `spec/TECHNICAL.md` 6;
- the rustup installer, when the person says yes (it is code from the
  network; the question names its origin).

## 11. Build plan

S50b is stages 1 to 3, S50c stages 4 and 5; stages 6 and 7 are later. Sizes: S is under
a day for an agent, M is a day.

| Stage | Build | Touches (folders) | Needs | Done when |
|---|---|---|---|---|
| 1 Script | `install.sh` (3), the crate `watane-install` with `tests/script.rs`, `tests/support/`, `tests/fakes/`; add `install` to the root `Cargo.toml` members and run `cargo hakari generate` | `install/`, root `Cargo.toml`, `workspace-hack/` (generated) | none | INST-S-1 to INST-S-22, INST-S-26 to 28, 30 and 31 pass in `cargo test`; the swap of 3.6 is built whole. Size M |
| 2 Versions | `watane-node --version`; `--version` of the three programs prints `<name> <version>` and exits 0 (a test each) | `node/` (bin and its `program.rs` test), `host/` and `dev/` only if a test finds a difference | none | `watane-node --version` prints `watane-node 0.1.0`. Size S |
| 3 Packer | `package.sh` (4), `tests/package.rs` | `install/` | stage 1 (the script copy) | the tree of 4.2 is made from fake programs; refused when a part is missing; INST-S-32 to 35 pass in `cargo test`. Size S |
| 4 Workflow | `.github/workflows/release.yml` (7); the bundle step (7.3); README line with the one-liner; a dry run on `macos-14` | `.github/`, `install/`, `README.md`; `app/` only if the bundle needs a field of `tauri.conf.json` or `Info.plist` | stages 1 to 3 | INST-S-29 passes in `cargo test`; a `workflow_dispatch` dry run uploads an archive that stage 5's test installs. Size M |
| 5 Clean Mac | `tests/clean_mac.sh` (12.3, 12.4); the second macOS user prepared by the human; the first real release `v0.1.0` cut by the human | `install/`; the human | stage 4 | INST-S-1, 23, 24, 25 pass on the clean Mac against the real release. Size M, local |
| 6 App notice | the app shows the one-liner and the Gatekeeper text where `app` shows its first start (optional) | `app/` | stage 5 | not required for S50b |
| 7 Linux | the CLI-only archive (9) | `install/`, `.github/`, `dev/` if a path differs | S50b | a Linux clean-machine run; not scheduled |

Stages 1 and 2 are independent; 3 waits for 1; 4 for 1 to 3; 5 for 4.
S50b (the script and its archive) is stages 1 to 3; S50c ("a person
installs watane on a clean Mac") is stages 4 and 5; the human is needed
for stage 5 only.

## 12. Test plan

### 12.1 In `cargo test` (stage 1 and 3)

`tests/script.rs` runs `sh install/install.sh` as a child with:
- a cleared environment (`env_clear`, so no `CARGO_HOME`, `RUSTUP_HOME` or
  `WATANE_*` of the developer leaks in), then `HOME` an empty temporary
  folder and `TMPDIR` another;
- `PATH` = `<fakes dir>:<symlink dir>` where the symlink dir holds links to
  only the real tools of 3.3 that the test needs (and `gzip`, which GNU tar runs and macOS's tar does not), so a tool that 3.3 does
  not list cannot be used by accident (a test that the script ran a
  tool outside 3.3 fails with `command not found`);
- fake tools in `tests/fakes/`: `uname`, `sw_vers`, `sysctl` (platform);
  `id` (root); `xattr` (records its arguments in a log); `rustup` and
  `rustc` (state of the toolchain from files of the test); `mv` (fails, or
  sleeps until killed, when an argument matches a shell glob that the file `mv_fault` of the test names, mode on line 1 and glob on line 2; the real `mv` is reached through `$FAKE_REAL`, a folder that is not on `PATH`; the globs are `*watane.app.new.*` for INST-S-14 and 33, `*receipt.new.*` for INST-S-32 and 33, so that the undo's `mv <path>.old.<pid>` is not hit);
  `rm` (same, from the file `rm_fault`; INST-S-33 names `*.old.*`);
  `xcode-select` (`-p` fails or prints a path, and records any other call:
  INST-S-34);
  `curl` is the real one;
- `WATANE_RELEASE_BASE=http://127.0.0.1:<port>/release` served by a
  `tiny_http` thread of the test that logs each request path (the oracle of
  "no request" and "exactly these two"), or `file://` for the tests that
  need no log; the fake release holds a `SHA256SUMS` made by the test (with
  `shasum -a 256` or `sha256sum`; the crate adds no hash dependency) and a fake
  archive: `tar -czf` of a tree with the layout of 4.2 whose programs are
  small shell scripts that print `<name> <version>` for `--version`;
- **no terminal**: each test that expects none starts the script as
  `perl -MPOSIX -e 'POSIX::setsid(); exec @ARGV' sh install.sh ...` (`perl`
  is on stock macOS and on the Linux runners; it is a tool of the test, not
  of the script), so that there is no controlling terminal even when
  `cargo test` runs in a terminal and `/dev/tty` cannot be opened;
- **every question** (the offer, INST-S-2, 3, 5 (a), 7; the uninstall, INST-S-15,
  16, 28) is answered on a real pseudo-terminal by `support/pty.rs`: it starts the script under
  `script(1)` (macOS: `script -q /dev/null sh install.sh <flags>`;
  util-linux: `script -qec "sh install.sh <flags>" /dev/null`), reads
  its output until the line that ends the question (`[y/N] ` or `keep it: `) appears, then writes the answer and a newline to `script`'s
  stdin. It needs no `expect`. These tests are `#[ignore = "slow: needs a
  pseudo-terminal from script(1)"]`, so the gate runs them with the other
  slow tests of the files a task touched. If `script(1)` does not forward a
  piped stdin on macOS, the fallback is `openpty` in this test-only module
  (the crate then allows `unsafe` in `support/pty.rs` alone). The clean-Mac
  run (12.3 step 5) types the answer the same way, so the real macOS
  `script` covers the path as well;
- `WATANE_RUSTUP_URL=file://<fake installer script>` that writes
  `$HOME/.cargo/bin/rustup` (a fake that reports state `ok` afterwards) and
  records its arguments.

Also:
- the script under test is a copy of `install/install.sh` with `@RELEASE@`
  replaced by `0.1.0`, as `package.sh` does; the checkout copy itself refuses
  to run (3.1);
- each test puts on `PATH` only the fakes its scenario has: `rustup` and
  `rustc` are absent unless the scenario says they are there (INST-S-6, 7),
  since the detection of 3.7 takes any `rustup` that `command -v` finds;
- `PATH` of the child holds no `perl` or `script`, so the test starts them by
  absolute path (`/usr/bin/perl`, `/usr/bin/script`): `Command` searches the
  child's `PATH`;
- on a pseudo-terminal stdout and stderr arrive merged, each line ends in
  `\r\n`, and the typed answer is echoed after the question; the oracles of
  those scenarios match lines after removing `\r`, and an oracle that names
  stderr is checked in the no-terminal scenarios only;
- INST-S-5 (a) has no question to wait for: the test types `y` and Enter
  when the line `checked: sha256 ...` appears.

The tests run as a normal user and compare a listing of `$HOME` before and
after (INST-S-1, 2, 8, 20). They run on Linux and macOS, since the
platform is faked. `tests/package.rs` runs `package.sh` over fake
programs and a fake vendor folder and lists the tree against 4.2; the
`codesign` and `otool` steps are skipped when the tool is absent
(`#[cfg(target_os = "macos")]` for the signature check).

No test of this crate reaches the internet or the real GitHub.

### 12.2 What only a clean Mac can show

INST-S-23 to INST-S-25, and: a real `curl` over real TLS from the real
release; the real `shasum`, `tar` and `xattr` of macOS; ad-hoc signed
programs actually executing on Apple silicon; the app launching; and
`watane-dev new`, `doctor` and `test` with a real rustup, a real
`wasm32-wasip2` target and the vendored crates. These need a Mac, a
network and a published release.

### 12.3 Where a clean Mac comes from (S50b's test)

Options:

| Option | Is | Costs | Shows |
|---|---|---|---|
| **A second macOS user** on the human's Mac (recommended) | a user `wclean`, standard account, no Homebrew and no `~/.cargo`; created once (System Settings or `sysadminctl`), logged in over `su - wclean` or fast user switching | one-time setup by the human (needs the admin password); shares the machine's `/Applications`, its Homebrew (not on the new user's `PATH`) and the system's command line tools | a real `$HOME` with no cargo, a real quarantine and Gatekeeper, a real GUI session for the app, no reset needed: `rm -rf` the home's files between runs |
| A VM (Tart, UTM, Parallels) with a fresh macOS image | a clean guest, snapshot per run | an image of macOS (licence, 15 to 20 GB), time to restore | everything, including a Mac with no command line tools; the strongest clean |
| A GitHub `macos-14` runner | a clean VM per run; has rustup, Homebrew and Xcode preinstalled, so it is **not clean** until those are hidden | no setup; needs `PATH` and `HOME` to be reset by the script, which is what 12.4 does; no GUI session for Gatekeeper | the smoke of 7.2 step 7 only |

None of the three options except the VM shows a Mac without the command line tools (Q10): a second user and the runner share the machine's tools. S50b uses the second user for stage 5, and the runner for the smoke of the
workflow (the fresh-`HOME` form of the same script). The VM is the upgrade
path if a clean machine with no command line tools is wanted (Q10).

`tests/clean_mac.sh [--from <dir>] [--fresh-home]` does the whole run
with no person at the keyboard; each answer is typed on a pseudo-terminal by the same `script(1)` method as 12.1:
1. `--fresh-home` (the runner): `H=$(cd "$(mktemp -d)" && pwd -P)` (the
   physical path: `doctor --json` reports canonical paths, `/private/var`); otherwise `H=$HOME` of
   the clean user. `cd $H`.
2. Run every later command under `env -i HOME="$H" PATH=/usr/bin:/bin:/usr/sbin:/sbin
   TERM=dumb`.
3. The assertions of 12.4, then the install: `curl -fsSL <url> | sh`
   (or `sh <from>/install.sh` with `WATANE_RELEASE_BASE=file://<from>`).
4. INST-S-1, then INST-S-2 (answer `y`) from a fresh home, INST-S-23, 24.
5. INST-S-3 and 25 from a fresh home; INST-S-15 and 16 (uninstall, answered on a pseudo-terminal, 12.1).
6. Print `CLEAN-MAC PASS` or the first failed assertion with its command.

With `--fresh-home` (the runner's smoke, 7.2 step 7) only steps 1 to 3 run,
then INST-S-1 with no terminal, the three `--version` lines and the `otool`
assertion of 12.4; no answer is typed, no toolchain is installed and no app
is opened. Without it, "a fresh home" means: `rm -rf` exactly `$H/.local`,
`$H/Applications/watane.app`, `$H/.cargo`, `$H/.rustup`, `$H/.watane` and
`$H/Library/Caches/watane-dev`, never `$H` itself, then the assertions of 12.4
again. INST-S-23 needs a GUI session of the clean user (fast user switching,
not `su - wclean`: `open` from `su` has no window server session of that
user), so the whole run is started from Terminal inside that session; the
human logs in, the script then needs no one at the keyboard.

The script reads JSON (`doctor --json`) and plists with `plutil -extract
<key path> raw -o - <file>` (stock macOS, reads JSON too), so it needs no
`jq` or `python3`. "The app's executable" is
`Contents/MacOS/<CFBundleExecutable of Contents/Info.plist>`.

### 12.4 What proves "no cargo, no target, no checkout"

An assertion, not trust. Before the install, in the environment of 12.3
step 2, the script `tests/clean_mac.sh` runs and fails the test on the
first that does not hold:

```sh
# no cargo, no rustc, no rustup, no target: anywhere the programs look
for t in cargo rustc rustup; do ! command -v "$t" >/dev/null || fail "$t on PATH"; done
for d in "$H/.cargo" "$H/.rustup" "$H/Library/Caches/watane-dev" "$H/.watane" "$H/.local/bin/watane"; do
  [ ! -e "$d" ] || fail "$d exists"; done
[ -z "$(ls -d "$H"/.rustup/toolchains/*/lib/rustlib/wasm32-wasip2 2>/dev/null)" ] || fail "target exists"
# no checkout: nothing above the working folder is a watane checkout or any git work tree
# (`-e`, not `-d`: a git worktree's `.git` is a file)
d=$H; while [ "$d" != / ]; do
  [ ! -e "$d/Cargo.toml" ] && [ ! -e "$d/.git" ] || fail "checkout at $d"; d=$(dirname "$d"); done
# no variable that points at one
! env | grep -E '^(CARGO[A-Z_]*|RUSTUP[A-Z_]*|WATANE_[A-Z_]*)=' || fail "variable set"
```

After the install, before the answer `y`: the same loop finds no `cargo`;
after `y`, `$H/.cargo/bin/cargo` exists and `$H/.rustup` holds the target.
After the install, for each of the three programs and the app's
executable, `otool -L` lists only paths that start with `/usr/lib/` or
`/System/` (no `/opt/homebrew`, no `/usr/local`, no path under a checkout
or `$H`'s `target`): the binaries do not lean on anything of the build
machine. `watane-dev doctor --json` is run with none of `WATANE_BIN`,
`WATANE_DEV_VENDOR`, `WATANE_DEV_TOOLCHAIN` set, and must report
`watane` at `$H/.local/bin/watane` and the allowlist at
`$H/.local/share/watane-dev/vendor`, which shows that the programs found
what they need beside themselves.

### 12.5 Which test runs where

| Test | Where |
|---|---|
| INST-S-1 to INST-S-22, INST-S-26 to 35; the package test | `cargo test` of `watane-install`, anywhere (Linux, macOS, cloud) |
| the workflow's dry run and its smoke (12.3 `--fresh-home`, INST-S-1 and the 12.4 assertions) | GitHub `macos-14`, by `workflow_dispatch` |
| INST-S-23 to INST-S-25 and the full 12.4 | the clean Mac (a second user), against a real release |

`factory/check.sh` runs the first row for a task of `install`. The rows
that need a Mac are `#[ignore = "slow: ..."]` or a shell test and are run
by the human or the local foreman, as the `real_github` tests are.

## 13. Provides to, and needs from, other subsystems

| To | What |
|---|---|
| a person, J7 | `install.sh`, the archive, the release page (sections 3, 4) |
| `app` | nothing in code; stage 6 may show the one-liner |

| From | What |
|---|---|
| `host` | `watane --version` (exists); the program `watane` built with `--release --locked` |
| `dev` | `watane-dev --version` (exists); `scripts/vendor.sh [<out>]` and the `<exe>/../share/watane-dev/vendor` lookup (`dev 6.2`); `doctor --json` for 12.4; DEV-F-13 unchanged |
| `node` | `watane-node --version` (stage 2) |
| `app` | the buildable bundle: `cargo tauri build --bundles app` with the config override of 7.3; `Info.plist`; the same `version` in `tauri.conf.json` and `Cargo.toml` |
| `spec` | TC-7; `factory/check.sh` for the gate |

## 14. Questions

Each has the options, a recommendation, and the cost to undo each. The
answer used in this document is the recommendation, unless the human says
otherwise.

| # | Question | Options | Recommendation and used here | Cost to undo |
|---|---|---|---|---|
| Q1 | Does a macOS prompt appear for a `curl`-installed app? Facts hold that `curl` sets no quarantine mark, but the S50b author has not seen it on the macOS that the human runs, and Apple changes Gatekeeper between versions (macOS 15 removed the control-click bypass). | (a) Keep 6: no prompt expected; remove the mark anyway; print the text always. (b) Print the text only when `xattr -p` finds a mark. (c) Ask the person to open the app once while the script waits. | (a). INST-S-23 settles it on the clean Mac; if a prompt shows, the text is already there and the one-line `xattr` fixes it. | (a) to (b): remove a few lines of `install.sh`; a text change. |
| Q2 | Trust beyond TLS and a checksum from the same release. | (a) TLS plus `SHA256SUMS` (5). (b) Also publish GitHub build attestations and, for a person who has `gh`, document `gh attestation verify`; the script does not call it. (c) Sign `SHA256SUMS` with a watane release key (an ed25519 key like `log`'s) whose public half is in `install.sh`, verified in the script with `openssl`/`ssh-keygen -Y verify`. (d) `cargo-vet`-style reproducible builds. | (a) now, (b) when a person asks (a workflow line, no script change). (c) needs a tool that stock macOS may not have (LibreSSL's `openssl` lacks ed25519 verification on some versions; `ssh-keygen -Y verify` exists) and a key that someone must hold, which `watane` otherwise has none for releases; decide with the human before building. | (a) to (b): one workflow step. (a) to (c): a key, a release process, a script change, and every installed copy of the old script stays unsigned. (d): months. |
| Q3 | Intel Macs. | (a) Unsupported: exit 2 (TC-7: aarch64 first). (b) A second archive `x86_64-apple-darwin` built on `macos-13`, or a universal binary. | (a). A person on an Intel Mac gets a clear stop. | (a) to (b): a target in 4.1, a runner in 7.1, a case in step 4, a second clean machine for the test (cannot be tested on Apple silicon except under Rosetta). |
| Q4 | The oldest macOS. | (a) 14.0, the runner's own, tested. (b) 13 or 12, `MACOSX_DEPLOYMENT_TARGET` lower, untested. | (a). An older number is a claim nobody tests. | Change one number in `install.sh`, `release.yml`; a lower number needs an older test machine. |
| Q5 | Should the script put `~/.local/bin` on the `PATH` itself? | (a) Print the line only (3.10). (b) Ask y/N and append to `~/.zshrc`. (c) Append silently. | (a): no silent edit of a shell file (VISION), and the app and `watane-dev` work from the full path in the printed `Next:` lines. | (a) to (b): a question and an append; a later uninstall must then know the line it added (receipt gains `edit` entries). |
| Q6 | How `watane.app` is bundled. | (a) `cargo tauri build --bundles app` with a config override (7.3). (b) Assemble the bundle by hand in `package.sh`. (c) Set `bundle.active: true` in `tauri.conf.json`. | (a); it needs `tauri-cli` on the runner (an exact version, not cached, 7.3). Fall back to (b) if the override does not behave. (c) changes `app`'s config for every `cargo tauri dev` and is not needed. | (a) to (b): a day, `package.sh` only; the tree is unchanged. |
| Q7 | Where the version comes from. Crates each say `0.1.0` today. | (a) The tag, checked equal to five places (7.2 step 2). (b) A `[workspace.package] version` that every crate inherits. (c) Computed from the tag, written into the files by the workflow. | (a) now, (b) as its own small slice if five places hurt. | (a) to (b): every `Cargo.toml` of the workspace, hakari, the lock; mechanical. |
| Q8 | Where `install.sh` is served. | (a) A release asset with the version baked in (3.1). (b) `raw.githubusercontent.com/.../main/install/install.sh`. (c) A short URL on a domain that watane owns. | (a): pinned to a release, no moving `main`, no domain. The cost is a long URL, shown once in the README. | (a) to (c): a domain and a redirect; old URLs must keep working. |
| Q9 | The 26 MB vendor folder in the archive. | (a) In the archive (4.2): the first `watane-dev test` builds offline. (b) A second asset `watane-dev-vendor-<v>.tar.gz`, fetched by the script. (c) `watane-dev` fetches it on first use. | (a). One download, one checksum, no first-use network (F-QUAL-1, DEV-F-14). | (a) to (b): a second file, a second checksum, a script step. (c) puts network and a trust step into `dev`. |
| Q10 | **Question for the human.** Which machine proves J7 ("`watane-dev new` straight after") on a Mac with no command line tools? The allowlist needs a host linker: `dev/allowlist/Cargo.lock` holds `proc-macro2` (a build script), `serde_derive` and `wit-bindgen-rust-macro` (proc macros), which rustc links for macOS with `cc`, and `cc` without the tools is only Apple's stub that asks to install them (`wasm32-wasip2` itself links with `rust-lld`); `publish` and `new --from` also need `git`. The design prints `xcode-select --install` when `xcode-select -p` fails and never runs it (3.9, IN-30). A second macOS user shares the machine's tools and a fresh `HOME` does not remove them, so neither shows what happens without them; GitHub's `macos-14` runner has them. Not yet seen on a Mac (S50a user-sim, from the lock). | (a) Accept: S50b runs the clean-user run (12.3) and reports that J7's `watane-dev test` after the install is proved only with the tools present; the printed line is the remedy. (b) A VM (Tart, UTM) with a fresh macOS and no command line tools, snapshot per run: proves the whole of J7 including the line and the first build. (c) A second physical Mac, or a wiped one. (d) Remove the tools on the human's Mac (`sudo rm -rf /Library/Developer/CommandLineTools`): needs root, breaks the human's own `git` and `cc`, and Xcode may supply them anyway. | (b) once, as a one-time proof for S50b (a day), then (a) for later runs; if the human has no time for a VM, (a) and S50b says so in its report. (d) is not recommended. | (a) to (b): a VM image (15 to 20 GB, macOS licence terms for VMs) and one test script; (c): a machine; (d): reinstalling the tools. |
| Q11 | Telling a person that a new version exists. | (a) Never, from `install` (VISION). (b) The app shows a link to the release page when the person asks. (c) The app checks GitHub in the background. | (a); (b) is `app`'s decision later. (c) contradicts "no auto-update behind the person's back" unless the person turns it on, as `net` is. | (b) is new `app` work; (c) a switch and a consent text. |
| Q12 | How the home is deleted on uninstall. | (a) A typed `delete` on a real terminal, no flag, variable or file (8.4). (b) A flag `--delete-home` for scripts. (c) Never: the person deletes `~/.watane` by hand. | (a): the question is one deliberate act on a terminal, and a script or an agent without a terminal cannot answer it. It is not a protection against a program of the same user, which can delete `~/.watane` itself (spec/TECHNICAL 6, not protected); the identity is not recoverable without a backup (F-APP-6), which is why the act is slow. | (a) to (b): a flag; (c): remove the question. |
| Q13 | A Homebrew tap or a `.pkg` as well. | (a) No. (b) A tap later. | (a): the human chose the one-liner; a tap is another place a version lives. | (b) is an additive stage; nothing here changes. |
| Q14 | Linux. | (a) Out of scope (9). (b) A CLI-only archive. (c) Archive and app. | (a) now, (b) as stage 7. | (a) to (b): a target, a runner, a platform case, a Linux clean machine. |
| Q15 | Where the receipt and script copy live (`~/.local/share/watane/`). | (a) There (2). (b) Inside the app. (c) `~/.watane` (the home). | (a): the home holds the person's data and an uninstall must not need it. | Change the path in 3 places of this document and the script. |
| Q16 | Repository settings that the human sets before S50b's first release; nothing in the repository's files can set them. (1) Who can create a `v*` tag, since a tag push starts a release. (2) Who can run the workflow by hand and approve the publishing job. (3) Whether `main` is reviewed, since the workflow publishes only a tag on `main` (7.2 step 2). | (1) (a) a tag ruleset on `refs/tags/v*` (Settings, Rules): restrict creation, update and deletion to the maintainer role, no bypass but admin; (b) the legacy "protected tags" list; (c) nothing. (2) (a) a GitHub environment `release` (Settings, Environments) with required reviewers (the human) and deployment tags `v*` only, named by the `release` job (7.1); the dry run publishes nothing, so `workflow_dispatch` stays with whoever has write access; (b) restrict Actions to the repository's own and pinned actions, and the default `GITHUB_TOKEN` to read-only (Settings, Actions); (c) nothing. (3) (a) a branch ruleset on `main` that requires a pull request and blocks force pushes; (b) none. | (1) (a), (2) (a) with (b)'s read-only default token and "approve runs from outside collaborators", (3) (a). Without them anyone with write access can publish a release that `latest/download` serves; the checksums do not help, as they ride in the same release (5). S50b lists them in its report and does not cut the first release until the human says they are set. | Each is a setting that can be changed at any time; the cost of a missing one is a release that is already public: delete it and the tag (a published `latest` is then served by the previous release) and tell people. |
| Q17 | **For a later `dev` slice, not S50b.** Two fix texts of `watane-dev` suit a build machine and not an installed one: the toolchain fix `Install rustup with your package manager, then run: rustup default stable` (`env.toolchain`, `dev 3`; an installed person has no package manager to ask and the installer offers rustup), and the missing-vendor fix `'<abs dev>/scripts/vendor.sh' '<path>'` (`env.allowlist`), whose `<abs dev>` is the compile-time path of the build machine's checkout (`CARGO_MANIFEST_DIR`) and does not exist on an installed Mac, where the vendor folder is `~/.local/share/watane-dev/vendor` from the archive. The installer cannot change what `watane-dev` prints (INST-F-13). | (a) Leave as is: `watane-dev doctor` tells an installed person to run a script that is not there. (b) Change `dev`'s texts to point at the installer: the toolchain fix prints the rustup command of 3.7 (still a printed command, DEV-F-13 holds), the vendor fix says `Reinstall watane: curl -fsSL <one-liner> \| sh` when the executable lives in a folder with `../share/watane-dev` (an installed layout) and keeps `vendor.sh` for a checkout. (c) Have the installer set `WATANE_DEV_VENDOR`: it cannot, it edits no shell file. | (b), as its own `dev` slice after S50b, with INST-S-25's literal fix text changed with it (INST-S-25 names dev's text today). | (a) to (b): `dev/src` text in two places, `dev/TECHNICAL.md` 3 and 6.2, and the tests that check the fix strings; a day. |

## 15. Decisions of this subsystem

| # | Decision | Reason |
|---|---|---|
| IN-1 | One archive of all three programs, the app and the vendor folder for each platform, plus `SHA256SUMS` and `install.sh`, as assets of one GitHub release. | One download, one checksum, and the layout `vendor_dir` and `find_watane` need (2). |
| IN-2 | Per-user paths only: `~/.local/{bin,share}` and `~/Applications`. No root. | V5; INST-F-5. The human decided the two folders (2026-11-18). |
| IN-3 | `curl ... | sh` with the script as a release asset and the version baked in; the script is plain POSIX `sh` on stock macOS tools. | The human's decision; no dependency to install first (3.1, 3.3). |
| IN-4 | A receipt of absolute paths, written last; upgrade and uninstall touch exactly its paths. | The script never guesses what it owns (8.1). |
| IN-5 | The toolchain is offered, y/N, default N, only on an answer typed on the terminal (`/dev/tty`); with no terminal the answer is N and the commands are printed. | The human's decision (2026-11-18); DEV-F-13; F-MAKE-5. |
| IN-6 | rustup is installed with `--profile minimal --no-modify-path -t wasm32-wasip2`. | Smallest that `watane-dev` needs; no shell file edit; `watane-dev` finds rustup in `~/.cargo/bin` (`dev 6.1`). |
| IN-7 | The programs and the app are ad-hoc signed (`codesign --sign -`), and the script removes the quarantine mark and prints the Gatekeeper text. | Apple silicon needs a signature to run; there is no Apple account; the human's "few, calm steps" (6). |
| IN-8 | The checksum is the only integrity check; its limits are stated (5, 10); no signing key in this version. | No release key exists, and the stock tools cannot verify one (Q2). |
| IN-9 | Linux is out of scope for this version. | 9. |
| IN-10 | The script edits no shell file; it prints the line (3.10). | VISION: nothing behind the person's back (Q5). |
| IN-11 | The workflow runs on a tag and by hand; a release is a draft until all files are uploaded; a hand run is a dry run. | `latest/download` must never serve a half release (7). |
| IN-12 | `bundle.active` stays `false`; the workflow asks for the bundle with `--config`. | No change to `app` for a release's sake (7.3, Q6). |
| IN-13 | The version is the tag, and the workflow checks it equals the crates' and `tauri.conf.json`'s. | A release whose programs print a different number is a bug (7.2, Q7). |
| IN-14 | A crate `watane-install`, tests only. | The tests must be in `cargo test` and the gate (1). |
| IN-15 | Tests fake the platform with fake tools first on `PATH`, and the network with `WATANE_RELEASE_BASE`; every answer (the offer and the uninstall question) is typed on a pseudo-terminal, and a test with no terminal runs in a new session. | The real script runs unchanged, with no test-only branch other than the two variables `WATANE_RELEASE_BASE` and `WATANE_RUSTUP_URL` (3.2, 12.1). |
| IN-16 | "Clean Mac" is a second macOS user plus an asserted `env -i` environment (12.3, 12.4); a VM is the next step. | Cheapest real GUI session and real Gatekeeper; the assertion, not the account, is the proof. |
| IN-17 | The home is deleted on uninstall only by the typed word `delete` on a real terminal; no flag, variable or file answers it. | A deliberate act, since an identity cannot be restored without a backup (F-APP-6). It does not stop a same-user program from deleting the home (spec/TECHNICAL 6). |
| IN-18 | Nothing checks for a new version and nothing runs after the script ends. | VISION; INST-F-23. |
| IN-19 | The oldest macOS is 14.0 and the platform is Apple silicon only. | What the runner builds and tests (Q3, Q4). |
| IN-20 | The one-liner is the `latest` URL; a pinned URL exists for a person or an agent who wants one version. | 3.1. |
| IN-21 | The version is checked against a fixed pattern before it reaches a URL, a file name or the receipt. | curl collapses `..` in a URL path; a version must not choose the repository (3.2; S50a adversary). |
| IN-22 | Every removal checks the path against the rule of 8.5, not against a prefix; the home question is never asked for `/`, `$HOME` or a folder above it. | The receipt and `WATANE_HOME` are inputs, not proofs (8.4, 8.5; S50a adversary). |
| IN-23 | The release workflow builds with a read-only token, no persisted credentials and no cache, publishes from a second job that runs no repository code, only for a tag on `main`; a hand run has no input that publishes. | A hostile build dependency must not get a token that writes to the repository (7.1, 7.2; S50a adversary). |
| IN-24 | There is no `--toolchain` flag, and no variable or file says yes. With no terminal the script installs no toolchain and prints the commands. | The human decided the offer is y/N and never silent; an agent that wants the toolchain runs the printed command itself (F-GET-3, 3.7). `--no-toolchain` stays, to skip the question. |
| IN-25 | The script has no test seam for answers: the toolchain offer and the uninstall question read `/dev/tty` alone. Tests type answers on a pseudo-terminal (`script(1)`, 12.1) and run a no-terminal case in a new session. | A file or variable that answers the offer lets an agent with no terminal install rustup, against F-GET-3 and "never silent"; one that answers the deletion question would be worse. Replaces the earlier seam `WATANE_INSTALL_TTY`, which no longer exists. |
| IN-26 | A first install, or an upgrade onto a path that no receipt lists, stops with exit 8 if the path exists, and changes nothing; there is no flag that overwrites. | INST-F-19: the script replaces only what it installed (3.4.2). |
| IN-27 | `~/.local/bin`, `~/.local/share` and `~/Applications` must resolve, with links followed, inside the home; else exit 2. | INST-F-5's intent: write only under the person's home, even if a link would lead elsewhere (3.4.1). |
| IN-28 | S50b does not cut the first release before the human has set the repository settings of Q16 (tag ruleset for `v*`, environment `release` with a required reviewer, read-only default token, `main` reviewed). | The workflow's own rules (7) assume them; the repository, not the repository's files, enforces who may start a release. |
| IN-29 | The receipt is the last entry of one swap and its move is the commit point; before it the old version and receipt can be put back whole (a journal and `.old.<pid>` names, undone on failure, on `HUP`/`INT`/`TERM` and by the next run after a `SIGKILL`); what only the old receipt held and the `.old` names are removed after it. | A rollback's exit 5 must be true, and a re-run must not meet its own half-swapped files as foreign (exit 8) (3.6, INST-S-14, 32, 33). |
| IN-30 | The script checks `xcode-select -p` and, when the command line tools are missing, prints `xcode-select --install` with one line of why; it never runs it and prints nothing when the tools are present. | The maker tools need `cc` and `git`; macOS opens its own dialog, and the install is the person's to start (3.9, INST-S-34; Q10 asks the human how J7 is proved without them). |
| IN-31 | The app is built with `cargo tauri build ... -- --locked`, and INST-S-29 checks the workflow line. | A release must build the app from the reviewed lock, as the programs are (7.3). |
| IN-32 | Dev's fix texts for the toolchain and the vendor folder are not changed in this design; a later `dev` slice points them at the installer (Q17). | `install` changes no code of another subsystem and INST-F-13 keeps `watane-dev` as it is. |
| IN-33 | A live journal of another pid stops the run (exit 5); the message names the journal file to remove when that pid was reused by an unrelated program. The script never removes it itself, since it cannot tell a reused pid from a running install. | Undo: drop the line; or later add a lock with the start time. |
