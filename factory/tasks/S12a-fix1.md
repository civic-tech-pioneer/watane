Subsystem: host
Effort: medium
Depends on: none
Slice: S12a — J2: Alice runs her mods together in the app (fix found at delivery).

## Goal
Closing a host releases its home at once. Today `catalog::participant::ac10_s12a_disable_enable_and_restart`
(host/tests/catalog.rs, in the `preview` test binary) failed once in 16 full runs, at the
`Host::open(..).unwrap()` right after `drop(host)` (line ~546): something still held the home,
most likely a clone of `Arc<Host>` that outlives the caller's own handle (a `Chain` of an event
delivery, a `call_async` thread, a server connection or `spawn_blocking` task after
`Served::shutdown().await`). Find the holder and make the release deterministic. The test must
not be loosened (no retry, no sleep around `Host::open`).

## Acceptance
- AC1 A red-first test reproduces the race: repeat the sequence of ac10 (install `shelf`, `recs`
  in a home with a `Confirm` fake; `items.add` through `call_async`; serve, a `/call` and a page
  `GET`, `served.shutdown().await`; `drop(host)`; `Host::open` at once) enough times (for example
  200, in a loop, kept under 10 s) that it fails on the current code, at least on a loaded
  machine; say in your report how often it failed before the fix. If you cannot make it fail,
  report the holder you found by reading the code and the test that shows it is gone (for
  example: after `call_async` returns and after `shutdown().await`, `Arc::strong_count` of the
  host is the caller's count, in a crate unit test).
- AC2 After the fix: `call_async` returns only after its thread dropped every host clone it
  made (including event deliveries the call waits for); `Served::shutdown().await` returns only
  after its tasks dropped theirs. ac10 stays unchanged and passes.
- AC3 `cargo test -p watane-host` (all binaries, 3 runs in a row), clippy `--all-targets -D
  warnings`, `cargo fmt --check` pass; `cargo build -p watane-dev` passes.

## Spec
host `TECHNICAL.md` 2.1 (`open`: `busy` for a held home; `call_async`; `Served::shutdown`), 3.1
(the home lock), 12.2 (shutdown).

## Interface
No public API change.

## Files
`host/src/**`, `host/tests/catalog.rs` (a new test only). Not `dev/`, not `app/`.

## Out of scope
Anything else in host.
