# S84 — Installed hook

`Confirm::installed` receives one report after `Host::install` commits the
catalog and swaps the registry. The report keeps the requester and lists
new and updated local names in install order. Unchanged entries do not
appear. An unchanged-only plan, a denial, or a failed install sends no report.
The callback runs synchronously on the host thread and must not block.
Its default body does nothing, so earlier Confirm implementations still work.

The tests in `tests/api.rs` cover the public type and default method,
the W37 local API path, app and manager updates, unchanged entries,
missing providers, stale plans, and rollback when the catalog write fails.
The recording callback checks the durable catalog and current registry.

No dependency or spec change was needed. The existing section 12.1,
stage 12, and W37 test-plan row already describe this interface.

Validation uses the shared `$CARGO_TARGET_DIR`. The fixture build passed;
seven rebuilt collect fixtures differed in bytes, so their committed
artifacts were restored. The default host suite passed (181 tests, four
ignored timing tests). Build, clippy with warnings denied, and fmt passed.
The serial run with `--include-ignored` passed all 185 tests.
