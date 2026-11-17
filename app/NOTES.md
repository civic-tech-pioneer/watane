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

## S37-3 — fetch providers, pick substitutes, and show derivation

- Provider resolution calls `fetch_for` with the consumer publisher, plans with
  binds, and counts a new bind as progress. A pick checks every call and
  subscription of every affected plan consumer, keeps search order, and binds
  all consumers pinning that provider. Background updates pass no pick resolver.
- Every fetched release with a link is considered for follow after installation,
  once per channel. Failed follows retain their channel cause without undoing
  the install. No application dependency was added.
- `derivedFrom` and hit `origin` retain ReleaseRef text. `derivedFromLabel`
  supplies the named publisher claim for Details. The confirmation adapter
  keeps the origin reference and appends the installed origin's name and party
  when the real host has it; the existing nullable `PlanMod.origin` field and
  consent UI remain in use. A missing installed origin falls back to the reference.
- `src/confirm.rs` is an unlisted file: it needs the real-host origin display
  lookup, plus the test proving installed binds are omitted from confirmation.
  No existing test was weakened; the no-host origin test still expects raw text.
- S23/S24 window fixtures explicitly choose Carol and Frank as indexes before
  stopping Carol's site. This permits loopback sites under dist's existing
  local-address guard, as in `tests/find.rs`; there is no driver exception.
  Dave also lists Carol, and the resolver passes Dave's publisher to `fetch_for`.
- New fixture components are built manually from the committed Rust/WIT sources;
  normal tests only pack and run those prebuilt components. Ratings records a
  call count, and Stats displays the value returned by that provider call.
- Native macOS window tests are written for S23/S24/S25 and compile with the
  driver feature. They are not executed in Sol's sandbox, per the task's
  macOS-driver limitation; the native validator must run them.

- Red/green evidence includes the missing candidate helper, rejected bad origin,
  raw-only real-host consent origin, and a regression where two pinned versions
  asked for the same provider twice. Replanning immediately after a pick makes
  the latter one request without changing the existing 32-round/stall assertions.
- Validation: `cargo build` and all 86 default tests pass; the driver-feature
  unit/static selection passes 64 tests; strict all-targets driver clippy and
  `cargo fmt --check` pass. `cargo test --features driver --no-run` compiles
  all window tests. Release with driver fails at the intended debug-only guard
  (the local rust-objcopy tool also warns about missing libLLVM while stripping).

## S37-4 — journey J5 on the real programs

- Added the single driver-feature `tests/j5.rs::ac1_journey_j5`, ignored as
  `slow: journey J5`. Alice, Carol, Dave and both independent Eve homes run
  the real app and their own static site servers; Frank uses a real dist home.
  The test builds this workspace's `watane-dev` executable (Cargo's artifact
  path, never PATH), then runs only `new --from` and `add --release` as child
  processes with explicit homes, isolated caches and parsed JSON results.
  Mod components come from the committed fixtures; no mod is compiled.
- Oracles cover the derived TOML/WIT, Alice's origin in Carol's release
  consent, Dave's dependency files and ordered release/list consents, the
  listing result and Dave's own index, Carol-first installation and follow,
  then Frank's candidate, confirmed bind, installed release and grant line.
  Both installed Stats pages call their real Ratings component.
- The first Eve follows only Dave and has no Carol record before installing.
  Fresh Eve chooses Dave's index while Carol is still online to retain only
  channel metadata, and chooses Frank's index; Carol's server is then stopped
  before the first install in that home, so no cached Carol package can mask
  the failure.
- Test-first evidence: the initial J5 compile failed on the absent workspace
  CLI helpers; adding those helpers made the test compile. Runtime red/green
  for the native journey is reserved for the macOS validator: this sandbox
  cannot start the driver, as specified by AC3. No existing test was changed
  and no dependency, product code or spec was changed.
- Validation: cargo build and cargo test pass (86 tests, 24.12 s reported for
  tests); cargo test --features driver --no-run compiles the complete suite.
  Strict all-targets/driver clippy, fmt --check and diff --check pass.
  cargo build --release --features driver fails at the intended debug-only
  driver compile_error. Native J5 has not been executed in this sandbox.

### Validator return 1

- Kept validator commit 2811b2f's consent heading and hidden-main focus
  refresh fixes. J5 now advertises each static server's literal loopback URL;
  it needs no public DNS.
- Authorized product exception: `src/lib.rs` now supplies dist options with
  `listed_local` enabled exactly when the `driver` feature is enabled, as
  TECHNICAL 10 specifies. The existing release/driver compile guard remains.
  The pure driver-options unit test failed with the original default options;
  the default-options unit test passed. Both pass with the feature gate.
- Conflict left unchanged as requested: `tests/find.rs:227`,
  `ac2_find_skipped_channel_and_cause`, expects the app to report `local address`
  for an unchosen loopback listing. A driver app now permits that listing, so
  this existing native driver test contradicts the specified configuration.
  `build_on.rs`, `j3.rs` and `j4.rs` contain no app local-address-refusal oracle.
- Return validation: both options unit tests pass under their respective cfgs;
  cargo build and the default suite pass (87 tests, 29.69 s test time).
  cargo test --features driver --no-run, strict all-targets/driver clippy,
  fmt --check and diff --check pass. Release with driver fails at the intended
  compile_error. No native window test was started in this sandbox.

### Validator round 2 (foreman decision)

- An always-on driver flag broke `tests/find.rs`
  `ac2_find_skipped_channel_and_cause` (refusal of a loopback listing). The
  foreman decided: a driver build sets `listed_local` only when
  `WATANE_DRIVER_LISTED_LOCAL=1` is in its environment; without it, or without
  `driver`, it is false. J5 starts each app with that variable; `find.rs` is
  unchanged and keeps its refusal check. Recorded in TECHNICAL 10 "Step 5".


## S44 — names, own identity and plain errors

- Party now carries serde `you` (default false). Own parties read `you` in
  providers, binds, grants, origins, consent, index rows and own-index hits.
  Old fixture JSON still deserializes; serialized parties now include `you`.
- The shared parties cache remembers local channel profiles rather than
  replacing them with only the current followed/index views. Core prepares
  every displayed publisher, origin, provider and bind target using
  `dist.channel(key)`, including unfollowed channels. API fetches fill the same
  cache. DistConfirm reads its request's parties locally through a weak dist
  reference; a failed profile read is visible in its details. HostConfirm
  still never calls or waits for dist.
- Origin consent fallbacks previously showed only a ReleaseRef; they now add
  the cached party label. Own listing rows name their target instead of
  showing only its key. All existing signature, grant and provenance data stay
  visible; untrusted text still uses textContent.
- `cause_chain` is unchanged: dist's `Log`, `Package` and `Release` are now
  `#[error(transparent)]`, so the S21b doubling ("X: X") no longer occurs and
  equality or a matching suffix catches every real dist chain (validator).
  Real dist package/IO/network/URL Display regressions and the publish 422
  message regression remain plain. Duplicate-version and already-listed
  app messages no longer include `at seq <n>`; dist itself is unchanged.
- Fetch request shape errors now read `give link or release` (400
  `bad_request`), including both/neither fields and both null. Wrong methods
  on each app API route return the JSON `method_not_allowed` envelope (405)
  and Allow; HEAD has no body under HTTP semantics.
- Find's old `No results` text is now `No mods found`. Index read failures
  name the index (`Could not read <name or fingerprint>: <cause>`).
  Already-included channels read `This channel is already in the index.`;
  list retains `already listed`, without a sequence. Direct and included
  provenance for the same index collapse into one through line, keeping both
  notes. An installed derived mod now reads `Installed`, with zero install
  buttons (previously one in build_on's derived-list assertion).
- Main-window action ids own the error line. Starting a newer action clears
  it; an older failure, including a delayed refresh error, cannot set it.
  Pure Node tests cover a late first failure after a newer success, and a
  late first failure after a newer failure.
- No dependencies added. Only app files changed.
- Validation: default build/test, pure Node wording/action and consent DOM
  checks, clippy with driver, and fmt pass. Release with driver fails at the
  intended compile_error. Driver tests compile, but native window runs in
  this session time out before app readiness with macOS
  `Connection Invalid error for service com.apple.hiservices-xpcservice`
  and `_LSModifyNotification ... error -50`. The targeted find/build_on runs
  and the full driver suite encounter this same environment failure. The
  full run was interrupted after repeated readiness failures (exit 130);
  window assertions remain enabled and need a working macOS GUI session.

## S58 — app completion and readable names/causes

- Saved toolset rows gain Remove, wired through the main-only
  `toolset_remove` command to host. Unknown names retain `no_toolset`;
  removal leaves tokens exactly as host does. J2 now saves and removes a
  second toolset after ending its token, asserting both row and option vanish.
  Static configuration and hostile-command literal lists include the command.
  `support_selftest::ac4_hostile_external_script_reports_every_attack_before_navigation`
  now expects 120 attacks (was 119) and 105 IPC calls (was 104).
  `gui_isolation::ac2_hostile_attacks_are_refused_without_side_effects` expects
  120 attacks (was 119).
- `confirm::index_tests::ac1_index_sign_details_are_complete_and_ordered`
  now expects `reading-list by <party> — <key>/reading-list` and
  `<party> — <key>`, including unlist Ends, for known/unknown parties.
  `ac4_host_origin_new_binds_and_bound_uses` checks known and unknown install
  origins with slug/version first; `ac8_real_host_omits_existing_binds_and_ac7_names_installed_origin`
  checks the installed name first and reference last.
  `ac4_release_ends_with_made_from_named_or_fingerprint_or_none` now checks
  readable Release and Made from values. The existing
  `consent::stage_c_tests::ac1_dist_confirm_maps_each_statement_and_broker_answer`
  release/withdraw expectations also change to slug/version before reference.
- Wrong-channel follow errors name both parties from dist's local log,
  falling back to fingerprints and using you for the own identity.
  Mod-card refresh notices now name the channel before its preserved cause,
  matching index rows; compromise, withdrawal and update notice priorities stay.
- `tests/api.rs::ac3_fetch_errors_loopback_and_host_origin_guard` now
  checks the serde cause for malformed/wrong-shaped JSON; both/neither
  fields retain exactly `give link or release`.
- `tests/find.rs::ac1_find_no_index` additionally requires no own row.
  A focused Node test covers empty own, listed own, included own and another
  chosen index. Existing own-listing assertions still require the own row.
- Saved toolsets return aligned Rust-derived `tools` alongside unchanged
  entries. Function and whole-interface mappings include unavailable/disabled
  mod entries; J2's existing MCP tool-name assertion stays unchanged.
- No dependencies added. Additional necessary files: `src/consent.rs`
  updates the existing adapter's changed consent fact;
  `tests/fixtures/hostile/ui/hostile.js` probes the new command, as required
  by the hostile-command coverage test.
- Validation: app and host binary builds pass; default app tests, focused
  Node empty-index checks, clippy with driver and fmt pass. The release build
  with driver fails at the intended debug-only compile error.
  J2, J4 and `ac1_find_no_index` fail before app readiness with macOS
  `Connection Invalid error for service com.apple.hiservices-xpcservice`
  and `_LSModifyNotification ... error -50`. The full driver suite also
  stops on the API window test's same readiness failure; its Rust/API tests
  pass. No front tests ran. Native assertions still need a working GUI session.

# S58 install consent release line

- Published install lines now read `<slug> <version> by <party label> · signature verified — <reference>`;
  previously they started with `<reference> by <party label> · signature verified`.
  Local lines and the other consent fields are unchanged. No dependencies added.
- The existing `regress_s58_install_consent_release_line_names_first` failed
  before the change and passes afterward, as do all Node consent modes
  (`regress_s58`, `pick`, `install`). No existing assertion required updating.
- `cargo build`, `cargo test`, driver clippy with warnings denied, and
  `cargo fmt --check` pass. The release build with driver fails at the
  intended debug-only compile error.
- `cargo test --features driver` passes 77 unit tests and nine API tests,
  then fails `ac6_publish_through_consent_window_and_host_guards` before app
  readiness: macOS reports `Connection Invalid error for service
  com.apple.hiservices-xpcservice` and `_LSModifyNotification` error -50.
  No front tests ran; no tests were weakened.

## S53 real GitHub

- Added opt-in `real-github` (enables `driver`) journeys
  `ac3_j3_real_github` and `ac3_j5_real_github`, ignored as
  `real: GitHub minmax-fund`. Ordinary driver and include-ignored runs
  without the feature cannot compile these tests in.
- Repositories are pre-created under `minmax-fund/watane-j-<random>` and
  recorded for deletion on drop, including unwinding. Preflight requires
  an HTTP 404 before creation. No organisation repository listing is used.
- The org token still makes `gh api user --jq .login` return the human's
  login. Both participants therefore supply their mod repository and site
  origin explicitly. Carol's repository retains Alice's tagged ancestry,
  but is not a GitHub fork (`push.mod.forked == false`). Real fork and
  owner selection remain out of scope.
- Child output is redacted before JSON parsing and assertions; authentication
  is supplied only through child environment variables, with isolated gh and
  git configuration. The made-up-token self-test failed before redaction was
  implemented, then passed.
- The foreman ran both real journeys with the token: `ac3_j3_real_github`
  passed (143 s), `ac3_j5_real_github` passed (222 s), and no
  `watane-j-*` repository was left. No difference from `fake-gh.rs` showed
  up in what dev runs, so `fake-gh.rs` is unchanged.
- Acceptance run (user-sim, after rebase on main): both real journeys and
  both fake journeys (`ac2_j3_github`, `ac2_j5_github`) passed; no
  `watane-j-*` repository left; the token never appeared in output.

## S71 — J5 organisation fork

- Fake J5 now publishes Carol as `carols-club` while her gh login stays
  `carol`; Alice still uses the default owner. Real J5 reserves Carol's
  `minmax-fund/watane-j-<random>` mod name instead of pre-creating it, then
  publishes with `--owner minmax-fund` and checks the GitHub fork parent and
  `watane` default branch (S53 expected `forked == false`, S71 expects true).
  Her site still uses a pre-created origin. The committed Ratings component's
  public export is renamed alongside its WIT package; no mod is compiled.
- Reservations retain the existing random-name rule and mandatory HTTP 404
  preflight, and are recorded before creation. Drop uses an included HTTP
  response for DELETE so an unused reservation's 404 is silent; the prefix
  guard still applies to every deletion. Local child-process gh stubs cover
  reservations, creation, cleanup, failed preflights and the deletion guard.
- Validation: the harness self-tests live in `support_selftest.rs` (run with
  `--features real-github`), so they run once, not in every test binary.
  Fake J5, both clippy runs and the real J5 (foreman, minmax-fund) passed.

## S52 front tests

- S52: Front tests now pass early on a locked screen with one explicit stderr skip line; unknown ioreg session state still panics, and focus assertions remain unchanged. All four real front tests passed locked with one skip line each; build/default tests (112), clippy and fmt pass. In the Codex sandbox the full driver suite fails before API consent-window readiness with the macOS hiservices XPC error; natively it passes (validator). No dependencies added.
- S52 front run (app, foreman, 2026-11-15 20:14-20:20, macOS 27, unlocked, terminal in front): all four front tests pass for real (gui_isolation ac1 in 310 s, most of it waiting for 30 s of idle time; gui_shell ac3_consent_takes_focus, ac3_reopening_an_open_mod_focuses_it, ac4_approve_delay_and_deny_focus in 32 s). First run of these four. On a locked screen each skips with one visible stderr line.


## S74-2 — main mod and manager (stage I)

- Startup reads `app/main`, removes an unknown installed name, and opens an
  enabled main page alone; otherwise it opens Library. Selection, unset and
  removal persist through that file. Every app install entry point shares the
  proposal adoption hook, with the first proposal in install order winning.
  Installed folders are only read through pkg's `Mod::read`; a read failure
  makes `proposesMain` false. No dependency was added.
- Native Mods… and Settings… menu items route the main window; its close no
  longer unconditionally quits. Closing the last non-consent window does.
  The driver waits for whichever startup page opens and has a `menu` op using
  the same event-loop handler. Tests use hidden windows, with no front tests.
- Library, Find and Updates are separate from Settings. Connection links use
  host's resolved local providers and the corresponding grant state. Disable
  and remove first warn about enabled dependents with Cancel focused; removal
  then keeps the established data question. Automatic update history uses
  host's `Plan::needs_confirmation`, holds at most 50 successes, and is not
  persisted. Journeys navigate explicitly with the `journey::area` helper.
- Stage G is absent in this checkout (the app has no node dependency or machine
  commands). Settings explicitly labels its machines section unavailable;
  implementing machines remains outside this task.
- Red/green checks: the new command capability expectations failed before the
  permissions were added; connection/warning/filter modules failed on missing
  exports; the removal and startup policy unit tests failed on missing methods.
  The new quiet-window tests cannot reach their assertions in this sandbox:
  macOS reports `com.apple.hiservices-xpcservice` Connection Invalid and the
  existing startup readiness timeout fires. This is the earlier documented
  sandbox failure, not a reason to relax any window assertion. Docker is not
  available (`~/.docker/run/docker.sock` is absent).
- `--skip front:` does not filter ignore reasons in libtest. The full driver
  run therefore also explicitly skips all four existing front-test names,
  while including ignored journeys and continuing through failed suites.

- Validator round 1: menu item names now use `item`, distinct from the
  numeric request `id`. The summary moved into Details, restoring the version
  directly after the name heading. J2 explicitly checks and continues the new
  dependent warning, retaining every prior assertion. App::eval again sends
  only the supplied JavaScript; J2/J4 navigate explicitly between areas.
  J5 is unchanged and uses commands rather than Settings DOM controls.
  All 118 default tests, strict all-targets/driver clippy and formatting pass;
  window suites are left to the native validator and were not run here.

## S54 real static host

- Two real runs (2026-11-16) saw 503 for a missing path after Pages had
  served the site, once after a 404 had already been seen for the same URL.
  The test accepts either not found or HTTP 5xx; dist reports
  `HTTP 503 from <url>` without a retry. Real J3 checks Ivy first,
  then waits up to five minutes for the missing head to answer 404 before
  Dan attempts the missing-base install.

- Real J3 enables GitHub Pages on the reserved site repository's `main` root
  and waits up to ten minutes for its binary `head` to equal the fresh clone.
  Alice has no loopback server. Ivy remains a chosen loopback index and lists
  Alice's public HTTPS base with the default local-address guard unchanged.
- The missing-path check keeps Alice's key and slug, changes the base to an
  unpublished subdirectory, and requires app code `dist`, `not found` or
  `HTTP 5<two digits> from`, and that subdirectory's `head` URL within
  60 seconds, with Dan's mod list empty.
  Bob still unfollows Alice before choosing Ivy to exercise a listed record.
- A known channel (followed or listed by a chosen index) is read from its recorded bases, so the missing-base check uses a fresh home.
- S54-fix2 replaces the red Ivy regression with a fresh-dist-home loopback
  check; the earlier AC5 test installs and unfollows first, so is not equivalent.
- Local gh stubs check Pages POST arguments, repository guards, HTTPS and
  trailing-slash normalization. The foreman's first real run timed out after
  600 seconds with no HTTP status: reqwest's dev features had no TLS support.
  Pages polling now uses the added `ureq = "3"` dev-dependency (rustls, as in
  dist); reqwest's features are unchanged. Deadline errors retain the last
  redacted client error as well as status. A local plaintext listener proves
  the client attempts TLS and reports its failure in under five seconds.
  Hakari generation needed no changes and verification passed.
- Real run (foreman, 2026-11-16): `ac3_j3_real_github` passed in 136 s
  total, over real TLS from GitHub Pages; afterwards minmax-fund held only
  OpenMLOps (the reserved site repository was removed). Pages first-serve
  time, content types and the CDN 404 body were not recorded.
- Validation: `cargo build`, default tests, test compilations with `driver` and
  `driver,real-github`, both clippy feature combinations and
  fmt passed. All 20 harness self-tests passed with `real-github` enabled.
  Release with `driver` failed as required (`the driver feature is only for
  debug test builds`). The fake J3 failed in the Codex sandbox (hiservices
  XPC); natively it passes (validator, 39 s). After the TLS fix, the driver
  suite still fails at API GUI readiness with the same sandbox hiservices
  XPC error; default tests (115), both feature compilations/clippy, fmt and
  the 20 harness tests pass.
- S54-fix1 validation: all 24 real-github harness self-tests and 118 default
  tests pass; builds, test compilation and clippy pass with `driver` and
  `driver,real-github`; fmt passes. Release with `driver` fails at the
  required debug-only guard. The driver suite fails before API consent-window
  readiness with the same macOS hiservices XPC / `_LSModifyNotification`
  error -50 recorded above. The real GitHub journey was not run (no token).

- S54-fix2 validation: 117 default tests and all 23 real-github-enabled
  support self-tests pass. Builds, test compilation and clippy pass with
  `driver` and `driver,real-github`; fmt passes. Release with `driver` fails
  at the required debug-only guard. The driver suite and fake J3 both fail
  before app readiness with macOS `hiservices-xpcservice` /
  `_LSModifyNotification` error -50 in the sandbox. Real GitHub J3 was not
  run (no token); the foreman must run it and validate fake J3 with a GUI.

- S54-fix3 validation: an extracted assertion check failed on HTTP 500 before
  the change, then passed for not found and all 500–599 statuses, rejecting
  malformed statuses and other errors. Exact stderr lines omit a synthetic
  token. The updated note passed a content check that failed on the old note.
  All 117 default tests and 23 real-github support self-tests pass, including
  the S54-fix1 source check. Builds, test compilation and clippy pass with
  `driver` and `driver,real-github`; fmt passes. Release with `driver` fails
  at the required debug-only guard. The driver suite again fails before API
  consent-window readiness with macOS `hiservices-xpcservice` /
  `_LSModifyNotification` error -50. Real GitHub J3 was not run (no token).
  The foreman's real GitHub J3 on b12b373 passed in 89 s; that run's edge
  answered `S54 missing base: not found`, so the 5xx branch is still unproven
  against the real host.
## S75 — app stage G (validation incomplete in this sandbox)

- The running home opens through `watane_node::Node`. Network-facing reads,
  signatures and post-install retention use its wrappers; the existing dist
  handle supplies views. The running router owns the same node and exposes
  no machines, pairing, relay, DHT or net route. The old standalone
  `api::router(Option<Arc<Dist>>)` remains for unchanged HTTP contract tests.
- Settings now has the own-machines controls, code copy/expiry, identity-loss
  and removal questions with Cancel focused, relay/DHT consent, device
  status and problems, replica identity text, and verified restore hints.
  `Join` and `Reach` accept only plain approval or denial through the broker.
- Fixed the keyless parties/index views and the post-choose `NoIdentity`
  failure. Node owns `IdentityInUse`; its text now says "has signed at least
  one statement", and its specification and regression test agree.
- New dependency: `watane-node` (workspace path), required to coordinate
  distribution with own machines. `workspace-hack/Cargo.toml` and the lockfile
  were regenerated with hakari; no other dependency was added.
- Changed test fact: the hostile fixture now tries 12 more commands. Its
  report grows from 120 to 132 attacks and its recorded invokes from 105 to
  117. `gui_isolation` and the standalone JS harness (`support_selftest`)
  retain their exact-count and refusal assertions at those larger counts.
  `support_selftest.rs` is an unlisted file needed to keep that oracle true.
- Test-first observations: the new consent test initially failed because
  `join` was not a Request variant; the keyless read initially failed with
  `NoIdentity`; the expanded capability literal failed against the old main
  capability; node's singular statement text test failed against the old
  plural. Those regressions now pass. The window scenarios could not reach
  their acceptance assertions, so a complete acceptance red/green cycle
  could not be established here.
- Validation: `cargo build -p watane-app` succeeds (the linker reports a
  compact-unwind size warning). `cargo test -p watane-app --features driver
  --no-run` succeeds. Clippy with all targets, driver, and `-D warnings`,
  `cargo fmt --check`, JS syntax checks, and `cargo hakari generate --diff`
  are clean. `cargo build -p watane-app --release --features driver` fails
  at the required "driver feature is only for debug test builds" guard.
  Node's `ac10_identity_in_use_is_true_after_one_statement` passes.
- Environment failures, retained as failures: the full default and driver
  suites fail the new real-pairing broker test because iroh reports
  `Failed to create netmon monitor`. The dedicated machines suite reports
  1 pass (route boundary), 9 failures before app startup: macOS refuses
  `com.apple.hiservices-xpcservice` and the driver cannot become ready.
  The earlier full driver run also failed the existing API window test at
  that same startup boundary. No `front:` test was run. A focused run of
  the unit, HTTP API, static configuration and JS harness suites, explicitly
  filtering only that real-network broker test, passes 120 tests. This is
  partial evidence, not a substitute for the full suite or S28–S36 on a
  macOS GUI session with network-monitor access. `tests/j8.rs` is untouched.

### S75 validator round 1

- Preserved validator commit `61c5782` (node error rustfmt, relay removal
  assertions, and the CLI scenario's slow reason).
- AC6 discovers and verifies the release by search before fetching. It first
  fetches the exact package bytes to warm the full machine's blob reference,
  clears the light cache, then changes the referenced file. This distinguishes
  sending changed bytes (a hash refusal) from rejecting a bad file on its
  first admission at the serving holder. The test now requires the named
  machine's hash-mismatch cause and confirms no bad package was stored.
  The no-path half restarts the light home after stopping the full home:
  refusing a peer deliberately drops it for that session, so reusing that
  session would never dial it. The fresh session asserts net on, no relays,
  DHT off, and an `unreachable` cause naming the same peer, a relay and DHT.
- AC4 now has anchor, full and light homes. It checks the role changes and
  expiry in all three views, including the anchor. With the anchor off and
  no local package or web base, a byte-equal successful fetch proves the
  full replica serves the light home. The light home goes offline and its
  cache is cleared before removal. After the full replica learns the
  removal, the anchor goes off again and the light home retries with its
  old list. Assertions keep its machine state `light`, require a failed
  fetch and no cached package: a local Removed state or an undiscovered
  release cannot satisfy this test.
- Settings navigation now refreshes immediately even in a hidden native
  window. The added JS behavioral test failed first (`0` refreshes instead
  of `1`) and passes after the change. AC8 retains the driver restore and
  now checks the restored fingerprint as well as the machine hint and
  next-sequence assertions, exercising navigation rather than the hidden
  window's intentionally suspended polling timer.
- Removed the test-only `follow_channel`, `choose_index`, and `fetch_release`
  shims from `core.rs`. Their existing tests retain their assertions and
  call production `map_follow`, `choose_index_with`, and `Node::fetch` /
  `Node::follow` directly. The own-index check additionally asserts that
  the choosing callback cannot be reached.
- Validation: the focused unit/API/static/JS suite passes 120 tests with
  only the real-network broker test explicitly filtered; the new hidden
  Settings regression passes. Build succeeds; clippy with all targets,
  driver and `-D warnings`, fmt, hakari diff, and JS syntax checks are clean.
  Release with driver is rejected at the expected compile guard.
- Native acceptance still needs the validator: this sandbox again refuses
  `com.apple.hiservices-xpcservice`, so all nine window scenarios in
  `machines.rs` (including corrected AC4, AC6 and AC8) time out before app
  readiness. Its two non-window tests pass. Full default/driver runs also
  retain the `Failed to create netmon monitor` failure in the real-pairing
  broker test; the driver run passes its other 94 library tests before
  stopping there. No `front:` test was run. The failing tests remain
  enabled, and no acceptance assertion was weakened for the environment.

## S75-2 — J8 journey (native validation incomplete)

- Added one driver-only, ignored slow `ac1_journey_j8` in `tests/j8.rs`.
  It uses the committed reading-list component, hidden app homes with
  loopback net binds, and the existing real `watane-node` process helper.
  No dependency, app/src code, existing test, or shared helper changed.
- AC1/AC2 compare the CLI short key with the consent DOM, check the
  joined list and Alice's online full machine, and refuse a reused code
  in a fresh home. AC3 polls the actual server package file against the
  packed fixture bytes with a five-second deadline starting at approval;
  no static server has been created at that point.
- AC4 creates Ivy with watane_dist only after AC3. Ivy's `list` reads
  Alice's head at the link's location (the API origin gave 404 natively),
  so Alice's site folder is served only now, and stopped with Ivy's.
  The server's printed verified pull proves it holds Ivy's listing.
  Ivy's site is stopped and Alice's net turned off before the light app
  searches and installs, so the server is its only full machine. The hit
  must name that server alone and be verified; the mod opens and stores
  a book through its page.
- AC5 warms the server's blob reference by installation, clears the light
  package cache, changes one server-file byte, and requires fetch failure,
  a hash-mismatch problem naming the server, and no cached bad package.
  AC6 repairs the bytes and restarts the light app (bad peers are dropped
  for the session), then proves a successful byte-equal fetch before
  removing it. The light app stays offline on its old device list until
  the server prints the new devices-change event. Its next fetch must
  fail while it still reports Light, with no package cached. Node's CLI
  prints that devices-change event; it does not emit a new Refused event
  merely for denying an untrusted caller's request.
- AC7 follows spec/FUNCTIONAL J8 and the fixed node interface: the restored
  identity remains anchor, issues a full code, and the existing server
  re-joins with `--replace`. Making the restored app join would delete its
  restored identity and make hint confirmation/publishing impossible.
  The identity DOM must show the server and seq; publish must return the
  unconfirmed-head error before confirmation, then publish seq 1 after
  the server's seq-0 hint. AC8 checks every machine's persisted empty
  relays and false DHT, including the refused-code home.
- Test-first limit: the initial AC1-only journey and the complete journey
  both compile but fail before app readiness: macOS rejects
  `com.apple.hiservices-xpcservice` with Connection Invalid. None of the
  acceptance assertions can run here, so this is not a demonstrated
  acceptance red/green cycle. Assertions remain enabled and unchanged.
- Checks: build succeeds (the existing compact-unwind linker
  warning); all-targets driver clippy with warnings denied and fmt pass.
  The default suite fails only its existing real-network pairing test at
  `Failed to create netmon monitor`; its other suites pass. Release with
  driver fails at the required debug-only compile guard.
- The requested J8 run and J2/J3/J4/J5/J9 `--include-ignored` regression
  command all fail at the same app-readiness boundary, before their
  journey assertions. J5's non-window origin check passes. Both full
  default/driver commands and every ignored-journey command explicitly
  exclude the four task-listed focus test names; no `front:` test runs.
- The full driver run (`--no-fail-fast`) completes: 140 tests pass and
  50 fail. The real-pairing broker fails to create netmon; the native
  window failures time out before app readiness with the same macOS
  service rejection. The default suite completes with 126 passes and
  that one netmon failure. J8's dedicated driver compile (`--no-run`)
  succeeds. These results do not establish AC1–AC8 or AC9's journey
  regression requirement; a native macOS validator must still run them.

### S75-2 validator round 1 — strengthen AC6

- Preserved validator commit `6037ae2`: Alice's static site starts only
  after AC3 so Ivy can read her head when listing the mod. The validator
  established AC1–AC3 natively; AC4 is blocked by net's missing server
  address on the newly joined light machine. No address-book workaround
  was added to J8.
- AC6 keeps the byte-equal successful fetch and the removal refusal in
  the same light app session. The AC5 recovery restart occurs before
  both fetches. Alice's net and both static sites are off for the positive
  fetch; its cached package is removed before removal and the retry.
- AC6 (as d141db6 and the round-2 validator left it): the light's fetch
  must fail with `fetch_failed` while it is still locally light, after the
  server printed `machines <seq>:` without it. The successful control runs
  after the same light off/on cycle, so removal is the only difference.
  Natively, without `machine_remove` the same fetch returns 200 (checked by
  mutation in round 2). The error text is dist's no-base fallback
  ("channel ... is not followed; ..."): node's `need_package` drops net's
  blob error, so neither the server's key nor `not_allowed` reaches the
  app. If net's text ever reaches it, the test requires
  `node <server> refused: not_allowed`.
- Validation: J8's driver `--no-run` compile and all-targets driver clippy
  with `-D warnings` pass; formatting is clean after rustfmt. No native J8
  run was attempted in this sandbox, per the validator's return
  instructions. The strengthened removal assertion awaits native validation.


### S77 — app stage J manager consent

- Implements `HostConfirm::manage` and `reduced` using host's existing
  interface, with Q28–Q33 option (a). No new dependency, Tauri command,
  permission, route or fixture. S77-2 owns Desk's page and J10.
- Manager installs use the existing provider resolution path with the
  manager carried into picks and `Requester::Mod` carried into install.
  A supplied link only checks the already-read channel; its base is never
  followed. All async app work stays on the app runtime, with the join
  awaited from host's runtime. Dropping the host future aborts the app task.
- Reduction notices are memory only and clear on enable; release/channel
  notices retain priority. Disable and close-page post a close to the main
  thread without waiting for it.
- Legacy requests omit absent `asked_by` when serialized, preserving the
  earlier exact-JSON test; deserialization defaults it to `None`. The new
  Manage request rejects withheld grants and provider choices.
- The table's manager titles alone omitted the asker's publisher/local
  status required by APP-F-71. Section 17.4 now explicitly puts the same
  asked-by line first for every manager request.
- Unlisted file: `tests/support/consent_dom.mjs`, extended to execute the
  shipped consent script for all actions, literal hostile names and the
  checkbox-free grant. This gives DOM coverage alongside the required
  native `gui_shell` tests when the sandbox cannot start native apps.
- Red: missing manager request types/labels and mapping/reduction helpers
  failed compilation; the DOM test failed on the missing asked-by line.
  The cancellation test failed on the missing task guard. Green: all new
  unit tests and the DOM companion pass. The final library run has 101
  passes and one existing netmon failure (`Failed to create netmon monitor`).
- Build succeeds; clippy with all targets and driver under `-D warnings`
  and fmt pass. The release driver build fails at the required debug-only
  compile guard. Native manager consent tests fail before readiness with
  `com.apple.hiservices-xpcservice` connection errors and
  `_LSModifyNotification` error -50, matching earlier sandbox findings.
  The final default run totals 137 passes and that one existing netmon
  failure. The four task-listed front tests are excluded by their exact
  names. No existing assertion was weakened or removed.

- Final isolated driver regression (`--include-ignored --no-fail-fast`, with
  the four exact front skips): 152 pass, 61 fail across 20 targets. Every
  panic is either app readiness or the existing netmon error; all new unit
  tests, all DOM companions and all 16 static-config tests pass. The
  native AC5 case fails before its assertions. AC5's native checks and
  AC6's full-regression acceptance still need native macOS validation.
  This run followed the final default build, with no concurrent build
  overwriting the shared app binary. Logs: `/tmp/s77-driver-final.log`,
  `/tmp/s77-default-final.log`, `/tmp/s77-confirm-final.log`.


## S77-2 — Desk page, management scenarios and J10

- Added `tests/fixtures/desk-ui/ui/`, `tests/support/manage.rs`, six quiet
  window scenarios S43–S48 in `tests/manage.rs`, and ignored slow J10 in
  `tests/j10.rs`. The S74 `desk` fixture and `tests/manager.rs` are unchanged.
  No dependency, product code, security check or spec changed.
- Desk copies host's prebuilt `manager` (local name `manager`, window
  `mod-manager`), keeps its Desk name/summary and adds `main = true` and the
  button page. Weather and Notes copy the existing app fixtures. Book club
  uses host's prebuilt counter, published by Ivy and discoverable through
  her chosen index. Dist includes an index owner's own releases and refuses
  an explicit self-listing; the fixture check independently chooses Ivy and
  searches for Book club. No new mod is compiled. The grant-free update
  edits equal-length WIT interface version strings as the existing publishing
  helper does. S46 re-reads the chosen index with `index_choose`, avoiding
  the automatic update that `channels_refresh` would otherwise start.
- The page has one button for every manager API export, including its three
  diagnostic functions; grant JSON and delete-data/request-id controls are
  explicit. It writes results, failure codes and catalog/request notifications
  with `textContent`. S45 checks the first consent paragraph, deny/approve
  outcomes and mods on disk, then strictly requires WouldBlock from an unread
  channel's non-blocking loopback listener after the failed outcome. S47
  proves Notes received private state before deletion and has none after
  reinstall. J10 attempts a valid current consent answer from Desk, checks
  the ACL refusal and pending state, and withdraws manage during a removal
  to check cancellation, preserved Notes and the next not_granted error.
- Red/green: the first AC1 test failed on absent desk-ui/index.html; the
  fixture/package test then caught a missing required summary in the copied
  manifest. Both AC1 tests now pass, including all page button arguments,
  result/error rendering, proposed-main metadata and actual index search.
- Validation: cargo build passes. cargo test --no-fail-fast has 137 passes
  and the existing pairing test failure at consent.rs:1074: `Failed to create
  netmon monitor`. The default driver command stops at that same existing
  failure (102 library passes). All driver test targets compile with
  --no-run. Strict all-targets/driver clippy, fmt --check and diff --check
  pass. The release-with-driver build fails at the intended compile_error.
- All six S43–S48 cases and J10 were attempted; none reaches a scenario
  assertion. Native macOS reports `_LSModifyNotification` error -50 and
  `com.apple.hiservices-xpcservice` Connection Invalid, then driver readiness
  times out, as in S77-1. The exact requested combined command also fails
  before J10 startup; --no-fail-fast is used to attempt both final targets.
  The four front tests are always excluded by their exact names. Docker's
  socket is absent, so the Linux runner is unavailable. AC2/AC3 runtime
  behavior and AC4 green acceptance still need a working native GUI session.
  No participant-free refusal was observed: startup never reached manage.
- Later (validator, user-sim, outside the sandbox): S43 to S48 and J10 run and
  pass; the startup failures above came from Sol's sandbox only.

## S81 — The app says what it means

- Only the Library's main entry shows `.main-mark` with `Opens at start`;
  unsetting main removes the marker. Install consent has a snake_case
  `PlanMod.becomes_main` flag (default false for older fixtures) and says
  `<name> opens when watane starts, in place of the manager.` only for the
  selected proposal when no main is set.
- Host's PlanEntry has no proposal metadata. MainMod reads pkg's `main()`
  from host's documented staging layout (host TECHNICAL 4.2), matches the
  package id including any source files, and selects in request order.
  This avoids a sibling interface change for folder and published plans.
  src/confirm.rs is an unlisted required edit: it maps that choice into the
  host install request's app consent model. No dependencies were added.
- Connection links prevent their default hash navigation, open the target
  details, and focus its heading button. Disable/remove warnings distinguish
  granted enabled callers (`These mods use it`) from subscribers (`These mods
  extend it`), uses first. A mod with both kinds occurs in both lists.
  A local name appears once per list. Cancel remains focused.
- Test-first: DOM checks failed on the missing marker, missing consent line,
  source entry retaining focus and the old extension-as-use wording. The
  proposal unit test failed with None instead of Desk; the spec check failed
  on the missing Library wording. Each passes after implementation.
- New tests: manager.rs has ac1–ac4_s81 DOM and native scenarios, plus
  ac5_s81_words_and_focus_are_documented_at_their_facts. main_mod.rs has
  ac2_s81_consent_selects_first_proposal_only_without_a_main, also checking
  pending plans with equal slug/version, source-bearing packages and order.
  The only changed existing test is
  consent::tests::ac9_request_and_answer_json_match_spec, which now expects
  becomes_main:false. No existing Extended-by-only text assertion or
  connection-link click test was found to change; none was weakened.
- Validation: cargo build passes; default cargo test has 102 passes and the
  existing pairing failure `Failed to create netmon monitor`. The exact
  requested driver/include-ignored command (all four front tests skipped by
  their exact names) has 103 library passes and that same failure before
  integration targets. API, static config and support selftests pass (30).
  Strict all-targets/driver clippy and fmt --check pass. Release with driver
  fails at the required `the driver feature is only for debug test builds`.
- All five S81 DOM/spec tests pass. All four new native scenarios were
  attempted; each fails before any scenario assertion, with HIServices XPC
  Connection Invalid, _LSModifyNotification -50, then driver readiness
  timeout. AC1–AC4 native validation and AC6 remain PARTIAL for the validator
  in a working GUI session. No screen-taking front test was run.

## S81-2: own machines wording

The anchor heading names its role once; replica headings say whose machine
it is. The local machine row omits remote liveness fields. Join disables
before reading status and stays disabled across refreshes; failure and
cancel allow retry. Relay and DHT controls follow the own-machines switch.
A first signing consent warns that the home can no longer join; restored
unconfirmed-head errors point to Settings, identity, From your machines
(or restore again). These are app mappings; no node/net/dist changes.

Changed fact in tests/j8.rs: the restored publish refusal now gives the
Settings recovery path instead of dist's bare cause. No check is weakened.

Validation in the sandbox: S81-2 unit and JavaScript DOM regression tests
pass; build, clippy with driver and `-D warnings`, and fmt pass. A release
build with driver fails at the intended compile guard. The driver suite
uses `--include-ignored` and the four explicit front-test skips. GUI tests,
including `ac1_journey_j8`, cannot reach app readiness: HIServices XPC
returns Connection Invalid. After repeated startup failures, the remaining
driver run was stopped; J8 was also attempted directly. The existing real
pairing unit test also
cannot create its netmon monitor. AC8 therefore needs validator execution
in a macOS GUI session. No test or security check was weakened.

`src/lib.rs` is an unlisted task file: the existing `From<DistError>` for
`CmdError` lives there, and mapping the restored-head error there keeps
command and HTTP messages consistent without duplicating the conversion.
No dependencies added.

### S81 reloads, S30 and temporary-folder audit

- Desk marks an outstanding button call in its reload URL fragment with
  `history.replaceState`. If a grant change destroys that JavaScript context,
  the replacement document consumes the marker and reports `page_reloaded`.
  A completed call clears it. No effect is retried or claimed to have failed.
  No dependency, command, capability or sibling implementation changed.
- Round 1's session-storage signal was unreliable in native WebKit: the
  give-back pass timed out waiting for `page_reloaded` (validator reported
  5/20 and 6/10, plus a full-driver failure). An asynchronous storage flush was suspected,
  not proven. The replacement uses navigation state instead of storage.
  Before either grant command, the quiet test confirms the held response,
  the page's marker, and the same URL in the native view reported by the app.
  This establishes the reload address on both sides of WebKit's process
  boundary without a fixed delay or a storage-persistence assumption.
- `ac1_manage_calls_settle_across_grant_reload` holds a real
  `api.request-remove` response during withdraw/give-back. It checks the
  replacement's code and a new call's result/error. Host owns the old outbox
  and cancellation: revoke ends the pending request `denied`, drops
  HostConfirm's app task and removes its consent. The test reads that exact
  request after give-back, checks Notes remains, then proves another request
  can be denied normally. The give-back-time call starts without manage and
  has no management effect or consent to leave behind.
- Core clears `driver.loaded(label, false)` before requesting a grant reload,
  so the next driver wait observes the replacement document's page-load
  event. Validator commit `6ec012b` adds that wait after `grant_withdraw`
  in J10; it relies on this invalidation and is retained unchanged.
- S30 retains its assertions and waits for the complete identity text,
  including the light role and anchor key. Native validator: **20/20 pass**.
- Native validator counts in the real `$TMPDIR`: driver **25 before, 25
  after**; default features **25 before, 25 after**. The sets of names are
  identical. No new temporary folder is retained by either run.
- Lifetime audit: app tests have no `TempDir::keep`, `into_path`,
  `mem::forget` or helper `process::exit`. The `into_path` calls in
  commands.rs convert dialog `FilePath` values, not temporary directories.
  Copied fixtures keep a scoped/fixture-owned TempDir; App and NodeProc kill
  and wait for their children before the caller's home drops. No app test
  intentionally retains a temporary folder, and no cleanup implementation
  was changed without a reproduced app leak.
- Round 2 red/green: the DOM regression now supplies navigation state and
  makes session storage unavailable. It fails with the old storage marker
  and passes with the URL marker. The unchanged fixture/page check remains
  part of validation. The native AC1 20-run loop and full-driver green check
  are delegated to the validator, since this sandbox cannot start windows.
- Round 2 local checks: build, all driver targets' `--no-run` compilation,
  strict driver clippy, fmt and diff checks pass. The default run's DOM
  regression and the unchanged Desk fixture/page check pass. Both full test
  commands encounter the existing netmon pairing failure; the direct quiet
  reload test stops at HIServices readiness. The release-driver build fails
  at the required debug-only compile guard. Logs: `/tmp/s81-r2-*.log`.
- Round 2 validator: the URL marker alone passed 7/20 natively. The cause
  was overlapping calls, not storage: the first pass's post-reload `mods`
  check could be satisfied by the load-time `read()` showing the same
  `not_granted`, and the still-running `mods` click then cleared the marker
  set by the next pass's held call. Desk now counts outstanding button calls
  and clears the marker only when the last settles; the test waits for the
  cleared marker after each post-reload call. Native: **20/20 pass**.
  Round 2 native runs: driver suite with `--include-ignored` (front four
  skipped by name) 242 passed, 0 failed, J10 included; default 142 passed.
  Real `$TMPDIR/.tmp*`: 27 before and 27 after each run, same names.


### S82 read consent and stop asks

- Added `Request::Read` before any network access for a manager's unread
  link. Denial ends the manager request as `denied`. Approval reads the
  release, then uses the existing managed install/provider-pick path and
  its separate install consent. A completed install follows the fetched
  channel through `follow_fetched`.
- Corrected app 17.3: dist's `open_link` immediately follows a channel;
  `node.read_link` returns the verified package and link without following.
  Retaining that `Fetched` keeps a denied install from adding a follow.
  The existing dist listed-location checks remain; the loopback journey
  uses the existing driver-only `WATANE_DRIVER_LISTED_LOCAL=1` switch.
- Every consent with `asked_by` offers the stop button after Deny, outside
  the approve delay. `stop` is optional; a true value denies only a request
  with an asker. The command then calls the details switch's `Core::grant`
  withdrawal path, including S81's page reload. The stop journey captures
  the request id before reload, checks the withdrawn grant and unchanged
  Weather, checks `not_granted`, then gives back the grant and reads that
  original request as `denied` before approving a new request.
- S45's unread-link expectation changed from `failed` to a read consent,
  followed by `denied`; its no-connection and unchanged-mods assertions
  remain. The previous confirm unit test now expects the Read request
  rather than the obsolete unread-channel error. No tests were weakened.
- Red/green: the read-answer test failed on the unknown `read` variant;
  the stop-answer test failed on the missing answer mapping; the shipped
  consent DOM test failed to finish rendering a read request; the real
  read/planning test failed on the missing read adapter. They now pass.
  The native tests could not reach their behavioral assertions: macOS
  refuses the sandbox's HIServices XPC connection at app startup.
- Validation: `cargo build` passes (one existing linker unwind-table
  warning); both full test commands stop at the existing real pairing
  test with `Failed to create netmon monitor`. The default suite with only
  that test excluded passes 144 tests. All driver test targets compile;
  the four selected consent/manager DOM tests pass. Strict driver clippy,
  fmt and diff checks pass. The release-driver build fails at its required
  debug-only guard. Native AC2, AC3, AC4, AC5 and AC6/J10 validation remains
  partial in this sandbox; none of the four screen-taking tests was run.
  Logs: `/tmp/s82-*.log`. No new dependency, command, capability, sibling
  code or unlisted file changed.

### S82 Open page

- `HostConfirm::reduced` posts `OpenPage` to the main thread through
  `Core::post_open` (beside `post_close`; `core.rs` owns the app handle).
  On the main thread `confirm::open_page_target` rechecks: an open consent
  (`Broker::current`), a mod now off or gone, or no page drops the open,
  not kept for later. Otherwise `views::open_mod` opens or focuses
  `mod-<local>`. No notice is kept.
- The no-page target in the window test is host's `echo` fixture:
  `counter` has a page and cannot show `bad_request`.
- `ac3_s82_open_page_is_unique_and_dropped_during_consent` checks the
  dropped open by a later open of another mod: posted window work runs in
  order, so once the later window is up the dropped one would be too. A
  menu round trip drains posted main-thread work; no sleep, no focus check.
- Validator native run: driver suite with `--include-ignored` (front four
  skipped by name) green, J10 and S43 to S48 included; removing the consent
  check in `open_page_target` fails the S82 window test.


### S84-2: Rust support for the manager design

- `HostConfirm::installed` adopts only the first proposing mod in
  `Installed.new`, for every requester. It reads installed metadata and
  writes the shared main choice directly, with no host catalog call,
  consent wait or event-loop dispatch. App commands retain only their
  update-failure cleanup. `Installed.updated` cannot adopt main, and an
  update/unchanged plan cannot claim the start-window line in consent.
- Moved the former main_mod install-order/existing-choice assertions into
  `confirm::tests::ac1_installed_adopts_only_first_new_proposal_for_every_requester`,
  preserving the missing/invalid-metadata checks and adding LocalApi,
  Mod and update-only reports. The startup test selects its fixture main
  explicitly; adoption is exercised by the callback test and S51.
- `GrantLine.short` defaults to empty; empty values are omitted in consent
  JSON, preserving the existing exact-JSON test and full consent words.
  Only ModView grants and offerGrants receive short words. Section 4's
  names map now uses provider identities rather than local names: a grant
  identifies a provider by publisher/slug, and a bind can connect two
  providers with the same slug but different installed names.
- Refresh times live only in Core memory. checkedAt records completion of
  node.refresh; readAt advances for Current/Behind reports. Failed reads
  and already-stopped compromised channels retain their last successful
  read. New Core instances have no timestamps.
- The menu uses Tauri predefined About, Quit, Edit and Window items, with
  the required separators. menu_items reads the installed Menu's labels;
  no duplicate expected menu is returned by the driver. The native S39
  test asserts the labels, including `Close Window`. No differing native
  label could be observed here because the sandbox cannot start the app.
- Main-window drag Enter/Leave/Drop use exported constant class scripts.
  The drop still hands its paths to the page's `wataneDropped`, which
  calls `install_paths` (5.3); the class scripts carry no path.
  (Validator: Sol's Rust-side install with a DOM-coupled error script was
  reverted; the brief said drops still go to install_paths.) No UI,
  command ACL, permission or dependency changed.
- The driver theme op applies light/dark/system to every open window.
  `ac7_s39_w37_quiet_theme_changes_prefers_color_scheme` asserts the actual
  matchMedia result after dark and light and does not skip on failure.
- Red/green: callback adoption failed with no main; the update-plan check
  failed with an incorrect Desk proposal; short-word, timestamp, constant
  script and driver-client tests initially failed on the missing fields,
  functions or methods. The focused unit tests then passed. S51 is slow
  because it builds the maker binary and proves local-API consent and
  restart; it uses the committed page-only Desk fixture, not a mod build.
- Native S51, S39 menu and S39 theme runs each failed before readiness:
  `Connection Invalid error for service com.apple.hiservices-xpcservice`,
  `_LSModifyNotification` -50, then the driver's 30-second startup timeout.
  These failures establish no platform theme result. AC2, AC5, AC7 and
  full AC8 GUI validation remain PARTIAL for the logged-in validator.
  No front test was run, and no assertion was weakened.
- The required full default and driver/include-ignored runs encountered
  the existing pairing test's `Failed to create netmon monitor`. The
  four front tests were explicitly skipped. A diagnostic default run
  excludes only that known pairing failure and passes 148 tests across
  22 suites, including static_config; all tests remain unchanged.
  Debug build, strict all-targets driver clippy, fmt and diff checks pass;
  release with driver fails at the required debug-only compile guard.
- The existing manager test
  `ac5_s81_words_and_focus_are_documented_at_their_facts` reads Markdown
  specs. It was retained unchanged under the instruction not to delete
  or weaken existing tests; no new test reads specification text.
## S84-3 — Library design

The manager now uses the W37 tokens in light and dark, a shared header,
selectable Library tiles, one details panel, grant and start switches,
connection chips, first-start cards, an inline confirmation, the installed
note, and the drag overlay. Settings keeps its ids. Find and Updates keep
their existing content layout; the single search field and refresh id use
5.3.7. S84-4 owns search-word parsing and the remaining Find/Updates design.
No dependency was added. `monogram(mod, size)` is exported from `ui/words.js`.
`modsView` is exported from the production page module so window tests can
render literal distribution-only badge states without a Rust test command.

Changed test facts required by 5.3: the details section is selected by a
tile, rather than opened as a disclosure; connections and dependency warnings
name mods, rather than local slugs; Off replaces Disabled; grants express
whether they are given through switch aria-checked, and their exact words
remain in the grant text's title. Removal keeps data by its checked radio;
Cancel takes focus and a separate confirmation performs either removal.
The main mark is on its tile. Made from is read from Technical details.
The Node renderer harness retains the previous main-mark, connection-focus
and dependency-warning assertions using these controls.

S84-2 was not present: the only Rust addition here is the fixed driver
`theme` operation and its `App::theme` support method. It calls set_theme
on every existing window. Empty or absent GrantLine.short falls back to text.

Validation limits in this sandbox: native hidden window tests cannot start,
with `Connection Invalid error for service com.apple.hiservices-xpcservice`
and a readiness timeout. Consequently native focus, colours (including S50),
drop visibility, and J2/J5/J10 remain unvalidated here. The test failure is
not turned into a skip. A normal test also fails before any UI in
`consent::machines_pairing_tests::ac2_real_pairing_events_wait_for_consent_and_decline_changes_nothing`
with `Failed to create netmon monitor`. The four screen-taking tests are
excluded from the driver run as required by the task.

Validator (round 1) decided the three `gui_shell` conflicts and one in
`gui_isolation` against 5.3, selector or words only: an empty home shows
the visible `#first-start` with `Install your first mod` (was `No mods
yet`); a disabled mod shows `button.enable` and no `button.disable` (was a
disabled `button.disable`; 5.3.4 replaces it) in `gui_shell` ac2 and
`gui_isolation` ac4; the version is `Version 1.2.3` in `h2 + .version`,
outside any `details`, and `details.technical` is closed (5.3.4 item 1, 9).

Checks completed here: the normal build succeeds; static_style, both
non-window look tests, static_config and the four renderer harness cases
pass. Clippy with all targets and driver and fmt check are clean. A release
build with driver fails at the intended compile_error. J2, J5, J10 and all
four new native look tests were explicitly attempted and fail at native
startup, before their page assertions, with the HIServices error above.
Validator native run (outside the sandbox): driver suite with
`--include-ignored` (front four skipped) green after the fixes above, J2,
J5, J10 and the four look window tests included. A first full run hit
`app did not become ready` in j5 `ac2_j5_github`, j8, j9 and look while
another worktree ran window tests; each passes when rerun alone.


## S84-4 — Find and Updates

Find now uses the 5.3.5 search box and pure `searchWords` parser in
`ui/words.js`. It recognises one publisher (including multiword profile
names and case-insensitive fingerprints) and one interface, retaining a
second match as text. The interface grammar follows dist's InterfaceQuery
and pkg's decimal-u64 versions, identifiers and 128-byte qualified limit.
The last parsed query reruns after refresh. Index chips retain fingerprints
and status for assistive reading and in their title; failure causes, includes,
skips and own-index listing controls sit below the chips in `#index-notices`.
Existing listing and skipped-channel tests change their selectors to that
container; no assertion is removed.

Find results and the three Updates sections use the shared card, monogram
and theme tokens. Installed links select their Library entry; another
installed version gets a `You have` tag and may be installed. Compromised
results explain the block and have no action. Not now is a page-memory map
of offer fingerprints; unchanged polls preserve the cards and focus. An
unrelated refresh/install failure does not reshow a dismissed offer.
`renderIndexes`, `renderResults` and `renderUpdates` export the page's own
renderers for the literal dist-state tests allowed by section 11.

Checking disables Check now and retry controls and turns the glyph until
`channels_refresh` returns, independently of the subsequent UI read. The
existing reduced-motion rule disables its animation. Missing S84-2
`checkedAt` and `readAt` are supported: the initial header says `Not checked
yet`, a successful manual check supplies its completion time, and absent
channel read times omit Last read. Empty `GrantLine.short` uses `text`.
No dependencies were added.

Changed fact: 5.3.5 requires `read <ago>` / `not read since <ago>`; 9.6
still described the old checked/local-time wording and now cites 5.3.5.
Validator: `tests/find.rs::ac1_find_search_install` now reads the slug from
the card name's `title` (5.3.5 shows the name, not the slug), `read <ago>`
from `.checked`, and `No mods found` from `#result-count`;
`ac1_find_stale_mark` asserts `not read since <ago>` of the same
`last_success` it compared as local time before. Words and selectors only.
The old `checkedLabel` stays for the consent page.

Validation: the new pure-word tests were run red (missing exports, then
invalid interface-version recognition) and green. The production renderer
harness was run red for absent renderers, publisher matching, poll focus
preservation, dismissal across unrelated failures, and Checking ending at
command completion, then green. Native tests cover both appearances,
real search command arguments, Review opening real consent, and a held
channels_refresh. The sandbox cannot start native app windows: readiness
times out with `Connection Invalid error for service
com.apple.hiservices-xpcservice`. Thus AC1/AC2/AC3 native interaction,
AC5 native appearance/consent and AC6 driver regressions remain PARTIAL.
The full normal suite also encounters the pre-existing
`consent::machines_pairing_tests::ac2_real_pairing_events_wait_for_consent_and_decline_changes_nothing`
netmon startup failure (`Failed to create netmon monitor`); the remaining
normal suite passes with that one test excluded. The four screen-taking
tests are never run.

Final checks: cargo build passes (the existing compact-unwind linker
warning remains); clippy with all targets and driver, cargo fmt --check,
and git diff --check pass. Release plus driver fails at the intended
compile_error. The normal suite excluding the single netmon test passes
150 tests; look/static_config/static_style pass 10 tests; the fixed-now
words test passes with driver. Targeted look, find, build_on, j4 and j5
native runs all stop at HIServices startup, including all three new look
window tests. The full include-ignored driver run (front four excluded)
stops in the unit suite at the same netmon test (109 passed, 1 failed).


### S84-4 validator return, round 1

The native Find test no longer attempts to replace Tauri's immutable
invoke property. Its real dist fixture has Ivy's Reading list (books
interface), Ivy's Notes (no book words), and Alice's Bookmarks (book words,
no books interface). The page starts from all three hits, shows two for
`book`, and only Ivy's release for `book ivy`; it starts from all three
again before checking the interface search. A non-window dist test proves
the fixture's three/two/one/one results independently of the native test.

The held Check now test now follows exactly one real channel and holds
its HTTP response with Site's watch gate, waiting for a request to reach
the hold before inspecting Checking, disabled, spin and `Reading 1
channel…`. Releasing the site lets the actual channels_refresh finish.
A non-window HTTP test proves the response stays pending until release;
Site releases a hold on drop too, so a failed assertion cannot hang its
shutdown. The empty-state snapshot's comment now describes its assertions.

Changed fact: build_on's provider-pick details use section 4's short grant
words, `Call api of Frank ratings by Frank (in place of ratings by Carol)`.
Its assertion now expects `of Frank ratings by Frank` and retains
`in place of ratings by Carol`.

The one authorised unlisted change is
`tests/machines.rs::ac8_settings_navigation_refreshes_hidden_page`: its
route extraction ends before the hashchange listener and stubs
removeAttribute, closeAdd and headerView. All three refresh-count
assertions are unchanged. This test was run red at the overlong extraction
and green after the repair; the HTTP hold test was run red for its missing
helper and green with the real server. The dist fixture test also passes.
No production code, dependency or specification changed in this return.
Native window tests are left to the validator as requested.

Round-1 checks: build, all-target driver clippy, fmt, diff check and
`cargo test --features driver --no-run` pass. The release-driver guard
fails at the intended compile_error. Both new non-window fixture tests,
the repaired machines navigation test, all five non-window look tests and
the fixed-now gui_shell wording test pass. The full normal run stops at
the known netmon monitor failure (111 passed, 1 failed); with that test
excluded, 153 tests pass. That supplemental run first saw an unrelated
support_selftest listener-shutdown failure; its isolated rerun and the
subsequent full supplemental run pass. AC1/AC3/AC5 native interactions and
AC6 native regressions still need the validator's native run.


## S84 — manager design comparison

The manager now uses the boards' spacing, normal control/row line height,
monogram type sizes, and decorative inline SVG icons. Paragraphs retain
1.5 line height; the consent view retains its existing line height and layout.
Settings uses the shared button shapes and spaced controls, with all ids kept.
No dependency, command, capability or subsystem interface was added.

Changed fact: Find's listing words use names only (or the fingerprint when
unnamed). `tests/find.rs` keeps its exact listing and merged-note assertions,
with `Alice` replacing `Alice (abcd)`; publisher assertions still include keys.
Changed fact: a Find hit installed in another version links to Library as
`Installed`, alongside `You have <version>`. The DOM harness assertion changes
from `Install…` to `Installed`; the native snapshot also asserts that the
install control is absent for that hit.
Changed fact: network grant chips use a decorative globe SVG instead of `◎`.
The DOM/native assertions retain the exact grant words and title, and the
DOM harness also checks that the SVG is present.

New `ac5`, `ac6`, `ac11`, and `ac12` renderer scenarios were run red before
implementation: block adds-to text, hidden start row, missing Installed link,
and withdrawal dismissal respectively. They now cover one/two/three installed
names (no adds-to text for several), note placement, the visible start row,
name-only listings with own/unnamed parties, withdrawal without Not now,
and Checking… with no channels. Existing look/find-updates scenarios pass.
Pure CSS values use board review, without changing static_style's token checks.


Validation: `cargo build -p watane-app`, driver all-target clippy with
`-D warnings`, fmt, static_style and static_config pass. Both requested
full test commands stop at the unchanged real-pairing test because this
sandbox cannot create a netmon monitor (111/112 other unit tests pass).
The supplemental normal run excluding only that test passes serially (157 tests);
a concurrent listener-shutdown failure passes both in isolation and in
that serial run. The driver run, with all four front tests excluded and
the netmon test excluded supplementally, reaches the API native test and
cannot start the app: `Connection Invalid` for
`com.apple.hiservices-xpcservice`, then a driver-ready timeout. The explicit
`--features driver --test look -- --include-ignored` run passes all ten
non-window/fixture checks; its seven native checks fail at the same startup
boundary. AC15's full native validation remains PARTIAL for the validator;
the manager's visual changes still need its native review. No test was
weakened or skipped in source. Release with driver fails at the intended
`the driver feature is only for debug test builds` compile_error.


## S84 — second comparison spacing and button polish

Details now use a 26 px flex-column gap, with the board's -10 px summary
margin after the heading and no extra action, confirmation or section margins.
The existing important hidden rule still removes unselected details and hidden
controls from layout. The Updates count is a centred 18 px pill; plain link
buttons use ink, and grant rows use line-height 1.4.

Settings primary actions are Set name, Set site URL, Save backup, Issue token
and Join. Save toolset and Confirm identity use plain buttons; all ids remain.
Empty Settings paragraphs and containers occupy no space, and Site no longer
adds a bottom margin inside the distribution wrapper. No behaviour, test,
dependency or interface changed. Rechecked look.rs and manager_dom.mjs: neither
counts Settings primaries.

Validation: build, driver all-target clippy with -D warnings, fmt and
static_style pass. Release with driver fails at the intended debug-only
compile_error. The required full normal test run passes 111 unit tests and
fails the unchanged real-pairing test because the sandbox cannot create a
netmon monitor. The required driver look run passes ten non-window checks;
all seven native checks fail before readiness with macOS
com.apple.hiservices-xpcservice Connection Invalid. Native visual review
remains outstanding in a GUI session. No test was weakened.

Supplemental serial normal run excluding only the netmon test: 157 passed,
1 filtered out. The final normal look run passes all eight tests.


### S84 — details head follow-up

Applied the PO's updated TECHNICAL 5.3.4 item 1: only the tile draws the
Off/Problem/Update status badge; the details retain their notice and local
publisher marker. Removed the duplicate drawBadge call in the head.
Searched look.rs, manager_dom.mjs, j2.rs, gui_isolation.rs and manager.rs:
only j2's disabled-entry assertion still expected Off inside details; it now
checks the tile badge while retaining its enable/open checks. Extended the
existing DOM scenario to check no b-off in details and the unchanged notice;
it failed on the duplicate badge before the renderer change and passes after.
The test files are additional to the original slice list because the updated
acceptance location needs regression coverage. No spec or dependency changed.

Follow-up validation: build, driver all-target clippy with -D warnings, fmt,
all eight normal look checks and static_style pass. Full normal tests again
stop at the netmon failure (111 unit tests pass). Driver look: ten pass, seven
native tests fail before readiness with the same macOS hiservices error.
All four J2 native tests also fail at that startup boundary, including the
updated ac7 assertion; no assertion is reached. Native suites were limited
to look and the changed j2 file.


### S84 — versioned Library update band

The details band now names the offered version and its grant requests, using
short words (falling back to full text), lowercasing the first letter of each
phrase, and joining one, two or more requests with commas and “and”. Offers
without new grants say `Version <v> is available.` Notices and errors retain
their own text. The offerLabel button retains its words and has intrinsic,
non-growing flex sizing at the right; the row still wraps on narrow panels.
No interface, dependency or spec changed.

Test-first: the new look.rs renderer check failed on `An update is available.`;
the sizing check failed on missing `flex: 0 0 auto`. Both pass after the change.
The look DOM scenario also covers both required 0.4.0 texts, unchanged
button labels, fallback words, two/three requests, and notice/error precedence.
All ten default look checks pass. Driver all-target clippy with -D warnings
and cargo fmt --check pass; git diff --check is clean.

The one required default `cargo test -p watane-app` run stops in the existing
real-pairing unit test: `Failed to create netmon monitor` in the sandbox;
111 unit tests pass. Per this task's execution instructions no driver/window
test was run. The external GUI runner still needs to run only driver look
with --include-ignored; native layout validation remains pending.


### S84 — short details offer button

Only the Library details band now labels its offer `Install <v>…` with new
grants, otherwise `Install <v>`. The band still names the grant requests;
release_install receives the same offer. Shared offerLabel and Updates are
unchanged. No dependency, interface or spec changed.

Test-first: ac1_ac2_update_band_version_grants_and_short_button failed on the
old long label, then passed with the short label. The update-band scenario
checks zero/one/multiple grants and the release_install argument. The native
layout fixture now has one grant and expects the ellipsis, retaining the
default-size same-row/right-edge and narrow wrapping checks.

Validation: targeted look test and find-updates DOM scenario pass; driver
all-target clippy with -D warnings and cargo fmt --check pass. The single
required cargo test -p watane-app run stops at the existing pairing test
(Failed to create netmon monitor); 111 unit tests pass. No window test ran,
as required by the task execution instructions. External validation must run
only driver look with --include-ignored for native layout coverage.


### S85 — separate mod context key on Linux

`open_mod` keeps `incognito(true)` and supplies the fixed context key
`<home>/app/mod-webview-context`. Main and consent keep the default key.
This prevents tauri-runtime-wry 2.12.3's once-per-key Linux protocol
registration for an ephemeral mod from consuming the main window's
`tauri://` handler. The builder comment records why the key is necessary.
The Linux image also installs `lsof`, used by machines S28. No Rust
dependency was added; round 1 changed no tests. The builder options live in TECHNICAL 5.1,
not section 10 (which describes the driver); that owning list is updated.

Linux filesystem check: a throwaway driver probe installed Desk, quit,
restarted into mod-desk, opened Mods from the menu, observed
`tauri://localhost/index.html#mods`, quit, and ran
`ls -la /tmp/s85-context-home/app/mod-webview-context`. The listing was:

```text
WebKitCache/
hsts-storage.sqlite  (12288 bytes)
mediakeys/
storage/
```

Thus AC1's expectation that nothing writes the key path is not true of
this dependency stack (WebKitGTK 2.54.0, GLib 2.84.4, libsoup 3.6.5).
Tauri constructs `WryWebContext::new(Some(path))` before Wry replaces it
with `new_ephemeral()` for an incognito view (`webkitgtk/mod.rs:259`).
The unused bookkeeping context creates these files; the mod's actual
view still uses the ephemeral context. The unchanged Linux regression
`gui_isolation::ac3_reopened_hostile_window_has_fresh_storage` also passes. No cleanup or change to the
isolation boundary was added to disguise this observation. Round 2 D1
accepts these unused-context bookkeeping files under amended AC1; mods
never use that context.

Validation: all four requested targeted Linux runner commands pass:
manager::ac3_s39_menu_and_last_window_exit,
manage::ac2_s44_disable_closes_page_and_restart_clears_notice,
j10::ac3_journey_j10, machines::ac1_s28_off_by_default_and_publish.
Buildx's default activity-file location is outside the sandbox; commands
use `BUILDX_CONFIG=/tmp/watane-s85-buildx` without changing the runner.
The initial manager run began before the edit but compiled against the
live bind mount; it passed, so it is not claimed as a reproduced red run.
The foreman's supplied failing Linux observations are the regression
baseline. No new source-shape test was added (AC5).

The single native `cargo test -p watane-app` run has 111 unit tests pass
and the existing real-pairing test fail with `Failed to create netmon
monitor`, as in S83/S84. Native driver/window tests were not run, per
the task's sandbox instructions; manager, manage and j10 still require
the external macOS GUI runner. Native all-target driver Clippy with
`-D warnings` stops in unchanged sibling code: disallowed
`PoisonError::into_inner` in log/src/lib.rs:169 and `catch_unwind` in
pkg/src/component.rs:59 and pkg/src/world.rs:191. These were not changed
or suppressed. `cargo fmt --check` passes (stable rustfmt warns about
existing nightly-only configuration options).

The one full Linux runner invocation exits 101: the default suite passes,
then the driver suite stops at
`look::windows::ac3_ac4_ac6_ac8_library_details_confirm_install_and_badges`.
Seventeen look tests pass; that test times out on the existing details-offer
geometry assertion (right edge within 1 px, short button under half the row
width, and the text and button overlapping vertically). Its expected text
is `Version 2.0.0 asks to connect to example.org.` and the button is
`Install 2.0.0…`. No assertion, skip, platform branch or layout code was
changed in round 1. Round 2 D3 addresses this failure as described below.

Native macOS: the one `cargo test -p watane-app` run reports 111 passing
unit tests and one failure in
`consent::machines_pairing_tests::ac2_real_pairing_events_wait_for_consent_and_decline_changes_nothing`:
`Failed to create netmon monitor`. The requested all-target driver clippy
command fails in sibling code on disallowed methods: `PoisonError::into_inner`
in log/src/lib.rs:169 and `catch_unwind` in pkg/src/component.rs:59 and
pkg/src/world.rs:191. Those siblings were not edited. `cargo fmt --check`
passes (stable rustfmt warns about existing nightly-only settings). Native
manager/manage/j10 GUI tests were not run, per the task's sandbox execution
instructions; they still require external macOS validation.


#### S85 round 2 — foreman decisions

D1 accepts the observed bookkeeping files above: they belong to Tauri's
unused context, while the mod view remains ephemeral. D2 confirms that
the newer-Clippy failures in log/pkg are pre-existing on main; those
siblings and lint settings remain unchanged.

D3 changes only the constructor of
`look::windows::ac3_ac4_ac6_ac8_library_details_confirm_install_and_badges`
from `App::start` to `App::background` on every platform. Hidden GTK
windows are never mapped and have `innerWidth === 0` (the foreman's probe
measured the notice at left -130 with width 280). The test asserts layout
at the default window size, so it requires a mapped background window,
as permitted by TECHNICAL 11. Every assertion remains unchanged; no CSS,
platform branch, ignore attribute or skip was added. The round 1 failing
Linux run above is the red regression evidence for this constructor fix.

Round 2 validation: the targeted Linux look test passes with the background
constructor. Native `cargo test -p watane-app` again reports 111 passing
unit tests and only the known netmon pairing failure. The requested
`cargo test -p watane-app --features driver --test look -- --include-ignored`
was attempted: 11 tests pass, while seven window tests (including the
changed one) time out before app readiness. Their stderr reports
`Connection Invalid error for service com.apple.hiservices-xpcservice`
and notification tracker error -50. Native GUI validation is unavailable
in this sandbox and needs an external macOS GUI session. No test was
filtered or weakened in response. Round 2 `cargo fmt --check` passes.

The first round 2 full Linux run passes all 18 look tests, then reveals
a later existing manager failure:
`ac5_s39_w37_menu_items_and_separators` receives GTK predefined menu labels
`Cu&t`, `&Copy`, `&Paste`, `Select &All`, `&Minimize`, `C&lose Window`,
which differ from the literal labels specified in TECHNICAL 5.4 and asserted
by the test. `views::install_menu` now supplies those six labels explicitly
through Tauri's predefined-item builder methods. Their native actions and
accelerators remain predefined; the test and its assertion stay unchanged.
This run is the red evidence for the additional Linux suite fix.

After explicit menu labels, a full Linux run exits 0. The next full run
passes look but fails manager's existing
`ac3_s81_used_by_link_opens_details_and_moves_focus` waiting for target
heading focus. Source inspection shows the setup race: `install` waits for
mod panels, which `refresh()` draws before it finishes its other awaited
work; the install action then selects the newly installed mod after the
test may already have clicked its connection. That later selection can
overwrite the connection selection and focus. The targeted test passes
alone, consistent with timing-dependent setup.

Unlisted file `tests/manager.rs`: this one test now waits for the existing
installed notice (set after the install action selects its new mod) before
clicking the connection. It retains hidden mode and its exact assertion;
there is no fixed delay, retry of the interaction, skip or platform branch.
The targeted Linux runner passes after adding this prerequisite wait.

Final round 2 validation: two consecutive complete Linux runner invocations
on the same code both exit 0, with zero failures in the default and driver
suites (`/tmp/watane-s85-r2-linux-final-1.log` and
`/tmp/watane-s85-r2-linux-final-2.log`). The runner retains its established
front-test skips; no skip was added. The original four S85 regressions,
fresh-storage regression, mapped look test, explicit menu-label test, and
synchronized connection-focus test all pass in both full runs.

Native macOS `cargo test -p watane-app`, rerun after the menu fix, reports
111 passing unit tests and only the known sandbox netmon pairing failure.
`cargo test --features driver --test look -- --include-ignored` was attempted:
11 tests pass, seven window tests time out before app readiness, including
the changed background test. App stderr reports invalid connections to
`com.apple.hiservices-xpcservice` and `_LSModifyNotification` error -50.
Native window validation is unavailable in this sandbox and still needs
an external GUI session. No test was skipped to hide these failures.
`cargo fmt --check` and `git diff --check` pass. D2's pre-existing sibling
Clippy failures remain unchanged. No dependency was added.
