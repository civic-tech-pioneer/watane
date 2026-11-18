# install: vision

## Why

Everything of watane is built and proved, but no person can get it. Today
a machine gets `watane`, `watane-dev`, `watane-node` and the app only from
a checkout and `cargo build`, which needs Rust, a Wasm target and the
whole repository. Alice, who has a Mac and no Rust, cannot start. A
coding agent that a maker asked to "install watane" has no command to
run.

`install` is the way watane reaches a machine: one command, from GitHub
Releases, that puts the programs and the app in the person's own folders,
says what to do next, and offers the one thing that the maker tools need
and that watane does not ship, the Rust toolchain. It is a script, a
release workflow and the documents that fix them. It runs once; it is not
part of watane at run time.

## For whom

| Person | What they need |
|---|---|
| Alice, on a Mac with no Rust, no checkout and maybe no idea what a toolchain is | One line to paste, plain words, a working app, and a calm answer when macOS asks whether she trusts it. |
| A maker's coding agent, in a terminal, with no person at the keyboard | The same line, output it can read, no question that blocks it, and every step that it cannot do (the toolchain) printed as a command. |
| Bob, who only uses mods | The app alone is enough; he may answer no to the toolchain and never see it again. |

## Goals served

| Goal | How `install` serves it |
|---|---|
| V1 Own environment | The programs and the app land in the person's own folders, as the person's own user, and the person can see and remove every file. |
| V4 No center | The only server is the release page of the watane repository, which anyone can mirror (`WATANE_RELEASE_BASE`); there is no account, no installer service and no update server. |
| V5 Safe by default | Nothing is written before the download is checked against the checksums of the same release; no root; nothing runs that the person did not ask for, and the toolchain only after a yes. |
| First-version success | Success 1 starts with "a person asks their coding agent for a mod": this is what puts the maker tools on the machine. |

## Boundaries: what `install` must never become

- **Never a silent toolchain install.** Rust and its Wasm target install
  only after the person said yes to a question that names what and where.
  With no terminal to ask on, the answer is no.
- **Never root.** No `sudo`, no `/usr/local`, no `/Applications`, no
  system folder, no password prompt. The script refuses to run as root.
- **Never an account.** No sign-in, no key, no token to download or run.
- **Never telemetry.** No count, no ping, no identifier. The script
  contacts GitHub for the release files, and `sh.rustup.rs` only after a
  yes.
- **Never an updater.** Nothing runs in the background and nothing checks
  for a new version. A person upgrades by running the line again.
- **Never the person's data.** Upgrade and uninstall leave the watane home
  (identity, mods, channels) alone. Removing it is a separate, typed
  question.
- **Never a second source of truth.** What the programs and the app are
  stays in `host`, `dev`, `node` and `app`; `install` only packs them.

## Success

- On a Mac with no cargo, no Wasm target and no checkout, one pasted line
  ends with `watane`, `watane-dev`, `watane-node` and `watane.app` in
  place, a clear next step, and no file outside the person's home.
- After a yes to the toolchain question, `watane-dev new` and
  `watane-dev doctor` work with no further setup.
- A download that does not match its checksum installs nothing.
- Running the line again upgrades; one command uninstalls and keeps the
  person's home.
- A reviewer rebuilds the whole from `TECHNICAL.md`, and the proof that it
  worked on a clean Mac is an assertion, not a hope.
