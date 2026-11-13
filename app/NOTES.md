# Implementation notes

- S10-4 (app): `gui_isolation` covers S5 to S8 against the fake hostile catalog, the fixture server and an independent listener. S5 checks `.visible_on_all_workspaces(true)` by searching the consent builder in `views.rs`, because `windows` has no all-Spaces field.
- S10-4 validator (app, host): on macOS 27 ATS refused every `http://<local>.localhost` page load (`http_security ... Operation not permitted` in the WebKit networking log): `NSAllowsLocalNetworking` does not cover `*.localhost`, so no mod page loaded and all four isolation tests failed at `open`. `Info.plist` now adds an `NSExceptionDomains` entry `localhost` with subdomains and insecure HTTP only; `static_config` checks it and the absence of `NSAllowsArbitraryLoads`. Spec changes: app TECHNICAL 12 (bundle note), host TECHNICAL Q10 (verified for WKWebView). The page loads over `::1`. Tauri embeds `Info.plist` at compile time without a rebuild trigger: after editing it, touch `src/lib.rs` or clean the crate.
- S10-4 download observation (app): WebKit reaches the native download hook for the `data:` attempt (`blocked` gets a `download` entry for `mod-hostile`), not for the `blob:` attempt; both are among the 109 refused attacks.
- S10-4 validator (app): with the fix, `cargo test --features driver` passes all 30 tests in about 20 s; no test needs `slow`. Mutations in `views.rs` (navigation allowed, new window allowed, no `incognito`, `close_mod` a no-op) each fail the matching isolation test.
- S10-4 support export (app): the `pages` module declaration also needs a scoped `allow(dead_code)` attribute because `gui_shell` includes `support` but does not use its fixture servers; otherwise the required clippy command fails. No dependency added.
- S10-4 checks (app): `cargo build`, `cargo test` (19 non-GUI tests), `cargo clippy --all-targets --features driver -- -D warnings`, and `cargo fmt --check` pass. `cargo build --release --features driver` fails at the required driver-only-in-debug compile guard.
- S10-3 spec gap (app): `mod_disable` is stage A, as required by the S10 interface; Core closes its window before changing the fake catalog's enabled flag (TECHNICAL 7).
- S10-3 uses only the task-authorized new dependencies: tauri =2.12.5 for native windows and IPC, and tauri-build =2.7.4 for the command ACL manifest. No host, dist, server, or dialog plugin in stage A.
- S10-3 validation environment (app): GUI startup in the sandbox reports `Connection Invalid error for service com.apple.hiservices-xpcservice` and never reaches the main page-load ready event; window tests remain enabled and unchanged for a logged-in macOS validator.
- S10-3 checks (app): build, 14 non-GUI tests, clippy with driver and warnings denied, fmt, and JavaScript module syntax pass; release with driver fails with the required compile_error message. The feature-enabled suite compiles; its seven GUI tests time out at startup in this sandbox.
- S10-3 validator (app): on a logged-in macOS 27 session the seven window tests pass. A window gets focus only while macOS lets the test-spawned app activate (cooperative activation): with another app in front, `document.hasFocus()` stays false and the focus checks of AC2 to AC7 time out. Run the window tests with the terminal in front. On the main thread, `run_on_main_thread` runs its closure at once, so the consent `Destroyed` handler answers the broker from another thread: the successor window then opens after the old one has released the label.

- S14 spec update (app): TECHNICAL 10 and 11 define driver screen modes, the actual AppKit policy diagnostic, constructors, and the ignored front-test rule. No factory or sibling code changed; no dependencies added.
- S14 changed test fact (app): a hidden/background consent document has no real focus, so `ui/consent.js` intentionally leaves Approve disabled even when its timer fires. Content/withhold coverage now enables the button only through native driver eval; the unchanged real focus/800 ms gate assertions live in ignored front tests. The shared consent helper checks readiness/topmost configuration; its focus assertions moved to `ac3_consent_takes_focus`. The reopen/focus loop moved to `ac3_reopening_an_open_mod_focuses_it`; hidden ac2 retains the single-window/reopen check.
- S14 initial hidden WKWebView check (app): `gui_shell::ac1_hidden_screen_loads_evaluates_and_runs_timers` timed out at startup before the main page-load ready event in this sandbox. Hidden page loading, native eval and 800 ms scheduling cannot be established here; the enabled probe and the hostile isolation tests remain for the logged-in macOS validator. No test has been moved to background for a demonstrated hidden WKWebView scheduling failure; only `ac2_background_screen_is_visible_without_focus` explicitly probes background mode.
- S14 checks (app): `cargo build -p watane-app`, final `cargo test -p watane-app` (22 non-GUI tests), `cargo test -p watane-app --features driver --no-run`, clippy with all targets/driver and warnings denied, fmt, and the normal release build pass. `strings` finds no `WATANE_DRIVER_SCREEN` in `target/release/watane-app`; release with driver fails at the intended compile guard.
- S14 GUI validation limit (app): the one full default driver run (`--no-fail-fast`, no ignored tests) compiled everything, ignored all four front tests, and timed out all 12 quiet GUI tests before readiness, with `Connection Invalid error for service com.apple.hiservices-xpcservice`. No front test was run. The logged-in macOS validator must establish visibility, focus, actual Accessory policy, hidden page/eval/timer behavior and hostile polling; no hidden scheduling fallback can be justified from this startup failure.
- S14 existing support-test observation (app): the unchanged `ac3_listener_counts_connections_and_stops_on_drop` intermittently failed its post-drop connection-refused assertion in parallel runs (including the full driver run), then passed in the final normal suite. Its assertions and listener implementation remain unchanged; the driver support suite also passes with `--test-threads=1`.
- S14 validator (app, logged-in macOS 27): the default driver run passes all 12 quiet GUI tests (4 front tests ignored, not run). Hidden WKWebView windows finish page loads, answer native `eval`, and fire the 800 ms timer probe in range; the hostile poll-loop isolation tests pass hidden. No test needs `App::background` for a scheduling reason. The frontmost app did not change during the run. The release binary has no `WATANE_DRIVER_SCREEN`. The support self-test post-drop connection-refused race showed up once more, this time in `ac1_pages_serve_each_origin_on_both_loopbacks`, during the full run; it passed in 5 isolated reruns.

- S12a-2 (app): stage B replaces the fake catalog and fixture page origins in every window test with the real home host; `ready.port` is now the bound nonzero listener port. `gui_shell::ac1` checks `/api/status` and “No mods yet”.
- S12a-2 changed test facts (app): the hostile fixture is a host mod (`ui/`, `wit/`, `mod.toml`), with listener configuration copied into `ui/config.js` before install. The script harness loads two external scripts instead of one and gets its listener from config instead of a query string; all 109 attack oracles remain. The mods-tree comparison now checks every file byte-for-byte after installation. Shell catalog tests install actual page-only/books and prebuilt logic fixtures; the logic-only local name is `reading-list`.
- S12a-2 spec gaps (app): TECHNICAL 11/12 require committed fixture components and manual rebuilds; FUNCTIONAL S9 gives back a declared grant directly in main without consent, matching TECHNICAL 7; TECHNICAL 5.1 preserves consent focus when reopening a mod.
- S12a-2 dependencies (app): adds the required `watane-host` path dependency and `tauri-plugin-dialog` for the Rust folder picker specified in section 7; moves the existing `axum` dependency to runtime for the empty extra API router. No direct pkg dependency.
- S12a-2 polling (app): `App::background` does not make a page visible: on a logged-in macOS 27 session an unfocused window behind other apps is occluded and WebKit reports `document.hidden === true`, so the `!document.hidden` poll gate (TECHNICAL 7, "while visible") stopped all polls and the 5 s oracle saw none (old result: 0 extra `mods_list`). Hidden-window timers fire (S14). The test now runs hidden and declares the main page visible (`document.hidden` getter returns false); with the 2 s interval removed it fails.
- S12a-2 host integration (app, host): host stage B `grant`/`revoke` changes authority but emits no page notification. Core reloads an existing mod page after either operation so giving a grant back is visible immediately, without a consent request or a second click.
- S12a-2 spec gap (app): TECHNICAL 9.2 adds a weak host reference to HostConfirm, initialized after `Host::open`, so `uses` can resolve installed provider local names without a reference cycle or a second provider catalog.
- S12a-2 support race (app): the previously recorded `ac3_listener_counts_connections_and_stops_on_drop` failure recurred in a parallel default run. The four support tests that bind ephemeral ports now share a mutex, preventing another test from reusing the just-closed port before the connection-refused oracle. Assertions are unchanged.
- S12a-2 CLI defaults (app, validator per foreman): `Config::from_args` follows TECHNICAL 1: `--home` else `watane_host::default_home()`, `--port` else `WATANE_PORT` else `DEFAULT_PORT`; 0 is any free port. Stage A's `--home` required / port 0 default is gone; the tests always pass `--home` and `--port 0`. `static_config::ac1_home_and_port_default_and_port_is_validated` replaces `ac1_home_is_required_and_port_is_validated`.
- S12a-2 final checks (app): `cargo build -p watane-app`, `cargo test -p watane-app` (28 non-GUI tests), driver test compilation, clippy all-targets/driver with warnings denied, `cargo fmt --check`, and JavaScript syntax checks pass. Release with driver fails at the intended `compile_error!`. Window tests are compiled, not executed in the sandbox.

- S18 spec update (app): TECHNICAL 10 records the Linux driver popup setting; TECHNICAL 11 documents `scripts/linux-test.sh`, its named registry/target volumes and front-test exclusions; TECHNICAL 12 stage F uses arm64 Linux in Docker (deputy decision), with x86_64 only if arm64 hides a gap. No sibling code or interface changed.
- S18 platform assertion (app): hidden/background `screen.accessory` is AppKit’s actual Accessory policy on macOS and false on Linux. Both GUI probes now assert the platform value explicitly; the macOS policy assertion, visibility, focus, page-load, eval and timer oracles remain enabled.
- S18 WebKitGTK popup observation (app): the first Linux isolation run refused all 109 attacks but failed `ac2_hostile_attacks_are_refused_without_side_effects` at “missing new_window refusal”; only navigation hooks fired. WebKitGTK defaults [`javascript-can-open-windows-automatically` to false](https://webkitgtk.org/reference/webkit2gtk/2.42.4/property.Settings.javascript-can-open-windows-automatically.html). Linux driver builds now enable that engine setting for mod views so the unchanged native `NewWindowResponse::Deny` handler is exercised. Production settings and every isolation oracle are unchanged. Adds only Linux-only `webkit2gtk =2.0.2` (already transitive through Tauri) to access that native setting in driver builds.
- S18 build resources (app): an initial debug-symbol/incremental Linux target grew to 10.3 GiB and filled the existing 58.4 GiB Docker filesystem (`No space left on device`). The image now sets dev/test debug info to 0 and disables incremental compilation, with two build jobs; dependencies still persist in the named target volume. A fresh default suite used 2.4 GiB. Only this task’s Linux target cache was cleared for the profile change; the macOS target and other Docker data were untouched.
- S18 GTK focus observation (app): under bare Xvfb the background probe returned `visible: true, focused: true` despite `.focused(false)`, failing its unchanged focus oracle. Adding `focusable(false)` also failed and was removed. The runner now starts Openbox inside Xvfb and waits for its `_NET_SUPPORTING_WM_CHECK` property; the window manager honors the existing focus hint. No constructor, test mode, or focus oracle changed.
- S18 Linux findings (app): Debian trixie arm64, Rust 1.95.0, WebKitGTK API 4.1/library 2.54.0, GLib 2.84.4 and libsoup 3.6.5 (`pkg-config --modversion glib-2.0 libsoup-3.0 webkit2gtk-4.1`). Hidden windows finish page loads, answer native eval and fire the 800 ms timer within the existing 750–2000 ms oracle; hidden hostile polling passes. No test needs background as a scheduling workaround.
- S18 localhost evidence (app): WebKitGTK resolves `http://hostile.localhost:<port>` in all four isolation tests and `http://reading-list.localhost:<port>` / `http://recommendations.localhost:<port>` throughout J2, including after restart. No hosts-file or resolver workaround was needed. The GUI tests use the real dual-loopback host without forcing an address family; `support_selftest::ac1_pages_serve_each_origin_on_both_loopbacks` separately verifies fixture responses on both `127.0.0.1` and `::1` (using explicit addresses and Host headers, not a claim about WebKit’s family preference).
- S18 Linux checks (app): `scripts/linux-test.sh` exits 0: 31 default tests, then 48 driver tests including all 12 quiet window tests and `ac5_journey_j2`; four front tests compile and are filtered out by name. A repeat from the same named volumes rebuilds no crates (Cargo finishes the two build phases in 0.50 s / 0.17 s). The sandbox required `BUILDX_CONFIG=/tmp/s18-buildx` to avoid writing Docker buildx activity outside its writable roots; the daemon itself was reachable.
- S18 macOS checks (app): build, 31 default tests, driver `--no-run`, clippy all-targets/driver with warnings denied, and fmt pass; release with driver fails at the required compile guard. The attempted macOS driver suite hits the existing sandbox `com.apple.hiservices-xpcservice` connection failure before the first page load (all four quiet isolation tests time out); a logged-in macOS validator is still needed for GUI execution. Front tests were not run on either platform.

## S21b-3 — stage C

- Added `watane-dist` (the task-authorized sibling dependency). `tower-http` with `fs` was already present. No other dependency added.
- AC1–AC4 first failed to compile for the absent DistConfirm, channel mapping, provider loop and dist error conversion. AC5 then failed on the missing main capability commands. AC6's route test first failed on the absent api module. These unit/static/route tests passed after implementation.
- The parties cache in `src/words.rs` now supports concurrent reads and replacement from verified channel views. Its existing before-stage-C test remains unchanged in meaning: PublicKey's fingerprint is still its first ten text characters. No existing window test asserted an exact app Status JSON; none needed an assertion change.
- The API router takes only `Option<Arc<Dist>>`, avoiding a server/router/Core ownership cycle and allowing the unavailable response to be tested without a window. Failed startup still serves nothing.
- `tests/api.rs` covers the complete publish response table without a GUI and adds a driver test through the consent window and host HTTP guards. `tests/j3.rs` is ignored as `slow: journey J3`; both driver tests are compiled for the validator, not run here (the task reserves GUI execution for it). J3 tests refusal of replacing a published identity with another backed-up identity; dist/log treat restoring the very same identity as idempotent.
- Validation: `cargo build` passed; `cargo test` passed (38 tests); `cargo clippy --all-targets --features driver -- -D warnings` and `cargo fmt --check` passed. `cargo build --release --features driver` failed specifically at the existing `compile_error!("the driver feature is only for debug test builds")`. No GUI test was executed.

## S21b-fix1 — two intermittent window tests

- `api::ac6`: the failing call was not the deny click but the next request's `consent()` ready check. After deny the answered window closes asynchronously and keeps the label `consent` until Destroyed, so `wait("consent")` and its evals reached the closing window; the dropped eval callback surfaced as `channel is empty and sending half is closed`. The test now uses `journey::deny`, which waits for the window to go, as `approve` already did. No assertion changed.
- `j2::ac8`: an app bug. `refresh()` returned an in-flight refresh to `mod_remove`'s caller; that one could have read `mods_list` before the remove and rendered the stale list, and hidden driver windows (`document.hidden` is true) never poll to repair it. A refresh asked for during another now runs once more after it. S21b-3 widened the window (`channelsView` before `mods_list`); the coalescing predates it, but ac8 passed 10 of 10 on `main` (e8c0f02).


## S25 — updates and withdrawals without an index

- App-only implementation; no new dependencies and no host, dist, dev or api.rs changes. Shared fetched-release planning serves install_link and release_install; refresh callers spawn independent installs, leaving consent to host. Update failures are kept by local name and cleared by successful app installs. Mods expose notice/offer; identity exposes releases and withdrawal controls.
- Red/green: AC1–AC3 first failed on absent rule functions. The additional AC3 lower-release regression failed with an unwanted 0.3.0 offer before the predicate required a newly reported higher version. The parsing and identity-row tests failed on absent helpers before implementation. Unit tests also cover withdrawal error causes and nullable ModView fields.
- J4 reuses the committed reading-list component in temporary copies. WIT package versions and equal-length component interface names change before pack; 0.3.0 declares an additional HTTP grant. The fixture self-test packs all three versions without compiling a component. The journey covers preserved books after upgrade and downgrade, refresh returning during consent, decline/failure retention, retry clearing the notice, withdrawal signing, the offer button, and restored-identity equivocation.
- Changed fact (app, unlisted file): tests/gui_isolation.rs must expect 111 hostile attacks instead of 109 because the fixture now invokes release_install and release_withdraw too. The support harness also expects 96 invoke calls instead of 94. All original attacks and refusal/side-effect assertions remain.
- Specs (app): TECHNICAL 7/9.5/11/12 documents the command/view contracts, update selection and task lifecycle, J4, and the built part of stage D; APP-F-38 still waits on the index design. FUNCTIONAL S16 records the story; AGENTS records the settled gap.
- Validation: cargo build passed; cargo test passed 47 tests on macOS; clippy --all-targets --features driver with warnings denied, fmt --check, JavaScript syntax checks and git diff --check passed. cargo build --release --features driver failed at the intended debug-only driver compile_error.
- macOS GUI limitation: both J4 and the default driver suite were attempted; apps timed out before driver readiness (the default suite stopped at api::ac6_publish_through_consent_window_and_host_guards). No window assertion was weakened. In the existing arm64 Linux test image with Xvfb/Openbox, cargo test --features driver passed all 64 default tests, including all 111 hostile attacks; the separately enabled ignored J4 passed in 9.86 s. Front tests remained ignored.

## S27 — Explain startup, updates and withdrawals

- Spec gap (app): TECHNICAL 3/7/9.5/13 and FUNCTIONAL APP-F-2/19/22/39/40 now name busy-port recovery, own withdrawal wording, dist reason mapping, consent headings, declined updates, offerGrants and the visible installed version. No sibling contract changes.
- AC8 intentional assertions: gui_shell's plural heading is now `Install 2 mods`; core's notice/serialization assertions use the decline sentence and offerGrants; commands' BadReason assertion now expects bad_input and the mapped reason; ignored slow J4 expects the decline sentence and asserts `Connect to example.org` in offerGrants. Searching all src/tests, including ignored journeys, found no other obsolete-text assertions. Publisher-withdrawal assertions remain correct and unchanged.
- Red/green: the new Rust tests failed on absent serve_error, withdraw_error, update_failure, update_declined, offer_grants and ModView.offer_grants, then passed. Reason tests use dist's real results for empty, 281-character and leading-space input; app adds no validation. Grant comparison includes withheld installed grants.
- Busy-port investigation (validator, real macOS): on the old code the window showed `cannot bind loopback port P`: a short cause with no way to choose another port. An empty window was not reproduced. The new AC1 test checks the recovery text in main/status, one stderr line and absent run.json. Port-aware support permits the driver's `failed` event only for an explicitly requested nonzero port and still requires a main page-load event.
- GUI validation limit: both the unchanged empty-launcher test and new busy-port test time out before driver readiness. Child stderr reports `Connection Invalid error for service com.apple.hiservices-xpcservice` and invalid message replies, as recorded for earlier sandbox runs. This sandbox cannot verify GUI behavior; the validator supplies real macOS window results.
- Validation: cargo build and cargo test pass (51 default tests); strict all-targets clippy with driver, fmt --check, JavaScript module syntax checks pass. The prior supplemental DOM pass claim for AC2/6/7 was incorrect. The actual window test tried to replace invoke, which Tauri makes non-writable and non-configurable, so it never reached those assertions. cargo build --release --features driver fails at the required debug-only compile_error. The full default driver suite was run with --no-fail-fast and no ignored tests enabled; GUI startup fails before readiness with the XPC error above. Slow J4 remains ignored as required and compiles. No dependency was added.

- Validator return 1 (app): replaced the invalid invoke override with default window tests that import `ui/words.js` (the own-withdrawal and offer-label functions also used by app.js), checking empty, single and multiple new-grant lists. AC7 now installs the real page-only Books fixture via install_folders and checks its 1.2.3 version immediately after h2, outside closed Details. The previous combined test's fake publisher notice is already covered by core's notice-priority test and the J4 withdrawal assertion. No Tauri IPC property is replaced. The validator's notice-only update_failures change is retained unchanged.
- Spec gap (app, validator return 1): TECHNICAL 1/9.5 records ui/words.js and its production/test callers; no contract or wording changed.
- Return 1 checks: the new word-module check first failed because ui/words.js was absent, then the exact own-withdrawal and zero/one/two-grant label cases passed under Node. cargo build, cargo test (51 default tests), cargo test --features driver --no-run, strict all-targets/driver clippy, fmt --check, JavaScript module syntax and git diff --check pass. Release with driver fails at the intended compile_error. No window tests were run in this sandbox on this return; the default AC2/6 and AC7 window tests are compiled for the validator.

## S29 — index and search Rust paths

- Changed fact: the hostile fixture now attempts all eight index/search commands. Its report grows from 111 to 119 attacks and from 96 to 104 invokes. `tests/gui_isolation.rs` and `tests/support_selftest.rs` keep their exact-count checks with the larger totals; every refusal and side-effect assertion remains.
- The consent model lacked List, Include and Unlist despite TECHNICAL 6.2 already specifying them; `src/consent.rs` now includes those variants. No page code changed.
- `skipped_rows` maps dist's `IndexView.skipped` (S29-1) to `[{party, cause}]`, the party from the parties cache (validator, round 1).
- Validation: macOS `cargo build` and `cargo test` pass (64 tests); `cargo clippy --all-targets --features driver -- -D warnings`, `cargo fmt --check` and `git diff --check` pass. `cargo build --release --features driver` fails at the required debug-only compile guard. Native macOS `cargo test --features driver` times out before app readiness with the existing `com.apple.hiservices-xpcservice` connection failure; the same default driver command passes all 85 tests in the existing arm64 Linux image under Xvfb/Openbox (front and slow tests remain ignored). No dependency added.

## S29-3 — find and index pages

- Added the find view, explicit index links, publisher/interface search filters,
  provenance, freshness, skipped-channel causes, consent-based result installation,
  and channel-row listing/include actions. Own index rows end either listing kind
  through a signature. All dynamic text uses `textContent`; no dependency or Rust
  command/view change was needed. Channel mods use the existing publisher search
  filter, and links use the existing channel key/base and returned slug.
- Pure sign-title and find-wording checks first failed on absent functions and
  passed after implementation. Initial native window attempts could not reach an
  assertion: the app timed out before readiness with the previously recorded
  `com.apple.hiservices-xpcservice` connection failure. The existing arm64 Linux
  image provides real hidden-window validation under Xvfb/Openbox.
- The loopback publisher in find/J4 is explicitly chosen as an index before Ivy
  is added. Dist DD-26 otherwise skips a local base supplied only by a listing.
  This preserves the production guard and keeps Alice unfollowed until the hit
  is installed; the test reads both states from `follows.json`. Another unchosen
  loopback publisher proves the skipped fingerprint and local-address cause.
- J4 now installs through the search result and consent page. Its later update,
  added grant, refusal, withdrawal/downgrade, preserved-book and fork assertions
  remain, followed by a compromised-result/no-install-button assertion.
- The first Linux suite passed the journey and all other find scenarios, but
  the new listing test's optional DOM wait returned `undefined` before the own
  row existed. Its wait now returns a boolean; no assertion was relaxed. Driver
  eval errors now include the expression and returned value for diagnosis. The
  focused listing test passes, including four signed site entries for list,
  unlist, include and unlist.
- Validation: macOS build and all 64 default tests pass; strict all-targets
  driver clippy, fmt, JavaScript module syntax and diff checks pass. Release
  with driver fails at the required compile guard. The complete existing Linux
  runner passes 64 default tests and 98 driver tests with ignored journeys
  enabled and all four front tests excluded by name. Front tests were not run.
- Validator (S29-3, round 1): natively on macOS, `cargo test --features driver -- --include-ignored` with the four front tests skipped by name passes all 98 tests, including `tests/find.rs` and the whole `tests/j4.rs`. J4 also checks that the compromised hit loses "signature verified" (its install button is absent anyway, the hit being installed).
- S29-fix1 changed test fact: the own `IndexRow.mods` now counts live `list` listings (not includes, not unlisted), no longer the own release slugs. `find_tests::ac3_indexes_put_own_channel_first_and_name_included_parties` asserted the old count (`1`, from two releases of one slug, no listings) and now asserts `0`. No other src/tests assertion (including ignored J4 and `tests/find.rs`) reads the own row's count.

## S37 — provider pick consent

- Added Pick/Candidate, kind and choice validation, and the consent answer
  argument table without a new command or dependency. Rejected answers keep
  the FIFO head. Install/sign adapters refuse a Pick defensively.
- Host's PlanEntry.binds contains all active binds, not only new ones.
  Grant and uses words use all of them; the install bind lines compare with
  the installed mod and show only new picks. Origin remains the publisher's
  claim. Release sign details append Made from, including the origin party.
- Changed test facts: the existing install JSON fixtures gain origin:null and
  binds:[]; the two existing release-sign expected detail lists append
  Made from:none. All previous field, ordering and answer checks remain.
- The new broker, argument-table, bound-word and adapter tests failed first
  on missing interfaces or the absent Made from detail, then passed. The
  page DOM helper first failed for an unsupported pick and missing install
  origin text; both scenarios pass after the page implementation. Native
  hidden window tests retain real IPC/DOM checks. The DOM helper exercises
  the shipped script with a minimal DOM and simulated focus/timers; it is
  supplemental, not evidence of native window behavior.
- Native macOS window attempts time out before driver readiness with
  com.apple.hiservices-xpcservice Connection Invalid (the existing sandbox
  limitation). No native assertion was relaxed. The hidden pick test models
  document.hasFocus locally because a hidden window cannot get native focus;
  the existing front focus/delay test remains unchanged.
- Validation: native macOS cargo build and cargo test pass (71 tests),
  strict all-targets/driver clippy and fmt --check pass. The complete default
  driver suite passes in the existing arm64 Linux image under Xvfb/Openbox
  (106 tests, seven existing front/slow tests ignored), including both new
  real-window tests and static_config. Native cargo test --features driver
  stops at the existing API window test before readiness with the XPC error
  above. No new dependency was added.
- cargo build --release --features driver fails at the intended
  "the driver feature is only for debug test builds" compile_error.
## S37-2 — local fetch and publish provider listings

- `POST /api/fetch` delegates to dist's `read_link` or `fetch`, returns the
  verified bytes and release/package headers, and requests no consent. The
  route explicitly sets the 64 KiB body limit because host does not apply its
  built-in-route limit to merged app routes.
- Publish validates the single optional provider-links header before signing,
  then lists distinct foreign providers in grant order after signing the release.
  Existing live listings need no consent. Header links take precedence over
  followed channel bases; third-party listed records are not listing authority.
  A listing refusal or failure leaves the release published and reports false.
- Red/green evidence: link fetch initially returned 404; provider publish initially
  omitted `providers`; malformed provider headers initially returned 200 and signed
  the release. Their integration tests pass after the route changes. Tests use
  committed components, real dist homes, local static sites, and recording consent;
  no build pipeline or new dependency is needed. Existing journey assertions read
  publish's release/link fields and remain unchanged.
- Native driver validation reaches the existing API window test, then fails before
  app readiness with `com.apple.hiservices-xpcservice` connection invalid (the same
  sandbox limitation recorded above). The new fetch Origin check uses a real host
  served port without a window and passes.
- A final AC3 case exposed that optional fields treated null as absence:
  a request containing both keys returned 502 rather than 400. The request now
  decodes as exactly one string field; both keys (including null) are rejected.
- The stock Linux runner reused driver artifacts from a parallel worktree at
  the shared `/repo` container path (it ran S37 consent tests absent here). Those
  results are not validation of this checkout. The retry uses the same runner
  and dependency cache with a unique `/watane-s37-2` mount/workdir; only a
  temporary copy of the runner is changed. Docker builder metadata uses `/tmp`
  because the sandbox cannot write the default `~/.docker/buildx/activity`.
- Final validation: native build and default tests pass; all eight API integration
  tests pass after the null-field fix; strict all-targets/driver clippy and fmt
  pass. The unique-path Linux runner passes all 72 default and 106 driver tests,
  including the existing consent-window API test and J2/J3/J4 journeys, with the
  runner's four front-only tests excluded and slow journeys enabled. Native
  driver execution remains limited by the XPC failure above. Release with driver
  fails at the intended debug-only compile error. No existing test was weakened.
- Validator (S37-2, round 1): natively on macOS, `cargo test --features driver` (front tests not run, being ignored) passes, including `ac6_publish_through_consent_window_and_host_guards`; the XPC failure above was the sandbox only.
