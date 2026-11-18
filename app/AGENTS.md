# app: notes for the builder

The Tauri 2 desktop app: `host` and `dist` of one home in one process, one
window per mod page, and the consent view for every confirmation.

## Read in this order

1. `../spec/VISION.md`, `FUNCTIONAL.md`, `TECHNICAL.md`.
2. This folder: `VISION.md`, `FUNCTIONAL.md`, `TECHNICAL.md`.
3. `../host/` and `../dist/` specs: the real signatures behind section 15.
4. Reference only: `~/Documents/local-projects/colos/explore/{app-driver-macos,webview-origin-isolation}/`.

## Commands

```bash
cargo build && cargo test                   # app, unit and static tests
cargo test --features driver --test it -- --include-ignored <file>::   # a task's window tests: only the files it touched
cargo test --features driver                # every window test (macOS GUI session): once per slice, by the foreman
cargo clippy --all-targets --features driver -- -D warnings && cargo fmt --check
cargo build --release --features driver     # must FAIL: no driver in a release
```

## Rules that matter most here

- No rule of installing, running, signing or verifying lives here. Call
  host or dist, show the result, and never hide a cause.
- Only `consent_answer` and the consent window's `Destroyed` handler call
  `Broker::answer`. No HTTP route does.
- No `remote` key, no `core:` or plugin permission, no window other than
  `main` and `consent` in a capability. No `devUrl`.
- A new command goes into `build.rs`, a capability and the literal lists
  of `tests/it/static_config.rs`. Untrusted text only via `textContent`.
- Window tests wait for page-load events, never a fixed time. Never weaken
  a test; record a changed fact in `NOTES.md`.

## Reading the specs

The specs were drafted from earlier experiments and are not validated.
Their detail exists to surface integration problems early, not to be obeyed
word for word. Follow their intent, and do not ask whether to.

- Priority: `../spec/` vision, functional, technical; then this folder's
  vision, functional, technical. The whole wins over this subsystem.
- When a spec is wrong, unclear or in the way, change it yourself: edit the
  document that owns the fact (any folder's spec, never another folder's
  code), fix each document that cites it, and keep it true to the documents
  above it.
- Ask only to go against a vision document or to weaken a security check.

## When a sibling is missing

Follow TECHNICAL.md section 12: stage A needs no sibling (`src/fake.rs`,
the driver's `ask`). If a sibling's API differs from section 15, adapt only
`src/core.rs` and `src/confirm.rs`. If an item is missing, stub it there
with code `unavailable` and record it in `NOTES.md`; never build it here.
