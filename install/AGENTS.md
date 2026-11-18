# install: rules for the builder of `install.sh`, `package.sh` and the release workflow

## Read first, in this order

1. `../spec/VISION.md`, `../spec/FUNCTIONAL.md` (J7, F-QUAL-2),
   `../spec/TECHNICAL.md` (1: TC-7; 6: the Install row).
2. `VISION.md`, `FUNCTIONAL.md`, `TECHNICAL.md` here. TECHNICAL.md is the contract.
3. `../dev/TECHNICAL.md` 6 (the toolchain and the vendor folder) and
   `../dev/FUNCTIONAL.md` DEV-F-13; `../app/TECHNICAL.md` 12 (the bundle).

## Commands

`cargo test -p watane-install` (no network, no Mac needed);
`cargo clippy --all-targets -- -D warnings && cargo +nightly fmt --check`;
`shellcheck -s sh install.sh package.sh` when it is installed.
The clean-Mac tests (`tests/clean_mac.sh`) run on a Mac, by hand or by the
local foreman.

## Rules that matter most here

1. `install.sh` is POSIX `sh` and uses only the tools of TECHNICAL.md 3.3.
   No bash-only syntax. Nothing is written before the checksum matched.
2. No root, no `sudo`, no write outside `$HOME`, no telemetry, no
   background process, no shell file edited, no toolchain without a typed
   yes (no flag says yes). Each is a vision boundary; ask the human before
   weakening one.
3. Names are defined in TECHNICAL.md only (archive, paths, flags,
   variables, exit codes, texts). Change one there and in every document
   that cites it, then in the script and the tests.
4. The script has no test-only branch except `WATANE_RELEASE_BASE`,
   `WATANE_RUSTUP_URL` and `WATANE_INSTALL_TTY` (TECHNICAL.md 3.2; the last answers the toolchain offer only, never the uninstall question). A test
   fakes the platform with tools first on `PATH`.
5. A test never reaches the internet. A test of the real release is the
   clean-Mac run, not `cargo test`.
6. A clean machine is proved by the assertions of TECHNICAL.md 12.4, not
   by the account it runs in.
7. Build the stages of TECHNICAL.md 11 in order.

## Reading the specs

The specs were drafted ahead of any code and are not validated. Their
detail exists to surface integration problems early, not to be obeyed
word for word. Follow their intent, and do not ask whether to.

- Priority: `../spec/` vision, functional, technical; then this folder's
  vision, functional, technical. The whole wins over this subsystem.
- When a spec is wrong, unclear or in the way, change it yourself: edit the
  document that owns the fact, fix each document that cites it, and keep
  it true to the documents above it.
- Ask only to go against a vision document or to weaken a security check.

## When blocked by a missing sibling

- Stage 2 needs `watane-node --version` from `node`; the bundle needs
  `app` to build with `cargo tauri build`. Never edit a sibling's code
  beyond what the stage lists: record what is missing in TECHNICAL.md 13.
