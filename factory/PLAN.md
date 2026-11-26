# Plan

The product owner's state; only the product owner edits it, on `main`. A new session continues
from here. Newest digest line last.

## Slices

Status: `[ ]` sketched, `[~]` in flight (with its branch), `[x]` delivered.

Step 1 (Make, J1). A first cut; the product owner refines each sketch before it hands it out.

- [x] **S1 Hello page.** Delivered 2026-11-10 (main `bacecdc`). Sol runs: pkg 1, host 1, dev 2.
- [x] **S2 The page calls its logic.** Delivered 2026-11-10 (main `7eb55ad`). Sol runs: pkg 1, host 2, dev 2.
- [x] **S3 An edit keeps the data.** Delivered 2026-11-10 (main `c883171`). Sol runs: host 1, dev 1.
- [x] **S4 The agent fixes a failing test.** Delivered 2026-11-11 (main `91e8d0a`). Sol runs: S4-1 1, S4-2 1, S4-3 1.
- [x] **S5 The agent works the mod through MCP.** Delivered 2026-11-11 (main `196452e`). Sol runs: S5-1 1, S5-2 1.
- [x] **S8 Same mod, same id.** Delivered 2026-11-11 (main `d79e73e`). Sol runs: S8-1 1, S8-2 1.
- [x] **S6 J1: Alice gets her reading list.** Delivered 2026-11-11 (main `856b431`). Sol runs: S6-1 1, S6-2 1.

Step 1 done 2026-11-11: J1 passes (`dev/tests/it/journey.rs`); every step-1 stage is done or its rest
is in step 2.

- [x] **S7 Pack a mod.** Delivered 2026-11-10 (main `a740d78`). Sol runs: pkg 1.
  Its follow-ups are done in S15.

Step 2 (Run together, J2).

- [x] **S10 Alice opens the app.** Delivered 2026-11-11 (main `da54bb4`). Sol runs: S10-1 1, S10-2 1, S10-3 1, S10-4 1.
- [x] **S11 The recommendations mod calls the reading list.** Delivered 2026-11-11 (main `e6dc87e`). Sol runs: S11-1 1, S11-2 3, S11-3 1, S11-4 2, S11-fix1 1.
  Left for later (user-sim): `mod.dependency_cycle` and `mod.self_dependency` have `fix: null`;
  `check.providers[].path` is null for a local provider; `add --dir` resolves the provider path
  from `--dir`; a subscriber's `/events` also gets the provider's raw `event` messages. Host
  stage 5 (`Host::plan`, `Source`, confirmation) and S25 (revoke) are not built: S12.
- [x] **S13 Cheap tests.** Delivered 2026-11-11 (main `cf35f70`). Sol runs: S13-1 1.
  dev's default `cargo test` takes about 1.5 min (was about 9); the full run with slow tests about
  10 min (was about 30). `dev/TECHNICAL.md` 15.2 maps each DEV-S scenario to its test and level.
- [x] **S14 Quiet GUI tests.** Delivered 2026-11-11 (main `dca54b6`). Sol runs: S14-1 1.
- [x] **S15 Packages say what is wrong.** Delivered 2026-11-11 (main `3c441d7`). Sol runs: S15-1 2 (new, resume).
  Follow-ups: a link inside a hidden folder (`ui/.git/x`) is still a `[path]` rule in `Mod::read`;
  `O_NOFOLLOW` guards only the last path part; `folder.rs` opens with plain `File::open` after
  `symlink_metadata` (a narrow swap window; packed bytes stay safe).
- [x] **S16 J1 with Codex.** Delivered 2026-11-11 (main `34c6cc4`). Sol runs: none. Codex made the
  mod alone (379 s, 31 commands, 6 failed, about 9 source reads); MCP calls needed one approval line.
  Log: `factory/harness-codex.md`; raw runs in `factory/runs/s16-bob/` (untracked).
- [x] **S17 The simplest tool to start with.** Delivered 2026-11-11 (main `786db15`). Sol runs: S17-1 1, S17-2 1, S17-fix1 1.
  Against S16, Codex / Claude: commands 38 → 27 / 14, failed 6 → 3 / 2, source reads about 9 → 5 / 1,
  guesses 3 → 1 / 1. Log and the smaller friction left: `factory/harness-s17.md` (no word on
  `watane.on` in the guide, a silent first build of about 25 s, `calls: ` for an empty list, serve
  `reused:false` does not say to update the MCP config, doctor in a mod folder suggests `new`,
  check on `mod.invalid` has `next` null; WIT syntax errors keep `line: null`: pkg's message has no
  location).
- [x] **S12a J2: Alice runs her mods together in the app.** Delivered 2026-11-11 (main `37b2a9e`). Sol runs: S12a-1 1, S12a-2 1, S12a-fix1 2.
  Left for later (user-sim, validator): the app's toolset view names a tool
  `reading-list:books.list`, but MCP names it `reading-list_books_list` (APP-F-25; cheap app fix);
  with a provider disabled, the recommendations fixture page shows "not granted" and hides the
  host's message; stage C must put the lock on `Parties` (now a BTreeMap alias); pages get no
  grant-change message, so the app reloads the open window; a `/call` whose client disconnects
  keeps an `Arc<Host>` past shutdown; host `ac4_program_defaults_to_environment_home_and_port`
  probes port 47341 and then binds it, a race with a running watane.
- [x] **S18 Alice runs the app on Linux.** Delivered 2026-11-11 (main `e7d9b5d`). Sol runs: S18-1 1, S18-2 1.
  Left for later (user-sim): the runner prints versions without names; about 17 AT-SPI warnings
  bury the output (`NO_AT_BRIDGE=1` in the image); the app's JS repeats the tool-name rule for
  saved entries.
- [x] **S12b The agent installs a mod into Alice's home.** Delivered 2026-11-12 (main `0c58053`). Sol runs: S12b-1 2 (new, hung; resume), S12b-2 1, S12b-3 1, S12b-fix1 1, S12b-fix2 1.
  The user-sim ran `install` against `fake-watane` and the real `watane --home`, not the app; the
  product owner ran app j2 (hidden) after the merge: green.
  Left for later (user-sim): `fake-watane --help` prints an error; the fake has no never-answer
  mode; the decline message is just "declined"; `--wait` does not count the test pipeline;
  the no-app result's `dir` is as typed while `next` is canonical; `add . --dir x` says ". is not a
  mod folder"; host `install::ac2_real_home_without_app` leaves `watane` running if it panics.

Step 2 done 2026-11-12: J2 passes (`app/tests/it/j2.rs`); every step-2 stage is done.

Step 3 (Publish, J3).

- [x] **S19 Alice's app holds her identity.** Delivered 2026-11-12 (main `48a9b2f`). Sol runs: S19-1 1, S19-2 2 (new, resume).
  Left for later (user-sim): `Log::open` on a file says "is not a regular file or directory" (should
  say "not a directory"); `Log::open("")` gives ": empty path"; "bad backup: length" does not say
  108 bytes are expected.
- [x] **S20 A mod becomes a release.** Delivered 2026-11-12 (main `0349cf0`). Sol runs: S20-1 2 (new, resume).
  Left for later (user-sim): the on-disk fixtures `reading-list` and `recommendations` cannot be
  packed before `logic.wasm` is built; the on-disk `hello` has `ui/app.js` (213 bytes), so its id
  differs from 7.3's 159-byte golden; the empty-input error repeats itself ("bad release body:
  release: end of input bytes").
- [x] **S22 Packages are safe on every path.** Delivered 2026-11-12 (main `0d686a3`). Sol runs: S22-1 1.
  Left for later (user-sim): `decode(&[0x8c])` reads "format: end of input bytes"; a FIFO at
  `ui/extra` passes `Mod::read` but `pack` refuses it; for `wit/.hid/k`, `Mod::read` names the WIT
  folder rule, not the link; release errors strip "release" in `malformed()`, not at the call site.
- [x] **S21a Alice publishes, Bob follows (dist).** Delivered 2026-11-12 (main `7fad99d`). Sol runs: S21a-1 1, S21a-2 2 (new, resume), S21a-3 1.
- [x] **S23 The maker's agent stumbles less.** Delivered 2026-11-12 (main `3a1bbdd`). Sol runs:
  S23-1 1, S23-2 1, S23-3 1, S23-fix1 1, S23-fix2 1. Left for later (user-sim): the progress line
  prints twice and says "first build" on warm builds; `serve`'s `next` ends the URL with `.` and names
  JSON paths in text mode; `check` says to correct the do-not-edit deps.json for an unused provider;
  text mode prints object lists on one line and an argument error as `: failed`. The template guide
  is at its 143-line cap: the next guide line raises the cap or cuts a line.
- [x] **S21b J3 in the app.** Delivered 2026-11-12 (main `cf65328`). Sol runs: S21b-1 1, S21b-2 1,
  S21b-3 1; S21b-fix1 by the validator (two intermittent window tests). Left for later: dist cause
  text prints twice in the app ("X: X": `DistError::Log` is `#[error("{0}")]` with `#[from]`, `Io`
  prints its source too; app's ac4 test expects the doubled text); the 422 message shows Debug
  text; URLs show in Debug quotes.
- [x] **S21c The agent publishes.** Delivered 2026-11-12 (main `355656d`). Sol runs: S21c-1 1, S21c-2 1,
  S21c-3 1 (one validator round-2 fix).

Step 3 done 2026-11-12: J3 passes (`app/tests/it/j3.rs`); every step-3 stage is done.

Between steps 3 and 4.
- [x] **S24 Publish and install edges.** Delivered 2026-11-12 (main `f6b40d9`). Sol runs: S24-1 1,
  S24-2 1, S24-3 1. The `install_link` question was no security gap: the host asks consent for every
  new mod (`Change::New`); for a followed channel dist ignores the link's site URL and refreshes from
  the stored one, so the already-installed mod came back unchanged.
- [x] **S30 The agent's publish texts read right.** Delivered 2026-11-12 (main `1a2097b`). Sol runs:
  S30-1 1. Left for later (user-sim): once the provider is published, publishing the consumer still
  refuses `local_dependency`, and no command depends on a release (step 5, `add --release`); its
  message still says "it has no release"; the self-dependency fix still offers "Import the
  provider's interface"; the `version_exists` rerun reports `dir` unresolved (/tmp vs /private/tmp);
  a fake-app publish took about 30 s, close to `--wait 30`.
- [x] **S25 Carol stays up to date.** Delivered 2026-11-12 (main `ff754cb`). Sol runs: S25-1 1. Refresh
  updates with no new grant quietly, with a new grant through consent; Alice withdraws with a reason
  and Carol sees it with an offer of the newest live version; a fork shows as compromised.
- [x] **S26 Design: indexes and search (step 4).** Merged 2026-11-12 (main `b5b58c9`), spec documents
  only, after one independent review (5 security issues, about 10 gaps, all fixed). Indexes are
  three new statement kinds (`list`, `include`, `unlist`); includes one level deep; search is local
  and offline; no shipped index. Open: Q12, log entries of unlisted channels are never removed.
- [x] **S28 Carol's dist finds Alice's mod (dist S6).** Delivered 2026-11-12 (main `9e4752e`). Sol runs:
  S28-1 1, S28-2 2 (new, resume), S28-3 1. Through dist's API: Carol chooses Ivy's index, searches
  "reading", and gets Alice's newest live release, verified from Alice's channel, listed by Ivy.
- [x] **S29 Carol finds Alice's mod in her app (J4).** Delivered 2026-11-13 (main `6ea4e5f`). Sol runs:
  S29-1 1, S29-2 1, S29-3 1, S29-fix1 2 (new, resume). User-sim rejected once (the own index row
  said "0 mods"), then accepted. Front tests not run (screen locked). Left for later (user-sim): own
  rows and consent details show raw 52-character keys; AlreadyListed says "already listed at seq 1"
  for an include; own-index hits read "listed by <own fp>", not "you"; an empty search shows no "No
  mods found"; the own row sits next to the "add an index" hint on a new home; index read failures
  are raw transport errors that do not name the index; "listed by Ivy" and "listed by Ivy through
  Alice" show together on one hit.

Step 4 done 2026-11-13: J4 passes (`app/tests/it/j4.rs`); every step-4 stage is done.
- [x] **S31 The agent searches (dev stage 8).** Delivered 2026-11-13 (main `956de83`). Sol runs:
  S31-1 1, S31-2 1. Tested against `fake-watane`; the real `/api/search` is S29's. Left for later
  (user-sim): 503 and other app errors get the generic "Correct the reported host error" fix;
  `search --help` lists the global `--dir`, which search ignores; a hand-made `wit/deps/<own slug>/`
  hides the own-provider message behind a WIT duplicate-package error; the default home in `next`
  shows `/tmp` where an explicit `--home` shows `/private/tmp`.
- [x] **S33 The agent's texts, second pass.** Delivered 2026-11-13 (main `8f2ca97`). Sol runs: S33-1 1,
  S33-2 1, S33-fix1 1. User-sim rejected once (an own-package copy at another version), then
  accepted. Spec debt: pkg TECHNICAL 4.7 lists two provider messages, pkg has a third; dev
  TECHNICAL 2.4 and 9.6 say "canonical or given home", the code uses the resolved home. Left for
  later (user-sim): an own-package copy under another folder name gets "repeated package"; a
  deps.json-only self entry names a wit/deps folder that does not exist; app 5xx is coded
  `host.refused`; after a failed test or serve `next` just reruns; `--home x/../h` with a missing x
  gives an internal error with no path.
- [x] **S32 Design: build on (step 5).** Merged 2026-11-13 (main `bf5649f`), spec documents only,
  after one independent review (18 findings: 6 security, 10 gaps, 2 simplifications; all fixed). No
  published format changes: `[origin]` and `Release.origin` exist since step 3, provider hints reuse
  `list` and `include`, binds live in `installed.json` with a serde default. Origin is the direct
  parent, the publisher's claim (W24); provider bases come from live list/include statements the home
  already reads (W25, closes dist Q2); a fit is the same slug with every used interface satisfied,
  any publisher, bound only after the person picks (W26); derived mods are a search filter (W28).
- [x] **S34 Eve's host binds a mod that fits (host stage 9).** Delivered 2026-11-13 (main `69a49ae`).
  Sol runs: S34-1 1, S34-fix1 1. User-sim rejected once (removing a bound provider in a preview),
  then accepted. Left for later: in a preview, ambiguous_provider names neither the competing mods
  nor that a pick settles it (S37).
- [x] **S35 Eve's dist finds Carol's release through Dave's listing (dist S7).** Delivered 2026-11-13
  (main `15d5942`). Sol runs: S35-1 1, S35-fix1 1. User-sim rejected once (a cache hit gave a dead
  base as `Fetched.link`), then accepted. Left for later: NotFollowed for a listing skipped as a
  local address says "no listing names a base"; the no-hint error gives no fix; every hint base is
  read even after a trusted one served; on a no-record cache path the link is the first base that
  served, not the first in trust order.
- [x] **S36 The agent builds on a release (dev stage 9).** Delivered 2026-11-13 (main `aec72d4`). Sol runs:
  S36-1 1 (about 67 min). User-sim accepted on the first run. A maker's agent runs `watane-dev new
  [<slug>] --from <link>` and `add --release <link|ReleaseRef>` through the app's `/api/fetch`; it
  tests offline, serves with the released provider installed first, and publish warns
  `publish.provider_unlisted`. Left for later (user-sim, validator): `new --from` keeps the origin's
  `name` in mod.toml; it copies the publisher's built logic.wasm; an unknown link's fix says "check
  the app"; a Watane-Release mismatch is `internal`; install's missing_providers fix omits the stored
  link; fake-watane exits when a fetch package file is missing; `store` unpacks straight into
  providers/<id>/, so a crash leaves a partial folder that is later reused.
- [x] **S37 Eve installs Dave's mod; Carol's comes first (app E, J5).** Delivered 2026-11-13 (main
  `d44c0d5`). Sol runs: S37-1 1, S37-2 1, S37-3 1, S37-4 2 (new, resume). User-sim accepted on the
  first run. Eve installs Dave's `stats` from his link; the app first installs Carol's `ratings`,
  found through Dave's listing, and follows Carol; if Carol's mod cannot be fetched, Eve picks one
  that fits (Frank's) and the install binds it. "Made from" shows in the signature, consent and
  details; Dave's publish lists Carol's `ratings`. Front tests not run (screen in use). Left for
  later (user-sim): `/api/fetch` with both or neither field answers serde text; `{"release"}` gives
  200 from dist's cache in a home with no record once a fetch by link cached it; "Made from this"
  shows Install for an installed mod; publishers not followed show as fingerprints only (pick, bind,
  grant, Made from, list consent); GET `/api/fetch` answers 405 with an empty body; host's preview
  `ambiguous_provider` text (S34 leftover) still open.
- [x] **S39 Eve's agent recovers from every error alone (dev cleanup).** Delivered 2026-11-13 (main
  `3458939`). Sol runs: S39-1 1 (64 min), S39-2 1 (48 min). User-sim accepted on the first run;
  the product owner rebased it over the step-6 spec commit. Crash-safe provider cache (unpack to a
  temp folder, rename), fixes that name the real action, canonical paths, `new --from` makes the mod
  its own, fake-watane and text mode hardened. Left for later (user-sim): text mode doubles
  "error" and prints all help on no args; after a failed serve the error's fix says `test --json`
  while `next` says `serve --json`; `add --release <own release>` fix says "Pass the folder of
  another mod"; `--home x/../h` fix still says "Start the watane app"; some `next`s are prose, not
  a rerun; a deps.json-only self entry's fix names a wit/deps folder that does not exist; killed
  runs' `<id>.<pid>.tmp` folders are never cleaned.
- [x] **S38 Network tests hold under load.** Delivered 2026-11-13 (main `0c9018a`). Sol runs: S38-1 1.
  `fast_options()` uses 10 s entry and package timeouts; only `fetch.rs` ac9 keeps a 300 ms package
  timeout. 5 of 5 parallel runs passed at load average 19 to 38. Left for later: ac9's 300 ms also
  covers its TooLarge step (a separate short-timeout Dist for the stall step would close it).
- [x] **S27 The app says what happened.** Delivered 2026-11-12 (main `292cd7f`). Sol runs: S27-1 2 (new,
  resume). Left for later (user-sim): a fast retry can leave a stale error (`action()` and
  `button()` both call showError); the busy-port window still shows "Install folders" with no
  working local API; hidden windows poll only on focus; consent requests from clients that stopped
  waiting pile up (S24).

Step 5 done 2026-11-13: J5 passes (`app/tests/it/j5.rs`); every step-5 stage is done. J5 broke at S39 (main `3458939`, `new --from`) and passes again since S47.

- [x] **S44 Carol reads names, not keys (app cleanup).** Delivered 2026-11-13 (main `95db4b6`). Sol
  runs: S44-1 1 (about 26 min). User-sim accepted on the first run. Publishers show by profile name,
  own rows as "you" (also unfollowed channels); errors read as one plain sentence; wrong methods on
  `/api/*` answer 405 JSON; stale errors cleared. Front tests not run. J5 fails on main since S39
  (S47). Left for later (user-sim): no busy state during a 30 s hanging follow or add-index; list,
  include, publish and install consents still show the 52-character key before the name; a
  wrong-channel follow error shows fingerprints only; a mod card's refresh error is raw transport
  text; a collapsed via line drops the include's note; a non-JSON body gets "give link or
  release"; an unknown /api route answers 404 `bad_request`; `Core::mods()` walks channel views on
  every 2 s refresh.
- [x] **S45 Eve's host and dist say what to do (host, dist cleanup).** Delivered 2026-11-13 from the
  cloud (main `a075851`, rebased by the product owner); the 67 host tests the cloud cannot run (no
  IPv6 loopback) and an app build ran on macOS: green, except a flake that predates S45 (below).
  Ambiguous and unreachable providers name the mods and what settles it (a pick; following the
  publisher's link); dist reads hint bases in trust order and stops at the first that serves.
  Spec debt: dist TECHNICAL 3.1 and 5.11 step 4, host 2.2 show the old variant shapes. Left for
  later: unfollowing a never-followed channel says "follow the publisher's link"; a stale first
  base gives "unknown release" with no step; host shows full keys where dist shows fingerprints;
  candidates sort as plain strings (shelf-10 before shelf-2).
  Flake found at the merge: host `api::ac5_extra_api_receives_one_mib...` (in the `preview` target)
  fails about 1 in 3 at a `send().unwrap()`: the guard answers 403/421/404/413 without reading the
  1 MiB body and closes the connection, so the client sees a broken pipe.
- [x] **S46 A hostile mod folder is named, not half-read (pkg cleanup).** Delivered 2026-11-13 from
  the cloud in 13 min (main `fd7fbfb`); pkg, dev and timing tests rerun on macOS: green. A link
  anywhere below ui/ or wit/ (hidden folders too), a FIFO, or broken release bytes get one precise
  refusal and nothing below is read; every walk lists and stats through no-follow directory
  handles (20,000 swaps leaked nothing). Its spec lines are in pkg TECHNICAL 4.7, 6.4 and 7.2. Left for later: a linked `ui` reports two problems;
  `add.rs` (add_provider) keeps the check-then-read gap; mid-swap errors are plain I/O text.
- [x] **S48 Dist and host leftovers.** Delivered 2026-11-13 from the cloud (main `1c6f944`);
  host's full suite and an app build ran on macOS, and `api::ac5_extra` 20 times: 0 failures, the
  flake is gone. Unfollowing a channel that is not followed says so; a stale first base makes dist
  read the next one; host names publishers by the same 10-character fingerprint as dist and sorts
  candidates naturally. S45's spec debt (dist 3.1, 5.11; host 2.2, 18) is paid.
- [x] **S47 J5 passes again (dev fix).** Delivered 2026-11-13 (main `e2d1f55`); dev's full suite
  with `--include-ignored` and J5 rerun on the Mac: green, the timing test included. User-sim
  accepted on the third run. `new --from` keeps the package's `logic.wasm` and drops it only when
  the package has `logic/Cargo.toml`; a mod with `logic.wasm` and no logic source is *prebuilt*,
  and `test`, `serve` and `publish` skip the build and unit tests and run the contract tests
  against the wasm. Under a new slug a prebuilt mod cannot pass `test` (its wasm implements the
  origin's WIT package), so `new` warns `new.prebuilt_renamed`. DEV-S-41 to 51 are marked
  "planned: S42/S43"; the scenario map now covers DEV-S-1 to 51. Left for later (user-sim): `next`
  after `new --from` on a prebuilt package still points at `logic/src/lib.rs` and "then run test",
  contradicting the warning; publishing a prebuilt mod with no `tests/` gives a release that
  `new --from` refuses with a misleading `--no-source` message; the app fixture
  `reading-list/tests/books.json` uses the old contract form; `test` and `serve` refuse `--home`;
  `build.reads_outside` misleads when the cause is a `wit_bindgen` path left by the rename.
- [x] **S40 Design: share on GitHub (step 6).** Merged 2026-11-13 (main `6f3a2c9`), spec documents
  only, after two independent reviews (the second on the human's switch to real GitHub forks).
  pkg stage 8 (`repository`, pkg 4.2); dev stage 10 (`publish --push`, 10.4.1) and stage 11 (the
  `new --from` clone, 10.4.2); DEV-F-43 to 47, DEV-S-41 to 51; tests use local bare remotes, git's
  `insteadOf` and `fake-gh`. Tags are the ReleaseRef `<key>/<slug>@<version>`.
- [x] **S41 Alice's mod names its repository (pkg stage 8).** Delivered 2026-11-13 from the cloud
  (main `86478b9`); dev's real-host slow tests and pkg rerun on macOS: green (the scenario-map
  failure is S47's). `repository` in mod.toml is parsed, validated with pkg 4.2's messages, carried
  in the package; `watane-dev check` gives dev 2.2's fix. A non-string value gets the "must start
  with https://" message. Left for later: the fix could fill in the known slug.
- [x] **S42 Alice publishes to GitHub with one command (dev stage 10).** Delivered 2026-11-13
  (main `1aa2763`); dev's full suite with `--include-ignored` and J3 and J5 rerun on the Mac:
  green. User-sim accepted on the first run. `watane-dev publish --push`: the first run creates
  `<owner>/watane-channel` and `<owner>/<slug>`, writes `repository`, and its `next` asks for the
  raw site URL; later runs commit, tag the ReleaseRef and push. `app/tests/it/j3.rs::ac2_j3_github`
  passes against local bare remotes and `fake-gh`: Bob installs from a served clone of the pushed
  site and sees Alice as verified. Left for later (user-sim): `env.git_identity` keeps git's last
  five stderr lines, which cuts a hint mid-sentence and leaks user@host into the JSON (the last
  line alone would do); `push_failed` doubles "remote: remote:" and keeps trailing whitespace; its
  fix and `next` carry the `-c http.lowSpeed*` flags, so they are noisy to run by hand; every
  `next` appends `--home <abs>` even when the home came from `WATANE_HOME`.
- [x] **S49 The refusals say what is wrong (pkg, log cleanup).** Delivered 2026-11-13 from the
  cloud (main `aa7b66b`, rebased by the product owner after S42); pkg, log and dist with
  `--include-ignored` rerun on the Mac: green. User-sim accepted on the first run. A mod part that
  is a symbolic link (`mod.toml`, `deps.json`, `logic.wasm`, `ui`, `wit`) gets exactly one
  violation, rule `path`, and no second parts violation; release bytes that stop inside a value
  read "found <type> cut short by the end of input"; the log says "not a directory", names an
  empty path, and says a backup must be 108 bytes and how many came. Nine of the thirteen sketched
  outcomes were already delivered (S23, S45, S46, S48), so the dist task fell away: see `Friction`.
  Left for later (user-sim): `NotPlain` still adds "(a symbolic link is refused)" for a plain file;
  a bad backup magic shows U+FFFD; `Mod::read` and `pack` word a FIFO or link differently; dist
  does not try the next base when a mirror answers 404 for the package; by DD-33 the cache-path
  link can still point at a dead base.
- [x] **S43 Eve clones and forks Dave's mod (dev stage 11).** `new --from` clones the named
  repository at the ReleaseRef tag, checks it against the verified package, falls back to the
  package with `new.fork_differs` or `new.fork_unavailable`; Eve's first `publish --push` forks the
  upstream repository on branch `watane`. `app/tests/it/j5.rs::ac2_j5_github` passes; user-sim
  ACCEPTED on the first run. Verified on the Mac: dev 300 passed with `--include-ignored`,
  app driver + J3 + J5 142 passed. Front tests not run (they take over the screen).

Step 6 done 2026-11-14: `app/tests/it/j5.rs::ac2_j5_github` passes; every step-6 stage is done
(pkg 8, dev 10, dev 11). Every step of the build plan is delivered. GitHub is still the fake `gh`
and local git remotes: no real repository, no real static host, no second machine, and no
installer for watane itself.

Step 7 (The real world). Not a step of the build plan: the build plan is finished. VISION's
"Success of the first version" states five things that happen between people on machines, and every
one of them has so far been proved in one process tree, on `127.0.0.1`, against local bare git
remotes and `dev/src/bin/fake-gh.rs`, a GitHub written from the same spec as the code it answers.
These slices remove one piece of fakery each. S50 comes first: the others all need a watane that a
person can install. S52, S53 and S54 need things the human must provide (see "Waiting on the
human"), so they cannot be handed to a cloud foreman as they stand.

- [x] **S50a Installing watane designed.** Delivered 2026-11-18 (merged as 2fb602a; routine `trig_01Gj3q54bMDCtqf3x2FBGHS9` finished, branch removed). A design slice:
  no spec says how watane reaches a machine. The human decided (2026-11-18): one `curl … | sh`
  from GitHub Releases puts `watane`, `watane-dev` and `watane-node` in `~/.local/bin` and
  `watane.app` in `~/Applications`, then gives the Gatekeeper step; it offers (y/N, never silent)
  to install rustup and `wasm32-wasip2`; dev keeps DEV-F-13 (dev installs nothing). Writes a new
  subsystem `install/` (VISION, FUNCTIONAL with EARS rules and scenarios, TECHNICAL with the script,
  the release archive layout and its checksums, the release workflow, upgrade and uninstall, and a
  build-plan table), a journey line in `spec/FUNCTIONAL.md` and its place in `spec/TECHNICAL.md`,
  and the S50b test: where a clean Mac comes from (a second macOS user, or a VM) and what proves
  "no cargo, no target, no checkout". Outcome: a reviewer can build S50b from the documents alone;
  each open question is under `install/TECHNICAL` with options, a recommendation and the cost to
  undo. Touches: `install/*.md`, `spec/FUNCTIONAL.md`, `spec/TECHNICAL.md`, root `AGENTS.md` (one
  table cell). No code. Cloud.
  Delivered: a reviewer can build S50b from `install/` (VISION, FUNCTIONAL INST-F-1..26 and
  INST-S-1..35, TECHNICAL with the script, archive, SHA256SUMS, release.yml, upgrade, uninstall,
  receipt and journal, build plan 11, questions 14) and spec/FUNCTIONAL 2.10 (F-GET-1..5, J7).
  user-sim REJECTED once (receipt rollback, command line tools, app build not `--locked`), fixed,
  ACCEPTED; adversary: three passes on the design (15 breaches, all closed in the documents, each
  pinned by INST-S-26..35 for S50b's tests to turn red); validator PASS twice. Decisions (IN-24..33,
  each the stricter option): no terminal means nothing installed and the commands printed; the
  offer and uninstall's delete read only /dev/tty, tests use a pty; a first install refuses files
  it did not install (exit 8); install folders outside $HOME refused; the receipt commits a
  journaled swap with SIGKILL recovery; `xcode-select --install` printed, never run;
  `cargo tauri build -- --locked`. PO: Q10 and Q1 are settled on the macOS VM (the human's VM
  answer), not a second macOS user; Q17 is a later dev slice; Q16 waits on the human. Friction:
  `factory/check.sh` cannot gate a docs-only task; adversary and user-sim on a design slice patch
  documents, which took 3 and 2 passes.
- [x] **S50b The install script and its archive (install stages 1 to 3).** Delivered 2026-11-18 (merged rebased as f863f48; routine `trig_01AgdQrXH4dny5jSKwNRQYDM` finished, branches removed). Builds install/TECHNICAL
  11 stages 1 to 3: `install.sh` and the `watane-install` crate with its script tests (INST-S-1..22,
  26..28, 30, 31, with fakes for every Darwin-only tool so they pass on Linux), `--version` on the
  three programs (`watane-node --version`), and `package.sh` building the 4.2 tree from fake
  programs. Outcome: on the real programs, the script installs a packed archive into a temp
  `$HOME`, refuses each breach scenario, and upgrades and uninstalls. Touches: `install/`, root
  `Cargo.toml` (one member line), `Cargo.lock`, `workspace-hack/` (generated), `node/` (bin and
  `program.rs` test). Cloud.
  Delivered: the packed `install.sh` puts watane, watane-dev and watane-node in ~/.local/bin, the
  vendor folder and watane.app in place, then upgrades, reinstalls, reports `--status` and
  uninstalls; every refusal (checksum, foreign files, root, platform, version, folders outside the
  home, link members) stops before it changes anything. Proved on Linux with fakes for the Darwin
  tools. user-sim REJECTED once (a closed stdout, `curl | sh | head`, died of SIGPIPE and left the
  temp folder), fixed, ACCEPTED; adversary BREACHED twice (uninstall rmdir'd outside the home
  through a ~/.local link; a project's rust-toolchain.toml let rustup auto-install before any yes;
  an install folder linked to the home let a receipt line delete a top-level folder), fixed, then
  HELD. Decisions (IN-34..39): INST-S-32..35 join stage 1; uninstall and status skip the install-time
  folder check, every removal goes through 8.5; toolchain detection runs from $HOME with
  RUSTUP_AUTO_INSTALL=0; archives with any link member are refused (exit 5); journal and receipt.new
  made with noclobber; a home resolving outside $HOME is never deleted (the rm -rf is printed); PIPE
  trapped like HUP/INT/TERM (exit 141); install folders must resolve strictly below the home; the
  foreman skill's Install adversary row names install.sh, package.sh, release.yml. Friction:
  cargo-deny, cargo-hakari and shellcheck are missing from the cloud image; workspace clippy fails in
  the cloud (no gdk); clippy's dev/src/outcome.rs:1 lint on the cloud toolchain (second report);
  a cloud foreman cannot delete a remote branch (second report). For a later slice: dev doctor's
  rustup detection could take the same neutral-folder rule; dash's `printf: I/O error` on a closed
  pipe; no hint after a pipe closes past the commit point; "upgrading 0.2.0 to 0.1.0" for a
  downgrade; the no-terminal uninstall does not say how to delete the home.
- [x] **S50b-fix install's tests pass on macOS.** Delivered 2026-11-19 (main `441f083`). Sol runs:
  fix-1 2, fix-2 2, fix-3 2, fix-4 1, fix-5 1, fix-6 1, fix-7 3, fix-8 1, fix-9 1. user-sim REJECTED
  once (a Ctrl-C in the first second named a false cause), then ACCEPTED, three green Mac runs
  (128 tests); adversary BREACHED in 6 passes (13 red tests, all fixed), then HELD. Decisions: the
  macOS tree lists `watane.app/Contents/_CodeSignature/*` (IN-40); pty and request waits are one
  600 s constant, at most 4 installer runs per test binary, and an overdue run's group is killed;
  the test launcher resets ignored HUP/INT/QUIT/TERM/PIPE; answers are byte-exact (dd/od, added to
  3.3): only y/yes in any case and only `delete`, any NUL, CR or extra byte is no; the undo retires
  the journal before deleting staging names, is retried once, and says "could not be put back" only
  when an old entry is missing or still `.old`; signals act at once and every dropped status passes
  `check_signal`, guarded by a scan test that is a regression guard, not a boundary (its limits are
  hand-review forms in 3.8); known limits in 3.8 (a Ctrl-C during the swap on bash 3.2 may end as
  exit 5 with the swap undone; a real 129..143 exit from rustup reads as a signal; a signal in the
  final cleanup may leave the temp folder; a signal at a question acts when the read returns;
  uninstall and status need mktemp; a failed undo of a new entry may say "Nothing was changed");
  an unwritable TMPDIR at the home question stops uninstall with exit 1 after the files are removed.
  Open risks, moved into S50c. Linux: rerun in the cloud after the merge (S50b-fix-linux).
- [x] **S50b-fix2 install's tests pass on Linux again, and a Ctrl-C under load on the Mac.**
  Delivered 2026-11-19 (main `3a3418b`). Sol runs: s50b-fix2-1 2; validator PASS round 1.
  user-sim ACCEPTED (283 hand Ctrl-C runs at 0..1 s under load, all 130 with empty stderr);
  adversary HELD. Decisions: the false cause was `main_uid=$(id -u) || die` dropping id's 130:
  every `$(cmd) || die` runs `check_signal "$?"` first, and the status scan no longer exempts
  `|| die`; the stale-journal fixture pid is 99999999; the pty runner closes stdin before wait on
  Linux only (install/TECHNICAL 12.1); the SC2015 sites are explicit ifs. Known limit widened
  (3.8): a real 129..143 exit of any child (tar, mktemp) reads as a signal. After the merge: main
  green on the Mac (124) and on Linux twice (cloud check S50b-fix2-linux, shellcheck 0.9 clean).
- [x] **S50c A person installs watane on a clean Mac (install stages 4 and 5, J7).** Merged
  2026-11-19 (main `7a3a335`). v0.1.0 released 2026-11-19 (run 44572257701, approved by the human);
  `sh install/tests/vm_mac.sh` (no `--from`: the real one-liner over curl) then passed INST-S-1, 23,
  24, 25 against the real release (CLEAN-MAC PASS, VM PASS).
  Foreman `035bf637` (sessionId `035bf637-693e-4f46-b950-2abe784c40a3`) has ended; the PO runs it. Sol
  runs: s50c-1 2, s50c-2 1, s50c-3 1, s50c-fix1 2, s50c-fix2 1. user-sim REJECTED once (a refusal
  that suggested an `mv` through a re-linked folder; a VM home check tripped by macOS's own
  media-library writes), then ACCEPTED: the README one-liner in a fresh Tart VM, with and without
  command line tools, against the CI-built archive. adversary BREACHED twice (a tag named
  `origin/main` fooled the ancestor check; a link swapped in mid-run was written through), both
  fixed, then HELD; s50c-fix2 (refusal text only) had no third pass. Release dry run green twice on
  GitHub. Decisions: receipt `resolved <folder> <R>` lines and 8.5 rule 4 (IN-42); lock
  `~/.watane-install.lock`, a `mkdir`, never broken, a SIGKILL leaves a printed `rm -rf` (IN-41,
  INST-S-33); the undo restores `receipt.old` only with its `moved` line, and recovery does nothing
  when the receipt path is not allowed (IN-43); install folders are rechecked before the swap
  (exit 5); release on `macos-15`; tauri-cli pinned `=2.12.4`; clean Mac = Tart VM (IN-16, 12.3,
  Q10 (b)), images `watane-clean` and `watane-clean-clt` kept for S55; in the guest, macOS's own
  writes deeper in existing folders are skipped; clean_mac.sh reads Mach-O load commands with
  Perl when CLT is absent. Accepted risks (same user or older): a link alternated in the instant
  between check and mv (TECHNICAL 10, not protected); `clean_stale` and recovery remove dead-pid
  `.old`/`.new` names under rule 3 only; the ancestor check guards against mistakes, Q16 is the
  gate (7.1, 10).
  The original S50 sketch, for its intent:
  Someone with a clean Mac and no Rust toolchain ends up
  with a working `watane` and `watane-app` from one command, and can run `watane-dev new` straight
  after. Outcome: the install is exercised somewhere that has no cargo, no target folder and no
  checkout, so the test cannot be a `cargo test` in this repo. The human decided (2026-11-15): unsigned, since there is no Apple Developer
  account; the install tells the person how to get past Gatekeeper, in as few and as calm steps as
  macOS allows. Touches: a new `install/`, `app`, release
  packaging. Nothing in the repo does any of this today: `app/scripts/` holds only `linux-test.sh`.
- [x] **S51 Toolsets and tokens are complete in the app (host stage 4).** Folded into S58 and
  delivered with it 2026-11-14: it touches the same `app/src` and `app/ui` files, so it could not
  run beside it.
- [x] **S52 The front tests can be run.** `App::front` waits for an idle human but not for an
  unlocked screen, so the four `front:` tests fail on a locked Mac (`gui_isolation` ac1) instead of
  skipping. Make them wait for, or skip with a clear message on, `CGSSessionScreenIsLocked`, then
  run all four once on an unlocked screen and record the result. These four have never run.
  Needs the Mac and a human at it: the human allows the screen at weekends and after 18:00 on any
  day (2026-11-15). Local, branch `slice/s52`. Touches: app/src (the front wait), app/tests/gui_*.
  S53 owns app/tests j3, j5 and support/dev.rs.
  Delivered (16c874a): the four front tests ran for the first time, on an unlocked screen, 2026-11-15 20:14 to 20:20, and all four passed (recorded in app/NOTES.md). On a locked screen each skips with a visible line; `App::front` returns `Option<App>`; an unreadable lock state panics rather than counting as unlocked. Verified natively on the Mac by the foreman after its rebase on 19fe1e9: app `--features driver --include-ignored` green, clippy and fmt clean.
- [x] **S53 J3 and J5 against real GitHub.** The journeys run against a repository under a real
  account with the real `gh`, in place of `fake-gh`. `fake-gh` was written from the same spec as the
  code it serves, so this is the slice most likely to find something. Keep `fake-gh` for the fast
  suite; the real run is a separate, ignored target. Branch `slice/s53`, local (the token lives on
  the Mac). The human gave (2026-11-15) the throwaway org `minmax-fund` and a fine-grained token for
  it in `CLAUDE_GH_TOKEN` (`~/.zshenv`; Administration, Contents and Pages read/write; checked: it
  creates, pushes, turns on Pages and deletes there, and gets 403 on the human's other repos). Rules:
  the org also holds the human's `OpenMLOps`; a test touches only repositories it creates, named
  `watane-j-<random>`, refuses to start if the name exists, and deletes only those; the token never
  enters a log, a commit or a test's output. Outcome: J3 and J5 pass against `minmax-fund` with the
  real `gh`; each difference from `fake-gh` becomes a fix (or a question) and a `fake-gh` change
  that keeps the fast suite honest. Touches: app/tests (j3, j5, support), dev/src/bin/fake-gh.rs.
  S65 owns the rest of dev/ (src, tests) until it merges: a fix needed in dev/src goes to the product
  owner as a question.
  Delivered (8fa245b, merged rebased): J3 and J5 pass against real GitHub in `minmax-fund` with the real `gh` (`cargo test -p watane-app --features driver,real-github --test j3 --test j5 -- --ignored real_github`, with `CLAUDE_GH_TOKEN`). Behind the `real-github` app feature, so no cloud run reaches GitHub; test processes drop the token. No difference from fake-gh came up apart from publishing under an org (Waiting on the human); Carol's fork is not exercised. Verified on the Mac: hakari clean, app and J2 to J5 green, both real journeys green (77 s, 73 s), the org left with only OpenMLOps.
- [x] **S54 A real static host.** Alice's channel is served from GitHub Pages over real TLS and dist
  fetches from it, in place of `tiny_http` on `127.0.0.1`: real certificates, real latency, a CDN's
  404 rather than a test server's instant one. Checks DD-26 (a listed base on localhost or a local
  IP literal is skipped) against a base that is neither. The human chose (2026-11-15) GitHub Pages in
  `minmax-fund`, with the same token and rules as S53 (only `watane-j-<random>` repositories the test
  creates and deletes; the token never in output). Pages can take minutes to serve a first build:
  the test waits with a deadline and says so when it runs out. Local (the token), behind the
  `real-github` app feature. After S71: it publishes the channel with `--owner minmax-fund`.
  Branch `slice/s54`. Touches: a new app test file (`app/tests/pages.rs` or a new `real_github` test
  in a file S74 does not own), app/tests/support/github.rs and pages.rs, dist only if a finding needs
  it. S74 (local) owns app/src, app/ui, app/tests/support/journey.rs and driver.rs, j2, j4, manager.
  Delivered (merged as b514a89): Bob installs Alice's mod from her channel served by GitHub Pages over real TLS (`https://minmax-fund.github.io/watane-j-<r>/`), finds it through Ivy's listing with that public base (DD-26 does not skip it), and a fresh home given a never-published Pages path gets a clear `dist` error naming the URL. Decisions: the real J3 itself serves from Pages (no second journey); harness `RealGithub::pages` and `wait_served`/`wait_status` (ureq 3.5.1 with rustls as an app dev-dependency, polling every 10 s, a deadline panic with the last status); the missing-base check runs from a fresh home, since dist reads a known channel from its recorded bases; the test accepts `not found` or `HTTP 5xx` (Pages answered 503 for a missing path on a fresh site) after waiting for one 404. Verified on the Mac: hakari clean; the app driver suite green with ignored (front tests not run); dist green; the real J3 green (101 s); minmax-fund held only OpenMLOps.
- [x] **S71 Alice publishes under her GitHub organisation (`publish --push --owner`).** The human
  chose (2026-11-15) option (a) of S53's finding. `watane-dev publish --push --owner <account>`:
  derived repositories become `<owner>/<slug>` and `<owner>/watane-channel`, and a fork adds
  `organization=<owner>` when the owner is not the `gh` login; without `--owner`, as today. fake-gh
  accepts and checks `organization=`. The template `AGENTS.md` and `publish`'s help name the flag.
  Outcome: the real J5 runs Carol's fork in `minmax-fund` (forked, the parent set, watane's default
  branch), and the fake J5 checks `organization=`. Touches: dev/src (share.rs, the publish CLI),
  dev/src/bin/fake-gh.rs, dev's template and TECHNICAL, app/tests/it/j5.rs. Local (the token). After S69
  (dev). Also fixes dev's `ac10_endless...` deadline test (Friction, S65 and S69).
  Delivered (merged as 8de79ae): an agent runs `watane-dev publish --push --owner <org>` and dev creates `<org>/<slug>` and `<org>/watane-channel`, and forks a derived mod into the org (`organization=<org>`, parent set, default branch `watane`); without `--owner` nothing changes. Real J5 passed twice against minmax-fund, which held only OpenMLOps afterwards. Decisions: GitHub forks into the org that owns the parent under a new name, so the real J5 forks inside minmax-fund, with Carol's slug a reserved `watane-j-<r>` (harness `reserve()` checks a 404 and records the name for deletion); `--owner` checks GitHub's account rule and requires `--push`; `organization` is sent only when the owner is not the login (case-insensitive), and fake-gh refuses `organization=<login>`; ac10 builds the endless source once untimed, then times the cached run against 7 s. Verified on the Mac: dev green with ignored except envelope ac5 (red on main since S73, see Friction), app J5 green. For a later slice (user-sim): `--owner` does not apply to a channel whose origin is already stored, silently; the `--owner` usage errors point to `watane-dev --help` instead of `publish --help`.
- [x] **S87 A light machine reports a full machine's changed bytes again.** Delivered 2026-11-19
  (main `b620ef7`). Sol runs: S87-1 2, S87-2 1, S87-fix1 1, each validated in round 1. user-sim
  ACCEPTED; adversary HELD (Node boundary, net/src/). Cause: iroh-blobs 0.105 checks a referenced
  file against its outboard before sending and resets the stream, so the light machine gets either
  a cut stream (BadObject, `refused ... does not match its hash`) or the reset (Unreachable "not
  served"), a race; `need_blob` returned that Unreachable without emitting it, and 92363a6's timing
  tipped the race. Decisions: net TECHNICAL 5.9, `need_blob` emits Event::Unreachable when a full
  machine connected but did not give the package, at most once per machine until a fetch from it
  succeeds (adversary: 20 failures could push the only `refused` line out of 20 problems);
  machines::ac6_s33 and j8 AC5 accept either line, each with the machine's short key and its exact
  cause; opt-level 1 stays.
- [x] **S94 install's tests fit the budget (test cost).** Delivered 2026-11-21 through PR #6
  (main `b5e54ab`, rebase merge on green CI; PO fix `6f4d4e6`/`b5e54ab` for macOS; routine
  `trig_013eoogZcrq5CzENfEcRQFrb`, cloud, ~7.2 h). Runs: s94-1 1, s94-fix1 2; validator round 1
  each. user-sim ACCEPTED (13 install.sh guard mutants caught by the default set); adversary
  BREACHED twice (guard behaviours left slow-only, then pre-existing gaps), all pinned as default
  tests; no third round (only main's old gaps left). A shell-script unit level (install.sh sourced
  without its last line, each case a subshell under `set -eu`, TECHNICAL 12.1); process starts
  ~19,100 -> ~2,000. Mac: default 125 s (was 839 to 1059 s), with ignored 483 s. PO fix: unit
  receipt test expected the canonical `/private/var` path; the deadline test's scale 1200 left 2 s
  of the whole bound on a loaded Mac, now 300 (16 s). Decisions: AC7 flake was the test reusing one
  world for 40 Ctrl-C runs (now one per run, 12 delays); root-only failures fixed in the tests;
  accepted slow-only reductions (undo sweep one per kind, pty NUL shapes as a unit). RISKs: the
  uninstall `delete` consent is proven only by slow pty tests (as before); spec 3.6 step 0 cannot
  happen (recover removes this run's .new/.old first), spec vs code.
  install's default integration run took 858 s on main `501c123` and 1059 s after S89 (Suite time; budget 120 s).
  Story: a maker runs `cargo test -p watane-install` and it is done in under 2 minutes, proving
  the same rules. Outcomes: (1) the default run under 120 s on a quiet Mac and in the cloud;
  (2) each moved or merged test names the rule it still proves (a shell-script unit level, one
  script run per behaviour not per case, shared fixtures); the tests that truly need a whole
  install run become `slow:` with their reason; (3) nothing that guards a *guard* behaviour
  (the receipt and journal checks, refusing foreign files, signature and checksum checks) loses
  coverage; the validator's mutants show it. Touches: install/tests/, install/TECHNICAL (test
  section). Cloud, with the macOS sh run by the PO after.
- [x] **S89 Installing tells the truth at every step (leftover sweep: install).** Delivered
  2026-11-20 (main `1a4ea99`, PO fix `d5c0d15`; routine `trig_01BiHhgpCP7z8t1JR3CE9ExU`, cloud,
  ~3.5 h). Runs: S89-1 2, S89-2 2, fix1 to fix4 1 each; validator rounds 1 (S89-2: 2). user-sim
  ACCEPTED in round 4; adversary HELD after 4 BREACHED rounds, each a failure text that a nearby fix
  made untrue (exit-5 texts are better decided once from disk state, 3.8/3.9). Alice gets a true
  line at every step: told watane is installed after a late Ctrl-C or one at the toolchain
  question; `--status | head -1` quiet; `~` paths; install-it-later names the offer's URL; no
  empty folders after an interrupted fresh install; rustup's text set apart; release.yml actions on
  node24 (pinned by SHA); clean_mac's real-release test skips with its reason. Decisions: the
  interrupt hint's "committed" is decided as undo decides it; `say` hides printf's error only on
  PIPE; remove_created rmdirs only this run's folders, never through a link (limit: a same-user
  parent swap can remove an empty folder in the home); a leading `-` in WATANE_RUSTUP_URL is still
  a curl option (limit). On the Mac: install 193 passed; the `/dev/full` test is Linux-only (PO
  `cfg_attr` ignore). vm_mac.sh not rerun (messages only; the real-release run passed on v0.1.0).
- [x] **S90 The manager says what happened (leftover sweep: app).** Delivered 2026-11-20 (main
  `393ae6c`; foreman `a0945c7d`, ended; ~3 h). Sol runs: S90-1 1+2, S90-2 1, S90-3 1+2; validator
  PASS by round 2. user-sim ACCEPTED (7/7); adversary HELD (Local API, app/src/api.rs). Alice sees
  what was done or denied: a disabled provider's own reason on the open page, busy Follow/Add,
  channel rows in manager words, "watane now follows <X>", "You denied <what>; nothing was
  signed.", plain /api/fetch shape errors, hidden windows that still refresh. Decisions: hidden
  main refreshes every 10 s, visible every 2 s, main alone without WebKit background throttling;
  enable/disable of a provider reloads open consumer pages (FUNCTIONAL S9); /api/fetch shape errors
  stay 400 `bad_request` and repeated keys are refused; the channel-view memo is keyed on dist.heads
  plus (mtime, length, inode) of follows.json and site.json; J8 AC6 requires `node <server>
  refused: not_allowed` after S92; app/src/api.rs joins the foreman skill's Local API row; the
  focus guard dropped (the human). `j2::ac7` no longer counts polls in a fixed 5 s. Not fixed:
  "Keep data, reinstall marks storage new" needs host (`new_grants`, host/src/catalog.rs:470, no
  kept-state query).
- [x] **S91 Small truths in pkg, log and host (leftover sweep).** Delivered 2026-11-20 (main
  `54da8c9`, PO fix `b6fdede`; routine `trig_01DwQcTobkjJsLJ8rJcL9toc`, cloud, ~2.5 h). Runs:
  S91-1 1, S91-2 1, S91-3 1, S91-fix1 1 (Claude implementer); validator round 1 each. user-sim
  REJECTED once (a .wit swapped for a directory read as rule `limit`), then ACCEPTED; adversary
  HELD (Verification, Origin/Local API, add_provider writes). A maker or mod that hits a race or
  an odd file gets watane's own cause. Decisions: pkg races give rule `path` with CHANGED, a
  directory swapped in for a file `refusal::DIRECTORY` (4.8, 6.4); log opens identity.key once
  with O_NOFOLLOW|O_NONBLOCK and checks it with fstat (libc added to log; a mode hiding the key
  from its owner now reads Io, still refused); host's unknown route on the API host is 404
  `not_found`, outside HostError (2.2, 8.1); 413 is not drained, 8.1 says why that is enough;
  `Served::shutdown` 4 s for routers, 5 s in all (2.1, 12.2); the port test starts once. The cloud
  could not run host's 5 IPv6 tests; on the Mac one (`hostile::ac5`) still expected
  `bad_request`, fixed by the PO to `not_found` per 8.1, host 170 passed.
- [x] **S92 Blame the right machine (leftover sweep: dist, net, node).** Delivered 2026-11-20 (main
  `c224dd3`; routine `trig_01PPzACt58xPQLQu5d673Hoc`, cloud, ~2 h). Runs: s92-1 1, s92-2 1, s92-3 1,
  s92-fix1 1; validator round 1 each. user-sim REJECTED once (a holder error dropped the OS cause),
  then ACCEPTED; adversary HELD (Verification, Node). Alice's failed install on a light machine now
  names the real cause and whose machine: a full machine's bad copy names it and "not served"; a
  local failure reads "this machine | ... was not stored: <OS cause>" and blames no one;
  StaleBases reads "1 base" and says "no release of <slug>" when no base holds it. Decisions:
  dist `StaleBases.slug_held` (DD-34, app mapping unchanged); net `need_blob` returns a local
  failure as `NetError::Io`, no event, no mark (5.8, 5.9); node keeps every need error but Off and
  NoFullMachine and maps it to `DistError::Io` "the server"/"this machine" (5.3, NO-14, NO-4); a
  kept net error now replaces dist's "not followed" on a light machine even when no machine could
  have helped (accepted). Needs app: app/NOTES.md:741-745 is stale (node keeps the blob error);
  run j8 AC6 with `--features driver` on the Mac (S90 is in app).
- [x] **S93 dev's last misleading lines (leftover sweep: dev).** Delivered 2026-11-20 (main
  `e04b480`; routine `trig_01RZZM6VuY914sVyAGW9Vv4y`, cloud, ~5 h). Runs: S93-1, S93-2, S93-fix1
  to fix10, 1 each: twelve tasks, cut too big for one sweep. user-sim REJECTED three times, then
  ACCEPTED; adversary BREACHED three times (invisible characters in pkg's wit message; sibling
  paths), each fixed with a rule rather than a longer list, then HELD. A maker's agent gets a real
  cause and a runnable `next` at every dev failure in the leftover lines; a WIT error has a
  character column; publish --push offers the GitHub Pages URL. S30 settled by measurement (warm
  publish ~1.1 s, cold ~31 s, all before the post and outside --wait; dev TECHNICAL 9.7).
  Decisions: host.no_answer's rerun doubles the --wait in force (none counts as 600); pkg's
  escape_message escapes Unicode 16.0 Default_Ignorable, Cf, Hangul fillers and U+2028/2029 by
  range table (pkg TECHNICAL 4); sibling_paths names only strict descendants of the canonical build
  folder; usage.no_source's fix is `new my-<slug> --dir <dest> && add --release <ref> --dir <dest>`;
  host.declined's fix asks the person, then reruns; push.site.pages is
  `https://<o>.github.io/<r>/`. Adversary risks left (diagnostic choice only): the escape set is
  not wider (U+2800, Mn, Co); a prefix after `//`, `=` or `,` is not a path start; cutting before
  normalizing can name a false sibling; a stray generate! literal can replace real errors.
  On the Mac: dev's slow set 46/46 (283 s, the 18 real-`watane` tests included), pkg green.
- [x] **S88 The remove confirm always has Cancel focused (main fix).** Delivered 2026-11-20 (main
  `60606cc`; foreman `a244ec23`, ended). Sol runs: S88-1 1 + 5 resumes; validator round 1.
  user-sim ACCEPTED; adversary not run (tests and notes only). Cause (app NOTES, S88): ~30-40 ms
  after the consent window closes, WebKit natively blurs the focused element in the hidden,
  never-key main webview; the original failures clicked Remove inside that window. Decision: the
  test quits and reopens the app before Remove (as J2 does), every assertion kept, the focus check
  still immediate (it fails with app.js's `focus()` removed). 50/50 alone and in the full driver
  suite. Not proven: visible windows (front tests not run). Accepted limit (the human, 2026-11-20):
  no guard for WebKit's native blur; if the focus check flakes again in hidden windows, mark it
  `#[ignore]` with this reason rather than chase it.
- [x] **S55 Two machines.** Delivered 2026-11-20 (main `5014400`; foreman `4a61b2b4`, sessionId
  `4a61b2b4-c5d7-4988-a8cf-2bf6cbe171b6`, ended). Sol runs: s55-1 1, s55-2 1, s55-fix1 2, s55-fix2 1,
  s55-fix3 1. user-sim REJECTED once (an unreachable web host and an unnamed `--from` folder gave no
  cause), fixed, then ACCEPTED: TWO-MACHINES PASS three times on the real VM. adversary not run (no
  boundary: tests, specs and briefs only). Proves two homes and two network stacks (no host share,
  checked; every byte over the VM network), not NAT across the internet. Bob's app in the guest is
  a debug `driver` build in a minimal Bob.app, so the release app's consent window is not exercised
  there (a script cannot click it: SIP, no Accessibility); the installed v0.1.0 is checked by
  `--version` only. Decisions: Bob's app is launched in the guest's GUI session with `open -n -W`
  and driven over ssh (`App::over`, app TECHNICAL 11, install TECHNICAL 12.6); the fake web host is
  stock `/usr/bin/python3 http.server` (Apple-signed, so no firewall prompt); Bob.app copies
  NSAppTransportSecurity from app/Info.plist at run time (without it ATS blocked the mod page; the
  release app has it); VM helpers moved to install/tests/vm_lib.sh; hard deadlines (300 s per VM
  step, 900 s install, 1200 s journey, 3600 s run) in vm_two_machines.sh only; cleanup runs with
  `set +e`. Story: Alice publishes on one machine, Bob installs on another and uses the
  mod. This is VISION success criterion 3 stated as it is written, and it has never run between two
  homes that did not share a filesystem. After S50, S53 and S54. The second machine is a VM on
  this Mac (the human, 2026-11-18, said more than once before): its own disk and its own address
  on the VM network, so no shared filesystem. Prefer the macOS VM S50b sets up as its clean Mac, so
  one VM serves both. It proves two homes and two network stacks, not NAT across the internet; say
  so in the delivery.
  Outcomes (J3 between two machines, checked by a user-sim on the real programs):
  (1) Alice, on this Mac, makes the reading list and publishes it; Bob, in a fresh Tart VM cloned
  from `watane-clean` with watane installed by the one-liner (`--from` a locally built archive, or
  v0.1.0 when main has nothing new for it), follows her link, sees Alice as the verified
  publisher, installs the mod and uses it in his app.
  (2) Nothing reaches Bob through a shared folder: the VM has no mount of the host, and every byte
  comes over the VM network (the script checks there is no share).
  (3) Alice changes a file on the web host; Bob's watane refuses it and names the cause.
  (4) One script, `install/tests/vm_two_machines.sh` (beside `vm_mac.sh`, reusing its helpers),
  runs it end to end and prints `TWO-MACHINES PASS`; it deletes its VM on every exit.
  May fake: the web host, as an HTTP server on this Mac that the VM reaches by its host address
  (no push to the human's real GitHub without asking). Leaves out: NAT across the internet, Linux
  guests, and a second human. Touches: install/tests/vm_two_machines.sh, install/tests/ helpers it
  shares with vm_mac.sh, install/TECHNICAL 12; any fix it finds in dist/net/app is its own task.
  Local (needs the Mac's VM).
- [x] **S86 dev says what is wrong (leftover sweep 3).** Delivered 2026-11-18 (merged as 7c29140; routine `trig_01XMgpCFKi3MdbVHsqbfqzTU` finished, branches removed). A maker's agent reads a real cause and a
  `next` it can run at eleven places in dev where it now does not. Re-checked against main
  2026-11-18 (`### dev` in Leftovers; the doctor item is fixed). Outcomes: (1) a WIT syntax error
  gives a line number (pkg/src/world.rs renders wit-parser's error with its source map, :202, 226,
  249); (2) `test` and `serve` take `--home`, and every `next` and fix for them carries `--dir`
  when the maker passed one; (3) no fix is "run this command again" prose (api.rs:25, 255, 474;
  cli.rs:711; build_on.rs:251), and `add --release`'s no_source fix names the known release
  (build_on.rs:286); (4) the slug rename line drops its parenthetical when nothing has a dash
  (cli.rs:332), `local_dependency` says where a local provider's folder is named (cli.rs:766), a
  quoted multi-word search counts its words (cli.rs:564), `--help | head` exits quietly
  (main.rs:17), and an outside `.wit` read is one problem, not one per file (builder.rs:369);
  (5) fake-watane can be told never to answer, so `host.no_answer` has an integration test, and
  `ac2_real_home_without_app` stops `watane` through the existing `Host` guard. Touches:
  pkg/src/world.rs, dev/src/{cli,api,build_on,builder,main,preview}.rs, dev/src/bin/fake-watane.rs,
  dev/tests/{check,install}.rs, dev/TECHNICAL.md. Not dev/src/share.rs, dev/tests/it/share.rs,
  dev/tests/it/publish.rs or dev/tests/support/ (the human's test-speed session works there). Cloud.
  Delivered: a maker's agent gets a real cause and a runnable `next` at the eleven places: a WIT
  syntax error has file, line and column; `test` and `serve` take `--home`; reruns carry `--dir`
  and `--home`; app fixes end `then run: <rerun>`; `--help | head` exits quietly; an outside `.wit`
  read is one problem; fake-watane can be told never to answer. user-sim ACCEPTED; adversary
  BREACHED once (the WIT message echoed the untrusted source line raw: ESC, BEL, `\r`, U+202E),
  fixed, then HELD. Decisions: `Client::for_app(home, rerun)`, cli.rs builds every rerun and api.rs
  holds no command lines; pkg escapes control, bidi and invisible characters in the `wit` message
  itself (pkg/TECHNICAL 4 step 2 and its security row); several outside `.wit` reads name their
  deepest common folder; a search word starting with `-` gets `--` before it in the rerun;
  `serve --stop` accepts and ignores `--home`; publish keeps two old texts that the human's
  dev/tests/it/publish.rs pins (`// s86:`). Friction: the cloud cannot run the real `watane` (no IPv6
  loopback), so every real-preview test is not run there; the dev allowlist is not vendored in a
  fresh container (`dev/scripts/vendor.sh`); clippy's `too_long_first_doc_paragraph` on
  dev/src/outcome.rs:1 fails check.sh in the cloud only (newer toolchain); a cloud foreman cannot
  delete a remote branch; an implementer reported green while a pkg test failed (check.sh caught it).

- [x] **S56 Leftover sweep (dev messages).** Delivered 2026-11-14 (cloud, merged to main `3a39a6f`). What a maker's coding agent reads
  at the moment something has already gone wrong. Fifteen items from "Leftovers", every one of them
  a message, a fix or a `next` that misleads; none is hard, and together they are most of dev's
  text on a bad day. Re-check each against main before writing a task: this ledger was audited on
  2026-11-14, but that is exactly the check S49 skipped.

  In scope: the doubled "error" and the whole help text on no arguments (main.rs:20, 12); the failed
  serve whose fix says `test --json` while `next` says `serve --json`; the `add --release` fix that
  says to pass another mod's folder; the `--home x/../h` fix that still says to start the app; the
  `next` values that are prose instead of a rerun (cli.rs:184, 198, 308, 511, 850, 881); "it has no
  release" with dev/TECHNICAL.md:227 documenting a fix the code no longer gives; the deps.json self
  entry naming a `wit/deps` folder that does not exist; the prebuilt `next` that contradicts its own
  warning; the misleading `--no-source` refusal; `build.reads_outside` on a leftover `wit_bindgen`
  path; `env.git_identity` keeping git's stderr verbatim, user@host included; `push_failed`'s
  "remote: remote:" and trailing whitespace; the `-c http.lowSpeed*` flags in its fix and `next`;
  the `--home <abs>` appended to every `next` even when the home came from `WATANE_HOME`; and the
  `repository` fix printing a literal `<owner>/<slug>` though the slug is in scope.

  Out of scope, and why: `doctor` in a mod folder (unsettled, needs one run to confirm); the WIT
  `line: null` (the fix belongs in pkg); fake-watane's missing never-answer mode and the leaking
  `ac2_real_home_without_app` (test infrastructure, not a person's text); `test` and `serve`
  refusing `--home` (a feature, not a message).

  Outcome: a maker's agent that hits each of these reads a sentence that names the real cause and a
  `next` it can run. Two of these are pinned by expected strings in dev/tests (`push_failed`'s
  flags at share.rs:711, 791, 816, 860, 876; "it has no release" at publish.rs:76), so the tests
  move with the text. Touches: dev only. Cloud: dev's suite runs there; `scripts/vendor.sh` must be
  run first because `dev/allowlist/vendor` is gitignored. The app journeys cannot run in the cloud.

  Delivered. All fifteen were still open on re-check; four were described slightly wrong (the
  `lowSpeed` pins are in dev/src/share.rs unit tests, cli.rs:308 is the `new --from` next with no
  `--dir`, dev/TECHNICAL.md:227 was changed to the code's fix rather than the reverse, the cycle fix
  names its folder only when it exists). user-sim REJECTED once: S56-3 made a missing-argument usage
  error lose the argument's name; fixed in S56-fix1, then ACCEPTED. The foreman flagged
  `api::tests::ac5_search_timeout_is_thirty_seconds` as flaky (6 of 8 failed in the cloud); it
  passed 6 of 6 on the Mac at 30.51 s, so it is the cloud. Verified on the Mac stacked with S57 on
  S58: dev `--include-ignored` 311 green, including the 18 real-host tests the cloud cannot bind.
- [x] **S57 pkg, log and dist say what is wrong (leftover sweep).** Delivered 2026-11-14 (cloud, merged to main `3a39a6f`). Six open
  items from "Leftovers", all of them a refusal that misleads or a read that is not safe. Re-check
  each against main before writing a task.

  pkg: `add_provider` still lstats a path and then re-reads the path string (add.rs:9-26, 35, 48),
  so it never moved to the no-follow handle walk S46 built — the one part of pkg that still has the
  check-then-read gap S22 and S46 closed everywhere else. An error during a mid-read swap is plain
  I/O text with no rule (folder.rs:110, 200, 275) and has no test at all. `Mod::read` and `pack`
  word the same FIFO or link differently (folder.rs:92, 217 vs package.rs:24, 298).

  log: `NotPlain` adds "(a symbolic link is refused)" even when the type mismatch is an ordinary
  one (lib.rs:29, raised at identity.rs:23), so a plain file is blamed for being a link. A bad
  backup magic shows U+FFFD (`from_utf8_lossy`, identity.rs:212); the only magic test corrupts into
  valid ASCII, so non-UTF-8 is uncovered — write that case.

  dist: when every base is stale the answer is a bare `UnknownRelease` with no next step
  (error.rs:80). The single-stale-base case was fixed in S45 and reads well; this one reads the same
  to a person and tells them nothing.

  Out of scope: Q12 (log entries of unlisted channels are never removed) needs a `forget` API that
  log does not have, so it is a design question, not a sweep. The six items in "Decide, do not
  sketch" are not here either.

  Outcome: each of the six refusals names the real cause, and the two uncovered cases have a test.
  Touches: pkg, log, dist. Cloud: all three suites run there.

  Delivered. `add_provider` was already safe on the read side; the real gap was the write side
  (`create_dir_all` and `fs::write` under a swapped `wit/deps` wrote outside the mod). It now writes
  fd-relative (mkdirat, openat with O_CREAT|O_EXCL|O_NOFOLLOW, an unlinkat walk); the AC1 test fails
  on the old code. user-sim REJECTED once: a second mid-read window, between listing and stat in
  `open::entries`, still read as plain I/O; fixed in S57-fix1, then ACCEPTED. dist has a new
  `DistError::StaleBases { release, bases }`, and DD-34 says so. Verified on the Mac stacked with
  S56 on S58: pkg 140, log 29, dist 113, host 153, app 146, J2-J5 green; fmt and clippy
  `-D warnings` clean across the workspace.
- [x] **S58 The app is complete and says what happened (leftover sweep, host stage 4).** Delivered
  2026-11-14 (`c424dd2`, branch `slice/s58`, merged to main). Takes S51 with it.

  The last `part` row: the app can remove a toolset, not only end a token. `Host::remove_toolset`
  exists (host/src/tools.rs:195) and host/tests/it/program/catalog.rs:1000 covers it; nothing in
  `app/src/commands.rs` calls it. That closes host stage 4, carried as `part` since S5.

  The text, six items: list, include, publish and install consents show the 52-character key before
  the name, though own rows were fixed in S44 (app/src/confirm.rs:206, 219, 28 — pinned by
  `ac1_index_sign_details_are_complete_and_ordered`). A wrong-channel follow error shows fingerprints
  only, with no party-name lookup on that path (core.rs:537, dist/src/error.rs:99). A mod card's
  refresh error is raw transport text while index rows go through `words::index_read`
  (core.rs:357, 229). A non-JSON body gets "give link or release", discarding the serde cause
  (api.rs:296). On a new home the own row sits beside the "add an index" hint (app/ui/app.js:222).
  The app's JS re-derives the MCP tool name with `.replace()` instead of using the server's `tool`
  field (app/ui/app.js:117).

  Out of scope: the app's behaviour items (no busy state on a hanging follow, hidden windows polling
  only on focus, `Core::mods()` re-reading channel views every 2 s, the busy-port Install button,
  the missing grant-change notification) and the four host items. They are a slice of their own.

  Outcome: host stage 4 goes to `done`, and a person reads a name rather than a key or a transport
  error everywhere the ledger named. Touches: app, host. Needs the Mac: the app cannot build without
  GTK, and J2 and J4 must pass.

  Delivered. The app's agents view has a Remove button per toolset (`app/ui/app.js:122` ->
  `toolset_remove` -> `Host::remove_toolset`), driver-tested in `app/tests/it/j2.rs:310`: host stage 4
  is done, the last `part` row in the table. All seven items were still open on re-check; only one
  line had drifted (the mod-card notice surfaces at `core.rs:237`, not 229). Consent values now read
  `<readable> - <key or reference>`, name first, full key kept so it can still be checked.
  `toolsets_list` returns `tools[]` from the same Rust function as `tools_list`, so the JS no longer
  re-derives tool names. user-sim REJECTED once: the install consent's per-mod release line
  (`app/ui/consent.js:73`) was key-first too and the sketch had missed it; fixed in S58-fix1, then
  ACCEPTED. Verified by the product owner on the Mac with `WATANE_BIN` set: app `--features driver`
  all non-ignored green (77 + 10 + 5 + 10 + 1 + 4 + 19 + 3 + 3 + 5 + 9), J2, J3, J4 and J5 green,
  host `--include-ignored` 153 green. The four `front:` tests were not run (S52, needs an unlocked
  screen).

Step 8 (Own machines). Decided by the human 2026-11-14: step 7 stays "the real world" and the peer
design is step 8. A person's machines keep each other current directly, with no static host between
them and no endpoint someone else runs (VISION "After the first version"). It is stage D1 of
`spec/PEERS.md` 10, designed there and in `net/`: the new subsystems `net` (node key, tickets with
hints, `watane/1`, iroh-blobs and iroh-gossip, pairing, a device list with expiry, mDNS, relay
choice, the DHT switch) and `node` (`watane-node`, full replica and relay role); a keyless open in
`log` and `dist`, and dist's holder methods; the app's views to turn `net` on, pair, list devices
and choose a relay. Its journey is J8 (spec/FUNCTIONAL.md). Friends, strangers, public peers and
scale (D2 to D5) are a sketch in PEERS.md and come after it.

The design landed on main as `7be1bfe` (2026-11-15), the review's questions decided by the human
(net/ARCHITECTURE-PROPOSAL.md 16.1). Four pieces are still marked "for the step 8 design slice":
`node`'s own documents (net/TECHNICAL 8), dist's keyless open and holder methods (dist Q13), log's
keyless open (log/TECHNICAL 12 item 13), and the app's views for own machines (net Q3). So step 8
starts with a design slice, and net's N1, which needs none of those, runs beside it.

Decided by the human 2026-11-15: step 8 runs beside step 7. VISION says each step works before the
next starts, but nothing in D1 needs real GitHub or a real static host, and step 7 waits on the
human for its credentials and machines. S59 and S60 started the same day.

J8 runs on one machine: net's test plan binds every node to loopback with mDNS off (net/TECHNICAL
7), so the journey's server and laptops are homes on the Mac. Running it on real separate machines
is a step-7-style check, after S55.

- [x] **S59 Step 8 is designed down to its last subsystem (design slice).** Delivered 2026-11-15 (cloud, `5e02495`, merged). Writes `node/VISION.md`,
  `node/FUNCTIONAL.md` and `node/TECHNICAL.md`, with a build plan: the crate that joins `dist` and
  `net` (neither knows the other), `Holdings` over a `Dist` handle, the `told`, `need` and
  `need_blob` calls, and the `watane-node` program (a keyless home, full or light, relay by flag).
  Settles dist Q13 in dist/TECHNICAL.md (`Dist::open_keyless`, the holder methods of net/TECHNICAL
  3.1, a record reason `kept`, reads that do not hold the store lock across network calls, RV-06)
  and log item 13 (`Log::open_keyless`) as build-plan stages of their own. Designs the app's D1
  views (turn on, the join question with the short key, devices and roles, expiry, relays, the DHT
  switch with its consent text) in app/FUNCTIONAL.md and app/TECHNICAL.md, which answers net Q3,
  and decides net Q2. Outcome: every stage step 8 needs has a row in a subsystem's build plan, and
  `Stage coverage` here gets one row each. No code. Touches: new `node/`, dist, log, app, net
  (Q2, Q3 closed). Cloud: documents only. A reviewed design merges; a choice that is hard to revert
  goes to the human as change, alternatives and consequences.
  Delivered. Documents only (15 files, +1293); no code, so no implementer and no user-sim. Validator
  RETURN in round 1 (7 items, all fixed), PASS in round 2. New stages, in build order: log L6
  Keyless, dist S8 (one slice with L6, since both change `identity()`), node 1 Holder (needs L6, S8,
  net N1 and N2), node 2 Machines (needs net N4), node 3 Program (`watane-node`), app G Own machines
  (needs node 2 and 3; J8 in app/tests/it/j8.rs). Checked by the product owner before merging: docs
  only, the two Q2/Q3 rows its only net/TECHNICAL edit, the doc-reading app tests green (static_config
  5, linux_runner 3, lib 77), and every net call node relies on (`told`, `need`, `need_blob`,
  `answer_join`, `Holdings`) present in net's API, `open_keyless` and `drop_identity` agreeing
  between log and dist. Four net gaps are left as questions in node/TECHNICAL.md 13, because the
  brief forbade S59 to edit net beyond Q2 and Q3: Q1 and Q4 belong to N1 (`Options.on` and
  book.json; `Options` and `Limits` derive Clone, Debug, Default), Q3 to N3 (gossip hints on a light
  machine only for channels in `heads(None)`), Q5 and Q6 to N2 and N4 (prefer a full replica over
  the anchor; a cap of 32 channels per query). Whichever net slice comes next takes each one.
- [x] **S60 Two machines exchange heads and entries over `watane/1` (net N1).** Delivered 2026-11-15 (cloud, `4d93c35`, merged with a product-owner fix `5a53ccd`). Creates the `net`
  crate and adds it to the workspace: the node key, the pairing code text, the wire codecs and their
  limits in bytes, `hello`, `heads`, `entries` and `fork` between two local nodes, and the 4.3
  golden vectors made with `ed25519-compact`. Pins iroh, iroh-blobs and iroh-gossip, and answers
  net Q1 against those pinned releases: can the blobs store add a file by reference, and can a
  protocol handler be wrapped in iroh's router. If either is missing, the replacement goes into
  net/TECHNICAL.md 12 before N3. Outcome: scenario S1 and `heads` and `entries` between two trusted
  test nodes pass; S13 runs from here on. Touches: new `net/` crate, workspace `Cargo.toml`. Cloud:
  bind `127.0.0.1`, not `::1` (the cloud has no IPv6 loopback); no test reaches the internet.
  Delivered. Pins iroh =1.4.1, iroh-blobs =0.105.0, iroh-gossip =0.102.0 (iroh-relay waits for
  N4); Cargo.lock adds 170 packages and moves none (checked by package set, not by diff). net Q1
  answered yes on both counts, each with a test: iroh-blobs adds a file by reference
  (`ImportMode::TryReference`, a 1 MiB file grows the store by under 64 KiB), and a `Gate<P:
  ProtocolHandler>` wraps a handler in the router by the connection's remote key. N3 needs no
  replacement; byte budgets will poll `Connection::stats()` in N2. node Q1 (from S59) answered in net
  5.1: `Options.on` false keeps net off, with it true book.json decides. Stand-ins until N2:
  `Options::trusted` for the device list; `Options::bind` sets the bind address; every endpoint has
  no relay, no address lookup and no portmapper. user-sim REJECTED once (the fixture leaked temp
  dirs), S60-fix1 fixed it; the foreman reviewed the 14-line fix itself and did not rerun user-sim.
  Product owner on the Mac: S1 panicked, because it counted sockets through `/proc/self/fd`, which
  macOS lacks; fixed to stat `/dev/fd/<n>` (`5a53ccd`), and S1 still asserts a socket appears once
  net is on, so the count is not vacuous. Then net 37/37 `--include-ignored`, fmt and clippy clean,
  host 153/153 (the cloud cannot bind `::1`; one earlier run had 1 failure under S61's load and
  passed twice since, test not captured), and the workspace builds with the app.
- [x] **S61 A home can be a replica: it reads, verifies and stores with no identity (log L6, dist
  S8).** Branch `slice/s61`. Builds log L6 (log/TECHNICAL.md 5.5 and the L6 row of 9:
  `Log::open_keyless`, `identity() -> Option<PublicKey>`, `drop_identity`, `channels`, `NoIdentity`,
  `HasIdentity`) and dist S8 (dist/TECHNICAL.md 5.12 and the S8 row of 8: `Dist::open_keyless`,
  `identity()` and `fingerprint()` as Option, `drop_identity`, `NoIdentity` on every signing path,
  the holder methods of net/TECHNICAL 3.1, `HeadState`, the record reason `kept` with `Via::Kept`,
  `search_stored`, and the lock rule of RV-06: no read lock and no own-channel lock held across a
  holder call). One slice because both change `identity()`. dist gains no thread and no dependency
  on `net`.

  The Option ripples to every caller: about 37 sites in app/src (core.rs alone has 25), and dist's
  own. Each caller of a home that has an identity keeps its behaviour; a keyless home answers
  `NoIdentity` wherever it would sign. Outcome: a keyless home opens, reads, verifies and stores
  another participant's channel, refuses to sign with `NoIdentity`, and `drop_identity` deletes only
  an identity that signed nothing; every existing journey still passes. Touches: log, dist, app (the
  Option), possibly host and dev. Needs the Mac: the app cannot build in the cloud, and J2 to J5 must
  stay green. S60 is building net in the cloud at the same time and owns the workspace Cargo.toml
  and Cargo.lock: add no dependency.
  Delivered (d1d0ba0, merged through S64 at ccf206d): a home can be a replica that opens with no identity, holds another participant's channel and refuses to sign (`NoIdentity`, 409 `no_identity`). Stopped once on a full disk. Verified on the Mac: log, dist, net, pkg, host, dev, app and J2 to J5 green, except dev's scenario map (main's step 9 spec, below). Stage G leftovers from user-sim: `Core::indexes` fails keyless; `choose_index` reports `NoIdentity` after it worked; `IdentityInUse` text says 'replaced'.
- [x] **S62 Alice pairs her server with a code, and only her own machines are served (net N2).**
  Branch `slice/s62`. Builds net N2 (net/TECHNICAL.md 5.3 to 5.5 and the N2 row of 9): the signed
  device list with expiry, pairing by code (`answer_join` with the short key confirmed), classes,
  and allowances and budgets in bytes per peer on every protocol, polled from `Connection::stats()`
  and enforced through the `Gate` wrapper S60 proved. Removes the `Options::trusted` stand-in: the
  device list decides. Outcome: scenarios S2, S3, S4, S10 and the budget part of S9 pass (net
  FUNCTIONAL 3), each as a test `s<n>_...`. Also takes node/TECHNICAL.md 13 Q4 (`Options` and
  `Limits` derive Clone, Debug and Default) and the N2 part of Q5 (a light machine prefers a full
  replica over the anchor), each answered in net's documents. Touches: net only. Cloud.
  Delivered (ea231f3): one implementer run and a validator pass, plus S62-fix1 from user-sim (a hello pulls a newer device list from a trusted node; pairing codes trim whitespace). Verified on the Mac: workspace build, net all green including ignored, host green.
- [x] **S63 Alice publishes on the laptop, and her server has it within seconds (net N3).**
  Branch `slice/s63`. Builds net N3 (net/TECHNICAL.md 5.6 to 5.8 and the N3 row of 9): sync on
  connect, on a hint and every 10 minutes, over kept connections (ND-13 said N3 keeps them); hints on
  the own iroh-gossip topic; packages over iroh-blobs added by reference, with the size cap and
  `store_blob`. Takes node/TECHNICAL.md 13 Q3 into net 5.7: a light machine acts on a hint only for a
  channel in `holdings.heads(None)`, a full machine for any. The holder is a fake in
  `net/tests/support` (node 1 builds the real one); the `Holdings` trait of 3.1 does not change.
  Outcome: scenarios S5, S6, S7 and the rest of S9 pass (net FUNCTIONAL 3), each as a test
  `s<n>_...`; a forged hint, a changed byte, another author's entry and a package with the wrong
  hash change nothing in the holder. Touches: net only (src, tests, TECHNICAL, FUNCTIONAL); adds
  no dependency (iroh-blobs and iroh-gossip are pinned since N1). Cloud.
  Delivered (d24cf4c): Alice publishes on the laptop and her server has the entry and its package, same bytes, in about 60 ms on loopback; an offline server catches up when it is back. net gains `n0-future` (already in the lock through iroh) to name gossip's Stream, and `Limits::sync_interval`. Blobs are added by reference on request; a fetch holds at most `size` bytes in memory. Node Q3 is in net 5.7. A dropped machine stays dropped for the life of the Net. The query part of S9 moved to N4 (S67). Verified on the Mac: hakari clean, net and node green with ignored, host green, app builds.
- [x] **S64 A home holds another participant's channel through the holder (node 1).**
  Branch `slice/s64`. Builds node stage 1 (node/TECHNICAL.md 9, row 1): the `node` crate as a
  workspace member, `DistHoldings` and `convert.rs`, `Node::open`, `open_keyless`, `dist`, `net`,
  `machine`, `close`, and the wrappers with `net` off. Outcome: node scenarios S1, S2 and S3 pass
  (node FUNCTIONAL), each as a test; a keyless node opens and holds a channel it was given. Touches:
  node (new), the workspace `Cargo.toml` members line and `Cargo.lock`; not net (S63 owns it): a
  gap in net's API goes to the product owner as a question, not a change. Cloud.
  Delivered (ccf206d): the `node` crate opens a keyless home or an app home, and `DistHoldings` answers net's `Holdings` from what dist stored; a keyless node holds a channel it was given. dist re-exports `Grant`, `InterfaceName`, `Provider` so node depends only on dist and net. Verified on the Mac: hakari verify clean, node and dist green with ignored, app and host green (host's one flaky test passed 3 of 3 on rerun). Open for node 2: a `Kept` ingest that fails on a gap leaves its `kept` record; `DistHoldings::set` twice keeps the first.
- [x] **S67 Alice's light laptop searches and installs through her server (net N4, part 1).**
  Branch `slice/s67`. Builds the light-machine and restore half of net N4 (net/TECHNICAL.md 5.9 and
  5.11): `query` with its 32-channel cap, `need`, `need_blob`, `NoFullMachine`, `own_heads`. Takes
  node Q5 (a light machine's `query` and `need` prefer a full replica over the anchor; NET-F-21
  holds for full replicas only) and Q6 (a cap per caller over 5 s, so one participant query pulls at
  most 32 channels), each written into net's documents. Also the query part of S9 that S63 moved
  here, and S63's user-sim follow-ups: a package over `limits.blob` gives an event with its cause
  (NET-F-32); the wrong-hash cause in watane's words; a `Refused` cause names the short key.
  Outcome: net S8, S12 and the query part of S9 pass as tests `s<n>_...`. Touches: net only. No new
  dependency. Cloud. Reach (mDNS, relay, DHT) is S68.
  Delivered (d750ed5, merged as edae082): Alice's light laptop searches through her server and installs from it, same bytes; a restored laptop learns its head as a hint and pulls nothing until she confirms. The anchor syncs as a light machine (node Q5); a cap of 32 pulled channels per 5 s across all queries of a Net (node Q6); `query` answers trusted callers only, at most 200 hits. user-sim rejected once (iroh's words for a package not served), fixed with `regress_s67.rs`. Verified on the Mac: net green in 78 s, node green with ignored, app builds.
- [x] **S68 Two of Alice's machines on different networks connect through her relay (net N4, part 2).**
  Branch `slice/s68`. Builds reach (net/TECHNICAL.md 5.10): mDNS, stored addresses, addresses from
  `devices` answers, the relay of `relays`, the DHT switch, `Unreachable` with NET-F-28's cause, and
  `relay_server` (iroh-relay, pinned `=1.4.1` as net 2 says). First task: bring net's default test
  run well under the 120 s limit (it took 120 s in the cloud after S67, 78 s on the Mac): S67 named
  `support::settle`'s 3 x 300 ms window and the ~3 s `Net::close` / `set_options(off)` with open
  connections. Outcome: net S11 passes; no service
  is reached that the options did not turn on (`no_default_services`). Touches: net, the workspace
  `Cargo.lock`. Cloud; the product owner runs `cargo hakari` on merge. After S67 (both own net).
  Delivered (3527962, merged rebased with its hakari commit 3659eee): two of Alice's machines on different networks fail with "no direct path; choose a relay or turn on the DHT" and connect once both choose her server's relay; machines find each other at stored addresses and at addresses passed on in `devices` answers. net's tests: 121 s to 39 s (the cause was a 1200-entry fixture chain, not close). New dependencies: iroh-relay (server), iroh-mdns-address-lookup and iroh-mainline-address-lookup =0.6.1. Decisions: node Q1 built literally (`book.json` decides on/relays/dht; a fresh home stays off until `set_options`); mDNS only when on and not on loopback; the relay server is plain HTTP and admits only trusted machines (W32), `Net::relay_url()`; at most 4 passed-on addresses per machine, own machines only; a connect that runs out its deadline is `Unreachable`. Verified on the Mac: hakari clean, net and node green with ignored (67 s), app builds. For node 2 (node's documents): the relay URL is `http://<addr>/`, not https; the DHT consent text must say LAN addresses are published too; `watane-node --relay` needs a fixed port; with a relay chosen and the DHT off, the cause still names both.
- [x] **S70 Alice publishes on her laptop and her server keeps it; a light laptop searches and
  installs through it (node 2).** Branch `slice/s70`. Builds node stage 2 (node/TECHNICAL.md 9, row
  2): `told` and `need` in the wrappers, `installed`, `search`, `join`, `own_hints`,
  `confirm_own_hint`, `words.rs`, on net N1 to N4 as built. Outcome: node scenarios S4 to S9 pass
  (node FUNCTIONAL), each as a test `s<n>_...`, between homes on 127.0.0.1 in one process tree.
  Folds in S68's notes on node's documents: the relay URL is `http://<addr>/` (`Net::relay_url()`);
  the DHT consent text (node 7) says local network addresses are published too; `watane-node --relay`
  takes a fixed port (node 3 builds it; write it into the spec now); with a relay chosen and the DHT
  off, net's cause names only what is missing (a one-line net fix is allowed). And S64's two open
  questions, decided by the product owner: a `Kept` ingest that stores nothing (a gap) removes its
  `kept` record (dist 5.12); `DistHoldings::set` a second time is an error, not a silent keep.
  Touches: node, dist (the kept record), net (that one cause text). Cloud.
  Delivered (796c074, merged rebased): Alice publishes on her laptop and her server keeps entry and package within about 250 ms on loopback; a light laptop searches through the server (hits marked with its key), installs from it and gets the next release through it; an unused app home joins as a replica and drops its key; a restored identity takes its head only when Alice picks the server's hint. Decisions: net's Unreachable cause names only what is missing; a failed Kept ingest takes back only the kept mark it added; `DistHoldings::set` twice is `NodeError::HolderSet`; node's search sends one remote query with interface null (dist's `InterfaceQuery` has no accessor), so remote budgets are spent on any interface; S5 uses a 4 s host delay. Verified on the Mac: hakari clean, node, dist, net, log with ignored, host and app green. Found, not fixed (net 5.7): a gossip hint is acted on with the neighbour that relayed it, which may not hold the head yet, so about 1 in 10 updates on a three-machine topic waits for the 10-minute sync; node/tests/it/light.rs reconnects as a commented workaround. user-sim notes: 'own head 1 is not stored' reads wrong for a hint with the right seq and another id; `IdentityInUse` says 'signed statements' after one; an anchor never asks its full machines on fetch, follow or search (NODE-F-9), so it cannot search what only the server holds.
- [x] **S72 Alice runs her server as a program (node 3), and a hint reaches the right machine.**
  Branch `slice/s72`. Builds node stage 3 (node/TECHNICAL.md 6 and 9, row 3): `watane-node` with
  `join`, `run`, `status`, `relay` (a fixed port; port 0 refused) and `dht`, and the output of 6.2.
  Fixes S70's net finding (net 5.7): a gossip hint pulls from the machine that told it, or retries once
  the relaying neighbour holds the seq, so an update does not wait for the 10-minute sync; remove
  node/tests/it/light.rs's reconnect workaround. Outcome: node scenarios S10 to S13 pass on the real
  `watane-node` binary between homes on 127.0.0.1; the three-machine hint test passes 20 of 20 runs
  without the workaround. Touches: node (the bin, tests, docs), net (5.7 and its test). Cloud.
  Delivered (merged rebased as 69954c2): Alice runs her server as `watane-node` (`join`, `run`, `status`, `relay`, `dht`, optional `--bind`); a hint whose machine lacks the seq retries every 250 ms on the next trusted machine, up to 20 times, and the light.rs workaround is gone. Decisions: tokio's signal feature, not signal-hook; `url` a direct node dependency (Cargo.lock: tokio and url in node's list, no new crate); a join stores the anchor's answering address; `Net::options()`; a signal during join exits 1 `interrupted`; a removed machine's `run` points to `join --replace`; S13 uses another author's entries. Verified on the Mac: hakari clean, net and node green with ignored, node's light test 5 of 5, app builds. For a later slice (user-sim wording, about ten items in the S72 report): the key printed twice in causes; `--help` says unknown flag; the y/N prompt and its answer share a line; no confirmation after dht and relay changes; a second run reports the dist lock instead of another watane-node.
- [x] **S75 Alice pairs her laptop and her server from the app, and J8 passes (app stage G).** Branch
  `slice/s75`. Builds app stage G (app/TECHNICAL.md 9.8 and 12, row G) on node stages 1 to 3: turn own
  machines on, the pairing code and the join question with the short key, devices and roles, expiry,
  relays, the DHT switch with its consent text, restore hints; in the app's settings, where S74 puts
  own machines. Folds in S61's stage G leftovers (`Core::indexes` fails keyless; `choose_index`
  reports `NoIdentity` after it worked; the `IdentityInUse` text). Outcome: J8 (`app/tests/it/j8.rs`)
  passes on loopback homes on one Mac, and J2 to J5, J9 still pass. Touches: app (src, ui, tests/j8.rs,
  TECHNICAL 9.8). Local (the app). After S74, which changes the same app/src and app/ui.
  Delivered (merged as e1643b5): Alice turns on her own machines in Settings, pairs her server (`watane-node`) and a second laptop with a code and the consent view, publishes to the server with no static host, searches and installs through it from a light laptop, removes a machine, restores from the server's hint, and adds relays or the DHT only after consent; no mod or local route reaches any of it. J8 passes natively (5 of 5, 52 to 77 s). Step 8 is done. Decisions: net (no format change): after `pair` the joiner pulls `devices` once from the anchor, so a new machine learns the others' addresses (in memory, not book.json; a failed pull does not fail the join; net 5.5); J8's restore keeps the restored laptop as anchor and the server re-joins with `join --replace`; `Status.identity.signed`, and the Join question asks the delete question only for an identity that signed nothing; IdentityInUse reads "has signed at least one statement"; S61 leftovers: `indexes` works keyless, `choose_index` on a keyless home succeeds or changes nothing; tests/support/node.rs. Verified on the Mac: hakari clean; net and node green with ignored; the app driver suite green with ignored, J8 included (front tests not run). For a later slice: a light laptop refused by a server that removed it shows dist's "is not followed" (node/src/calls.rs:66 drops need_blob's Result; net turns not_allowed into Unreachable, light.rs:178); user-sim wording: a reused code says "not a pairing code", "no direct path" also when the anchor is just off (3x), no reconnect after both restart on new ports, the anchor's own row "Last seen: unknown", "anchor · anchor <key>", Join clickable twice before the 2 s refresh, the restore refusal does not point to Settings, no `watane-node --help`, `status` fails "home is locked" while run runs, relay/DHT controls enabled while off, publishing does not warn that the home can no longer join; node/tests/it/restore.rs s9 flakes about 3 in 12 (SiteNotWritten), also on main.

- [x] **S80 Alice's server says what is wrong (node, net).** Branch `slice/s80`. The node and net items
  of S72's and S75's user-sim reports: a light laptop refused by a server that removed it is told so
  (node/src/calls.rs:66 keeps need_blob's Result; net reports not_allowed, not Unreachable, light.rs:178);
  `watane-node --help`; `status` while `run` runs answers instead of "home is locked"; a second `run`
  names the running watane-node; the key printed once in causes; the y/N answer on its own line; a
  confirmation after `dht` and `relay` changes; a reused code says it was used; "no direct path" only
  when there is none, once; node/tests/it/restore.rs s9 flake (SiteNotWritten). Outcome: each item has a
  test on the real `watane-node`. Touches: node, net (no wire or disk format change). Cloud.
  Delivered (merged as 5e2e12f): watane-node says what is wrong: a removed light laptop is told "node <short> refused: not_allowed"; `--help`; `status` answers while `run` runs; a second run, dht, relay or join names the running watane-node; each key printed once; the y/N answer on its own line; dht and relay changes confirmed; a used code says it was used or expired; "no direct path" only with no known address, else "did not answer at its n known address(es)"; restore.rs s9 fixed (a test race). Decisions: no wire change (the joiner maps the anchor's bad_code to CodeSpent when the code parses); need/need_blob return Refused{NotAllowed} (iroh-blobs' 403 mapped to it), busy/over_budget unchanged; nested causes drop the inner node name; a kept refusal replaces only dist's missing-source errors, as DistError::Io { path: "the server" }; `Machine::of(&Net)`. Verified on the Mac: hakari clean; node green with ignored; net green with ignored on reruns (reach ac4 failed once under S81's build load, then 5 of 5 alone and 3 of 3 as a file); app J8, machines and J3 green. For a later slice: a refused fetch still waits dist's package_timeout before reporting; the chain's top line is just "the server"; join prints "waiting for your answer" before the code is checked; a join answered yes after its joiner died adds a machine that never joined (a net check); `relay remove` of an unset relay is silent; `watane-node bogus` asks for --home; "relative URL without a base" is parser jargon.
- [x] **S81 The app says what it means (app wording).** Branch `slice/s81`. The app items of S74's,
  S75's and S77's user-sim reports: the library marks the main mod; the consent for a mod proposing
  main says it becomes the start window; a connection link moves focus to the target's details; the
  disable warning words an Extended-by link; the anchor's own row and "anchor · anchor"; Join not
  clickable twice; relay/DHT controls disabled while off; publishing warns that the home can no longer
  join; the restore refusal points to Settings; Desk's call during a grant change gets an answer;
  `machines.rs ac3_s30` waits for its full text; the app's own temp-folder leaks (11, S76). Outcome:
  each item has a test on the real app. Touches: app. Local (the app).
  Delivered (merged as 6788eb6): the Library marks the main mod `Opens at start`; the install consent says when a mod becomes the start window; a connection link moves focus to the target's details; the warning separates `These mods use it` from `These mods extend it`; no "anchor · anchor" or "Last seen: unknown" for this machine; Join cannot be clicked twice; relay/DHT controls disabled while own machines are off; the first signature warns the home can then no longer join; a publish refused after a restore points to Settings › identity › From your machines; Desk gets `page_reloaded` for a call cut off by a grant change; ac3_s30 waits for its full text; no app test leaks temp folders. Decisions: `PlanMod.becomes_main` reads `Mod::main()` from host's staging layout (a `main: bool` on host's PlanEntry later); the restore refusal is dist's `OwnHeadUnconfirmed`, mapped in `From<DistError> for CmdError`; S81's built words sit at the end of app/TECHNICAL 5.3 for S84 to carry into the design, and S84 decides the extend wording of 5.3.4 item 5. Verified on the Mac: hakari clean; the app driver suite green with ignored (front tests not run). For a later slice: a host-side reload notice for a manager's in-flight call; the consent says "uses menu" for a mod that only extends it; the anchor's own row reads `Last pull: unknown`; a hidden anchor lists a new replica late and the replica shows the anchor offline right after pairing; the restore refusal names an empty From your machines while own machines are off, and "own head" is jargon; `Give back` on manage needs no consent (check against W35).

- [x] **S82 Desk opens a page, asks to read a new link, and Alice stops its asks (Q30 b, Q31 b, Q32 b).**
  Branch `slice/s82`. Builds the human's answers: host 6.10 gains `open-page(local)` (a direct call:
  the app opens or focuses that mod's window, never while a consent is open) and dev's manage fake
  with it; `request-install` with a link of a channel the home does not read shows a first consent
  "<mod> asks to read <link>" before the install's own; the consent view offers "Deny and stop asks
  from <mod>", which withdraws `Manage`. Specs: host 6.4, 6.10, 18 (answered); app 17; dev 8.1.
  Outcome: tests for each on the real app and host; J10 and S43 to S48 still pass. Touches: host,
  dev, app. Local (the app). After S81 (app).
  Delivered (merged as d3cdfed): Desk opens or focuses a mod's page, never over an open consent; installing from an unread link first shows "Desk asks to read <link>"; every consent a manager starts offers "Deny and stop asks from Desk", which withdraws Desk's manage grant. Decisions: Q31 rides `Reduction::OpenPage(LocalName)` through `Confirm::reduced` (no new Confirm method); `open-page` fails `no_mod` for an unknown mod, `bad_request` for one off or without a page; the app drops an OpenPage while a consent is open (no queue) and ignores one for a mod gone off; Q30 uses `node.read_link` (no follow; spec 17.3 corrected from `open_link`), the channel is followed only after the install succeeds; `consent_answer` takes an optional `stop` (deny, then `Core::grant(local, "manage", false)`), `stop` without asked_by is WrongAnswer, the button is not under the 800 ms rule and never has focus at load; the 15 short host.wit fixture copies are now the full HOST_WIT (shelf and recs hashes changed, checked independently). Verified on the Mac: hakari clean; host, dev and the app driver suite green with ignored (front four skipped), again after rebasing on the design revision 2c820c3. For a later slice: after "Deny and stop" the details show Manage granted until the window gets focus, and a manager install shows in the Library only after focus (refresh-on-focus); Desk cannot tell an open-page was dropped; focusing an open window was not checked (front mode).

- [x] **S83 The manager's look is specified: the human's design becomes the app's spec (W37).** Branch
  `slice/s83`. The human chose (2026-11-16) to build their mod-manager design
  (`spec/design/manager/*.dc.html`, the launch rule and design rules in its `canvas.json` notes). A
  demo found the app has nearly every function but none of the look: app/TECHNICAL 5.3 lists elements
  and ids but no layout (`spec/design/manager/COMPARISON-2026-11-16.md`). A design slice: app/TECHNICAL
  5.3 and 5.4 (and FUNCTIONAL where words change) specify the views as drawn: tiles with monograms and
  badges, a side panel for details with grant switches and words in plain language, one search box and
  index chips in Find, cards in Updates with Review…/Not now and Check now, the first-start view, tokens
  for light and dark (prefers-color-scheme), the app menu's separator; where the design and an existing
  APP-F rule disagree, the rule wins and the spec says so. Also: a `main = true` mod installed through
  `/api/install` becomes main like the app's own installs, and the consent view says an install makes
  a mod the start window. Each change gets a test row for S84. Docs only. Cloud.
  Delivered (merged as 1941864): app/TECHNICAL 5.3.1 to 5.3.7 and 5.4 specify the manager as the design draws it (tokens light and dark under prefers-color-scheme; header with tabs and one Add mod popover; Library tiles with monogram and one badge Off > Problem > Update > Main; a side panel per mod with grant switches in plain words, connection chips, an inline confirm panel; Find with one box and index chips; Updates cards with Not now / Review…, Checked / Check now; the first-start view; menu separators), with test rows (section 11, stage K in 12). Rule wins over design, each said in 5.3: system-ui font (the CSP refuses Google Fonts), the two-word mark, Make/Unset main, Made from this, a drop button on index chips, followed channels stay in Find, whole failure causes, the window title's party label. Decisions: a short grant form in the details only (`GrantLine.short`), the consent keeps the exact words; one search box maps interface-shaped words and party names; dialog ids become #confirm-panel/-cancel/-continue (window tests change selectors only); Not now is page memory, `Status.checkedAt` and `ChannelRow.readAt` app memory; an update never makes a mod main; Edit and Window menus with Tauri's predefined items; monogram colour by slug sum mod 8; driver ops `menu_items` and `theme`; Find's "Adds commands to <main>" line left for later.
- [x] **S84 The manager looks like the design (app).** Branch `slice/s84`. Builds app/TECHNICAL 5.3
  and 5.4 (S83, and the human's revision of 2026-11-16, 2c820c3): the views, tokens and dark mode, the
  first-start view, the Installed note after an install (APP-F-78), `Checking…` and the drop target
  (APP-F-79), the library order (APP-F-68), the main-mod rule for `/api/install`. Build from the
  originals: `spec/design/manager/*.dc.html` and `canvas.json` (the human's files; `Installed.dc.html`
  is new). The renders `render/1-5*.png` predate the revision: where they differ, the `.dc.html` wins.
  The spec says where a rule wins over the design. Existing ids and driver tests keep working, except
  the selector changes 5.3 lists. Outcome: the test rows of section 11 (stage K) and S49 to S52 pass;
  a screenshot of each view sits beside the design, and every discrepancy is (a) a rule that wins
  (5.3) or (b) just as good, with its reason; the PO checks this side by side before the merge
  (the human, 2026-11-16). Touches: app (ui, src, tests), host
  (`Confirm::installed`, host 12.1). Local (the app). After S81 and S82.
  Delivered (merged as 26a728a): Alice gets the manager as the human's design draws it, light and dark: first start, the Library grid with a details side panel, the inline confirm panel, the Installed note, the drop target, Find, Updates with Checking…, Settings in the same tokens; a `main = true` mod installed any way (watane-dev install, a managing mod) becomes the start window (host `Confirm::installed`). Side by side: three foreman rounds (about 30, then 7 gaps, fixed in fix1 and fix2), then the PO's own check of all 11 views found one more (the update band said "An update is available." with a full-width button), fixed in fix3 and fix4; every remaining difference is (a) a rule (two-word mark, monogram by slug, offerLabel in Updates, Search button, chip drop, "Adds commands to <main>" not built) or (b) just as good (selected tile outlined, 400 px panel, system-font wrap, Try again disabled while checking). Screens in spec/design/manager/screens (the app's live DOM re-rendered in headless Chromium: macOS refuses screen capture). Decisions: three app tasks (Rust; Library and shell; Find and Updates); look.rs renders dist states through the page (WATANE_APP_FAKE gone); the spec follows the board where no rule decided (start row stays during confirm, first-start controls 40/10, Installed for any version, "A, B and C are installed.", zero channels "Checking…", no badge in the details head); one primary per Settings section; grant_short keys providers by identity; drops reach the page's wataneDropped and Rust only toggles body.dropping; `Confirm::installed` only from Host::install after commit_plan, not when all entries are Unchanged; the details band reads `Version <v> asks to <grants>.` with an `Install <v>…` button (PO, 5.3.4.3), Updates keeps offerLabel. Verified on the Mac: the foreman's whole suite once (host, app default and driver, front skipped); the PO reran j4 and gui_shell (they read the band) after fix3/fix4. For a later slice (user-sim): a program's install shows in the open Library only after focus and leaves the earlier Installed note; a new home flashes the full Library before first start; a search with no index says only "No mods found"; the consent view still shows the long technical grant lines; manager.rs ac5_s81 reads TECHNICAL/FUNCTIONAL (spec grep, remove).

- [x] **S85 The app's suite is green on Linux (stage F), so app slices can run off the Mac.** Branch
  `slice/s85`. The human agreed (2026-11-17) after the test-time measurements. `app/scripts/linux-test.sh`
  (Docker arm64, xvfb, Openbox) runs the whole app suite with the front four skipped; 4 tests fail on
  Linux only: `j10` ac3_journey_j10 (DOM timed out: `body.dataset.ready === 'true' && location.hash ===
  '#mods'`), `s44` ac2_s44_disable_closes_page_and_restart_clears_notice (`body.dataset.ready`), `s39`
  ac3_s39_menu_and_last_window_exit (`location.hash === '#settings'`), all three in
  app/tests/support/journey.rs:14, and machines.rs:115 ac1_s28_off_by_default_and_publish (NotFound, a
  path that exists on macOS only). Find each cause (a real app bug on Linux, or a test that assumes
  macOS) and fix it at that level; never weaken a test or skip it on Linux without a reason that
  names what Linux cannot do. Outcome: `bash app/scripts/linux-test.sh` passes with 0 failures twice
  in a row; the native driver suite stays green. Touches: app (src, tests, scripts/linux-test.sh,
  linux.Dockerfile, NOTES.md). Local (needs Docker and the Mac's native run). No user-sim story
  beyond the two green runs. Next, not in this slice: a cloud environment with WebKitGTK and xvfb
  (the human's).
  Delivered (merged as 0277e15): `bash app/scripts/linux-test.sh` runs the whole app suite green (436 tests, twice in a row), so app slices can run off the Mac. Causes: a tauri-runtime-wry 2.12.3 Linux bug (protocols registered once per data_directory key; an incognito mod window built first left main without `tauri://`), fixed by giving mod windows the key `<home>/app/mod-webview-context` (still ephemeral; gui_isolation passes); no `lsof` in the image; GTK mnemonics in predefined menu labels (now `*_with_text`); a hidden GTK window has no size (one look test uses `App::background`); a race in manager ac3_s81 (waits for `#installed-note`). Decisions: WebKit leaves bookkeeping files in mod-webview-context (TECHNICAL 3, 5.1). Verified on the Mac: rebased on the Google style commit e8265e8, app clippy, fmt, default and the touched window files green; manager.rs failed 3 of 6 at first, but the foreman showed main failing the same way when both ran at once under load (load average 11 to 24), and the branch 7 of 7 alone. Not run after the rebase: the Linux runner (the rebase changed only style).

### Step 9: Extension points (J9)

The human decided (2026-11-15) that step 9 runs beside step 8: its files are disjoint from net and
node. Designed in 51e0dfb (W34): pkg 4.5, host 4.2, 6.4, 6.6, 9.4, dev 8.1, app 4.

- [x] **S65 A shell mod collects the commands of the mods granted to answer it (pkg 9, host 10, dev 12).**
  Branch `slice/s65`. Builds pkg stage 9 (the outlet-function rule and `World::collects`, A26 and
  A27), host stage 10 (the collect of 9.4, the outlet row of 6.4, collect edges in the cycle check
  of 4.2 step 8, the 5.3 rule, collect fakes in `Harness`, the fixtures `menu` and `item-*`), and dev
  stage 12 (collect fakes and `deliver` answers in contract tests, the template `AGENTS.md` lines).
  Outcome: host S36 to S39 pass (`host/tests/it/collect.rs`); pkg A26 and A27 pass; DEV-S-52 passes as
  `contract::ac_collect_fake_and_answer`, which turns dev's scenario map green on main; a maker's
  agent running `watane test` on `menu` with and without a fake sees the lists of DEV-S-52.
  Touches: pkg, host, dev (src, tests, fixtures, their TECHNICAL where a decision lands). Not app:
  the grant words and J9 are S66. Cloud. The human has uncommitted edits in pkg/FUNCTIONAL.md and
  pkg/TECHNICAL.md: change those two only where the slice must, and say so in the report.
  Delivered (1462015, merged as 7d58990): a shell mod calls a collect on its own outlet and gets the joined answers of the mods granted to subscribe, in local-name order, without learning who answered; a maker's agent fakes a collect in `watane-dev test` and checks an answer with `deliver`. Dev's scenario map is green again. Decisions: a failed answer's fuel stays spent (else N failing subscribers spend N budgets; host 9.4); the fixture world is `shell` (wit refuses a world named like its outlet); dev needed no src change. Verified on the Mac: pkg, host (with the ~90 server tests the cloud cannot run), dev, app and J2 to J5 green; dev's `ac10_endless_test_stops_with_exact_message_within_deadline` failed once under load (7.0 s) and passed on rerun. For S66 from user-sim: the template should say to rename the world when an outlet is named like the slug (the guide is at its 150-line cap); a misspelled fake function is silently ignored; `check --json` lists a collect outlet under `events`; `add`'s next hint does not say that exporting the outlet answers a collect.
- [x] **S66 Alice extends a shell with two command mods (app stage H, J9).** Branch `slice/s66`.
  Builds app stage H (app/TECHNICAL.md 12, row H): the step 9 text of the subscription row in
  `app/src/words.rs` ("Receive the {interface} events of {provider} and answer its requests"), and
  `app/tests/it/j9.rs` with host's fixtures `menu` and `item-*`. Outcome: J9 passes as spec/FUNCTIONAL
  writes it (the plan shows the answer grant; the shell shows both commands without learning who gave
  which; a failing command mod is left out; a withdrawn grant removes its command at the next
  collect; a command mod that also calls the shell is refused at the plan, naming the cycle); J2 to
  J5 still pass. Touches: app/src/words.rs, app/tests/it/j9.rs, app/TECHNICAL.md. Local (the app).
  S71 (local) owns dev/ and app/tests/it/j5.rs.
  Delivered (1259216): J9 passes. Alice installs a shell and command mods; the plan reads "Receive the {interface} events of {provider} and answer its requests"; the shell shows the commands without learning who gave which; a failing mod is left out; a withdrawn grant removes its command at the next collect; a cycle is refused at the plan. Decisions: the shell page is an app test fixture (`app/tests/fixtures/menu-ui`) over host's `menu`; `clicked` is an event every granted command mod receives; app gains redb 4.5 as a dev-dependency only, to read a mod's state after quitting; `factory/impl.sh` adds `CARGO_TARGET_DIR` to Sol's writable roots, so Sol can build into main's shared `target/`. Verified natively on the Mac by the foreman after its rebase on main: app suite, J2 to J5, J9 green; hakari clean. Later (user-sim): the cycle error reads like an internal message and does not say what to do; the mod list does not show which mods add commands to which, or that one answered nothing.

  **Step 9 is done:** J9 passes and pkg 9, host 10, dev 12 and app H are done.
- [x] **S69 A maker's agent is told how collects work (dev, S65's leftovers).** Branch `slice/s69`.
  From S65's user-sim: (1) the template `AGENTS.md` says to keep `world <slug>`; an outlet named like
  the slug then fails with "duplicate item named <slug>" and the fix text does not help: say to
  rename the world, in the guide (at its 150-line cap: make room) and in that error's fix; (2) a
  misspelled fake function in a contract file is silently ignored: refuse it, naming the function and
  the outlet's functions; (3) `check --json` lists a collect outlet under `events` only: add
  `collects` beside it (additive); (4) `add`'s next hint says that exporting an outlet is how a mod
  answers its events and collects. Outcome: a maker's agent that writes S65's `menu` and `item` from
  `watane-dev new` gets each of the four right on the first try. Touches: dev only (src, template,
  tests, TECHNICAL). Not `share.rs` or the publish CLI (the `--owner` question is open). Cloud.

### Step 10: Own manager (J10)

Specified by the human in 53ec11d (W35, W36). W35 is sketched: the authority split and the grant
`Manage`; its WIT, request forms, consent words and J10's tests wait for a design slice
(spec/TECHNICAL 9). The main mod (W36) is not a step: pkg stage 10 and app stage I, buildable now.

- [x] **S73 Step 10 designed: the management interface (W35).** Branch `slice/s73`. A design slice:
  `watane:host/manage` in WIT (what a managing mod sees: installed mods, grants, links; what it does
  alone: disable, withdraw a grant, close a page; what it only asks for: install, update, grant, a pick
  of a provider, a removal that deletes data), how a request reaches the consent view and how its
  answer comes back, the `Manage` grant in pkg, the host's checks, a fake manager in dev's contract
  tests, the consent words, and J10's tests; each subsystem's build-plan row for step 10, sized for
  slices. Outcome: a reviewer can build step 10 from the documents alone; every open question is
  either decided in the documents or listed for the human with options. Touches: spec/TECHNICAL.md,
  spec/FUNCTIONAL.md (J10 only), pkg, host, dev TECHNICAL and FUNCTIONAL; app/TECHNICAL.md only in a
  new section at its end (S74 edits app/TECHNICAL 5.3 and 5.4). Docs only. Cloud.
  Delivered (merged as 1153b15): step 10 can be built from the documents. host 6.4 (the WIT of `watane:host/manage`), 6.10 (reads; three reductions: disable, withdraw, close-page; requests: install, update, enable, grant, remove, each an id with `request(id)`; effects wait in the outbox and the grant is checked again before each; one open request per manager), 12.1 (`Confirm::{manage, reduced}`, `Requester::Mod`); pkg 4.5, 5.1, 7.2 (`Grant::Manage`, CBOR `[4]`, grant limit 82); dev 8.1 (a manage fake, `manages`); app 17 (words, consents, S43 to S48, j10.rs; APP-F-70 to 75 sit in 17.1 until S74 merges). Build rows: pkg 11, host 11, dev 13, app J. Validator PASS in round 1. Six questions for the human (host/TECHNICAL 18, Q28 to Q33), each built so either answer keeps the WIT.
- [x] **S76 A mod manages Alice's mods, and asks for anything more (pkg 11, host 11, dev 13).**
  Branch `slice/s76`. Builds the rows S73 wrote: pkg 11 (`Grant::Manage`, `World::manage`), host 11
  (`watane:host/manage`, host 6.10 and 12.1; S40 to S47), dev 13 (the manage fake, DEV-S-53).
  Outcome: host S40 to S47 and DEV-S-53 pass; every earlier suite passes. Built as designed (the
  options marked "built" in Q28 to Q33); a change from the human's answers comes as its own task.
  Touches: pkg, host, dev. Cloud. After S71 (dev).
  Delivered (merged as 797cd9f): a manager mod (grant `manage`) reads every installed mod, disables one, withdraws a grant or closes a page at once, and asks the participant for an install, update, enable, grant or removal as a request whose state it can read; a maker's agent fakes `watane:host/manage` and checks `manages` in a contract test (DEV-S-53); dev's envelope ac5 is green again. Decisions: Q28 to Q33 built as option (a); `Confirm::manage` runs on its own thread with a small current-thread tokio runtime (app J: run runtime-bound work on the app's own runtime and await it from there); a request-* in a preview or a home with no app answers `refused` before any other check; harness request ids r1, r2, ... count across one Harness, a failed call takes no number; fixed a deadlock in `Host::remove` (catalog lock held while sending the catalog message); `Grant` keeps Display only; app gains `GrantKind::Manage` and the words of app 17.2. Verified on the Mac: hakari clean; host, dev and the app driver suite green with ignored (front tests not run), the cloud's not-run tests included.
- [x] **S77 Alice's own manager asks, and she approves in the consent view; J10 passes (app stage J).**
  Branch `slice/s77`. Builds app stage J (app/TECHNICAL 17.2 to 17.4, row J): `HostConfirm::{manage,
  reduced}`, the consent page's `manage` request and `asked_by` line, APP-F-70 to 75, the fixture
  `desk-ui`, `tests/manage.rs` and `tests/j10.rs`; S76's note: runtime-bound work runs on the app's own
  runtime. Folds in Q31 (b) and Q32 (b) if the human takes them. Outcome: app S43 to S48 and J10 pass;
  J2 to J5, J8, J9 still pass. Touches: app (src, ui, tests/manage.rs, tests/j10.rs, fixtures,
  TECHNICAL 17). Local (the app). After S75, which changes the same app/src and app/ui.
  Delivered (merged as ad2a6cc): Alice lets a managing mod (Desk) turn her mods off at once, and every install, update, enable, grant or removal it asks for waits in the consent view, which names Desk as the one asking. J10 passes; step 10 is done. Decisions: Q28 to Q33 built as option (a), Q31 (b) and Q32 (b) not built; the notices `Turned off by <asker>` and `Grant withdrawn by <asker>` live in memory, cleared on enable or restart, and a release notice wins over them; Desk's requests run their dist and host work on the app's own runtime and are aborted if host drops the request; `manage_outcome` turns any host `refused` into `Denied`; APP-F-70 to 75 moved into app/FUNCTIONAL (Step 10), row J into app/TECHNICAL 12. Verified on the Mac: hakari clean; the app driver suite green with ignored, J10 included (front tests not run). For a later slice: withdrawing or giving back Desk's manage grant reloads its page, and a call Desk makes at that moment gets no answer; `machines.rs ac3_s30` (S75) is flaky (compares the identity text before its second sentence updates).
- [x] **S79 Bob installs from a site published minutes ago, and a new base sticks (dist).** Branch
  `slice/s79`. Builds the S54 deputy decision: dist retries an HTTP 5xx twice with a short backoff within
  the fetch timeouts (4.5), and a link's base for a known channel is appended to its record's bases
  (5.9). Outcome: a test server that answers 503 then 200 installs; one that always answers 503 fails
  naming the URL after three tries, within its deadline; a follow through a link with a second base
  reads both bases. Touches: dist (src/http.rs and the follow path, its tests, TECHNICAL 4.5, 5.9). Cloud.
  Delivered (merged as 7ca2e54): Bob installs from a site that answers 503 for a while (a 5xx is tried twice more, 250 ms then 500 ms, inside the read's timeout); a link with a new base for a known channel adds that base, and refreshes read every base. Decisions: only 5xx is retried, for every GET, and no retry starts past the deadline; follow, choose_index and open_link learn a link's base, read_link (preview) learns nothing, and a base whose verifying read fails is not learned; at most 4 bases (a fifth drops the oldest learned after the front one); a re-follow through a known base changes nothing (it used to overwrite `base`); follows.json stays version 1 (dist 4.4, 4.5, 5.9, DD-41, DD-42, DIST-F-61, 62). Verified on the Mac: hakari clean; dist and node green with ignored; app J2 to J5, J9 and find green. For a later slice: a 503 then a timeout reports only `request timed out`; a newly learned base is read twice (one extra head GET); a reused Dist waits the whole entry_timeout on a dropped keep-alive test host.
- [x] **S78 A full test run leaves nothing behind in the temp folder.** Branch `slice/s78`. Tests leaked
  186 GB of `.tmp*` folders on the Mac (Friction, S76): dev's build caches (`build`, `cargo-home`,
  `counter`: 359 folders), copies (124), and a few from host, net and dist. Find what keeps each (a
  `TempDir` kept or leaked, a child killed before its drop, a cache that defaults to a new temp folder)
  and fix it; a test that must keep its folder says why. Outcome: the number of `$TMPDIR/.tmp*` folders
  is the same before and after a full run of pkg, host, dev, log, dist, net and node (cloud: the tests
  that run there; the Mac checks the rest at merge). Touches: test support and the temp-folder code of
  pkg, host, dev, log, dist, net, node; not app (S54, S75 own its tests; the app leaks 11). Cloud.
  Delivered (merged as 94c6ecd): a full run of pkg, host, dev, log, dist, net and node leaves `$TMPDIR` as it was. Only dev leaked (about 2.4 GB a run): the warm template cache sat in a static `OnceLock<TempDir>` (now at `<CARGO_TARGET_TMPDIR>/watane-dev-warm-<binary>`, rebuilt at first use per run, removed by cargo clean), and a builder test's child exited with its TempDir alive. Guards: dev/tests/it/regress_s78.rs and a check in the builder unit test. Verified on the Mac: dev and host green with ignored, 0 entries left in a private TMPDIR. For a later slice: dev's real-program tests panic on a missing WATANE_BIN; pkg `ac6_round_trip_and_changed_package` races the ignored `regenerate_fixtures`; the app's own leaks (11 folders).
- [x] **S74 Alice's shell opens at start (the main mod: pkg 10, app I).** Branch `slice/s74`. Builds
  pkg stage 10 (`main = true` in `mod.toml`, `Mod::main`, A28, A29) and app stage I (app/TECHNICAL
  5.3 and 5.4: the app opens the main mod at start or the manager; the participant makes any mod with a
  page the main mod and unsets it; a mod that proposes itself becomes main when installed and none is
  set; "Mods…" in the app menu opens the manager; settings hold identity, agents and own machines).
  Outcome: app S37 to S42 pass (`tests/manager.rs` with the fixture `desk`); every earlier suite
  and J2 to J5, J9 pass. Touches: pkg (meta.rs, its tests, TECHNICAL 4.2 if a decision lands), app
  (src, ui, tests/manager.rs, TECHNICAL 5.3, 5.4). Local (the app). S71 (local) owns dev/ and
  app/tests/it/j5.rs; S72 (cloud) owns node/ and net/; S73 (cloud) owns the docs above.
  Delivered (merged as a2bf077): Alice's main mod (one she picks, or one that proposes itself with `main = true`) opens at start in place of the manager; Mods… and Settings… in the app menu open the manager (Library, Find, Updates, details with connections and a warning before disable or remove) or Settings (identity, site, backup, agents, machines). Decisions: the app reads `Mod::main()` with `Mod::read(<home>/mods/<local>)` (read only, false on error), host's `InstalledMod` unchanged; the fixture desk is page-only; the driver op `menu` takes `item`, not `id` (collides with the request id, TECHNICAL 10); pkg: `main = true` with a symlinked ui/ gives only the path violation; j2 ac7 goes through the APP-F-67 warning; Settings shows own machines as "Unavailable" until S75. Verified on the Mac: pkg green with ignored; the app driver suite green with ignored (front tests not run), j2 and manager on a rerun after S54's build swapped `watane-app`. For a later slice (user-sim): the library does not mark the main mod; the consent for a mod that proposes main does not say it becomes the start window; a connection link does not move focus to the target's details; the disable warning says "These mods use it" when only an Extended-by link remains.

### Step 11: The agent layer (agentic-harness stages A to D), before step 6

The human chose (2026-11-21, "the bridge") to build the agent layer next, driven by the collection's
product owner (watane-mods `/po`, `factory/config.toml` here; watane keeps its own foreman, agents,
check.sh and CI). The design lives in `watane-mods/agentic-harness/design/` (VISION, FUNCTIONAL,
TECHNICAL; N1 to N11 and R1 are the human's answers there). Its TECHNICAL 15 is the checklist of
watane changes the human accepted (N3); TECHNICAL 17 gives each stage its watane part and its
agentic-harness part. A stage's watane slice goes first; the agentic-harness slice that uses it
names the same interface in its sketch and fakes it until this one merges.

- [x] **S95 The agent layer in watane's contracts (agentic-harness TECHNICAL 15, N3).** Delivered
  2026-11-21 through PR #7, merged 08:54 UTC (routine `trig_01PGTXRrSDbTaG2nkiMxLDd3`, branch `slice/s95`). A watane
  foreman can build stage A (host A1: H1 streams, H5 durable requests, H7 timers, page connections;
  pkg stage 12) from watane's own documents; every TECHNICAL 15 row is applied or named as left out
  (`caller.boot` Q34, `outcome` on `answered` Q35, the 4.5 host-item rules Q36). The agent layer is
  W38, its boundary W39, app stage L; host 19, app 19, dev 18; host stages A1 to A5; grants
  `Grant::ClaudeCode`, `Grant::ClaudeCodeShell` (CBOR [5], [6]); licence MIT OR Apache-2.0 in every
  crate. User-sim ACCEPTED after fixes it wrote; adversary BREACHED 5 then 4 (S61 to S68), each closed
  in the documents as a rule plus a FUNCTIONAL scenario the building slice turns red first. Six open
  security questions for the human in host 18 (Q38 to Q43), none blocking A1; Q37 open.
- [x] **S96 Stage A host half (host A1, pkg 12).** Delivered 2026-11-21 through PR #10, merged ~15:50 UTC
  (routine `trig_01G5BG7xm1TfA3JsHSBHzs8J`, branch `slice/s96`; 08:58 to 15:30, 6.5 h). A mod opens a stream
  and pushes numbered items to one consumer (a mod or its page's `/events`), in order, live, with
  `taken`/`defer`/`resume`, backpressure, the lag clock, byte budgets; requests and timers act only after
  a durable commit; timers survive a restart; `caller.identity()`. User-sim ACCEPTED on `watane --preview`
  with a page in headless Chrome (cases 20, 51, 58 host parts; 7, 21, 36 as host tests; stage A budgets);
  adversary BREACHED 3/4/3/1 over 4 passes (origins of removed mods, revoke and update races, install
  generations, remove's authority tail), all fixed with red tests in `host/tests/it/adv_s96.rs`, HELD on
  pass 5. V8: p95 3.25 ms (no write), 6.36 ms (one write per batch, writer busy 24.5%). Implementer
  runs 1/2/2/2/1/2 (Claude stand-in: the Codex relay was down). Suites: host `it` 40-46 s, with ignored
  144 s; pkg 5.2 s; dev 42.6 s. CI: Windows red (known, watane-log), the rest green.
  Friction: no IPv6 loopback in the cloud (4 `[::1]` tests run only in CI); no `cargo deny` there; the
  fixture build is not byte-reproducible (`logic.wasm` rewritten on every rebuild); scratch branches
  `slice/s96-2`, `slice/s96-adv` left on the remote (delete refused).
- [x] **S98 The macOS CI job is green again (guard).** Delivered 2026-11-21 through PR #8, merged 14:29 UTC
  (local foreman, then cloud routine `trig_01FuwkKwLpw5dh9JGrGdvM6P`, branch `slice/s98`). 5 workflow_dispatch runs in a
  row green on lint, ubuntu and macos-26 (runs 33, 35-38). Causes: a quit deadlock (main thread blocked on
  `served.shutdown()` while tokio workers waited on the main thread for `tauri://` assets; shutdown now runs on the
  runtime with `prevent_exit`, app TECHNICAL 3.7); polling helpers treat a failed eval as "not yet"; the look test
  renders its data in the same JS turn as the check; `#approve` enabled and clicked in one turn; node's bad-copy
  test corrupts exactly `dist/packages/<id>`, not a same-size temporary. Windows: the build.rs panic was the
  missing `app/icons/icon.ico` (added); Windows now stops in `watane-log` (`std::os::unix` file modes, 13 errors,
  Verification boundary: its own slice if ever wanted). No user-sim (CI state) or adversary (no boundary). 4.5 h.
- [x] **S97 The human's answers to Q34 to Q36 in the contracts.** Delivered 2026-11-22 through PR #11, merged by
  the PO with `--admin` (routine `trig_01AYz5xREjDp9cnjUabKdpJP`, ~1.5 h). S99's foreman finds `host-info.generation()`,
  `ask-resolved {by: answered{outcome} | expired | cancelled}`, `wrapped` claude lines, R7 (producer-side ends only:
  held items delivered before `ended`; consumer ends, `gone`, `lagging` and the backstop drop them, keeping
  HOST-F-68), the `connection` replaced message (prose under 8.5; S99 changes `watane.js`) and dev's `host.wit`
  rewrite as intended. User-sim ACCEPTED after two wording fixes; no adversary (docs only). For S99: the `caller`
  doc comment lists `host`; `watane-dev check` rewrites `host.wit` silently and the template's AGENTS.md does not
  say to commit it; whether `idle-s` runs while a deferred stream is not reading; stdin on `cancelled`; `changed`
  fires before the new `connection` after a reconnect. Friction: no IPv6 in the cloud (74 host tests fail there).
  Implementer runs 1/2/1 (Claude stand-in). Original sketch:
  The human decided (2026-11-21): restart detection is a new interface `host-info` with
  `generation()` (in place of `caller.boot()`, Q34); `answered` carries `outcome` (Q35); the host
  closes a check with `ask-resolved {by: expired}` at its `expires-at`, and the 4.5 host-item rules
  of agentic-harness TECHNICAL 4.5 and 9.8 apply (Q36). Write each as decided in host, pkg and the
  stage A2/A3 rows. Also from agentic-harness S32: host 19.2 says whether a provider stream that ends
  while its consumer holds a deferred batch delivers the held items before `ended` (R7); and host
  8.5's step-11 `watane.js` lets a page learn that its `/events` connection was replaced (today it
  keeps the `connection` message to itself). Docs only. Cloud.
  For a dev leftover: `watane-dev serve` overwrites a mod's tracked `wit/deps/watane-host/host.wit`
  on every run (agentic-harness S32).

  For stage B's host slice (H2), from agentic-harness S31: the host sets Claude's
  `allowUnsandboxedCommands: false` explicitly (Claude's default is `true`).
- [x] **S99 (routine `trig_01SRQRzaH2pwAAJuwqnW84TT`, cloud) [PO, 2026-11-22: DONE, PR #13 merged with --admin at c3ecb9da. Same-chain gap closed by s99-fix3; user-sim ACCEPTED at Host level; adversary HELD after 4 passes. CI-fix continuation `trig_01Jzmn7LUPenFwgMqFS9d3zy` (s99-fix5, tests only) made ubuntu, macos and lint green; Windows red as on main (watane-log, factory/README.md 'Windows', continue-on-error). check-main.sh's timing tests not run (nothing runs on the Mac).] [PO, 2026-11-22: agentic-harness S35 delivered against slice/s99 at 5471b16 and found a gap S99 must close before it merges: a `claude-code` stream started in a call chain is unknown to `streams.hand-off`, `streams.cancel` and `claude-session.say` in that same chain (`view_of` in `host/src/agent/mod.rs` knows only `Effect::Open`; `session_call` checks only `hub.info`), while agentic-harness TECHNICAL 10 has the unchecked provider start, say and hand off in one call. Without it a start fails `bad-launch: stream: unknown-stream`. If S99 delivers without it, a continuation adds it.] Stage B, end to end: Alice has a first conversation with Claude on dev's fake `claude`
  (host H2, `host-info.generation`, dev's fake `claude`, pkg; the watane half).** After S96 and S97.
  One story in two repositories, handed out together: agentic-harness S35 is the kernel, provider
  and console half (its PLAN). Alice runs `watane-dev serve` with the agent kernel and the unchecked
  provider installed, starts a session in a scratch workspace (agentic-harness 4.6), types a
  message and watches the fake `claude`'s answer stream into the console, live and after a reload.
  Builds H2 (`claude-code.start`, `claude-session.say`, unchecked path, `Grant::ClaudeCode`,
  `allowUnsandboxedCommands: false`), `host-info.generation` as S97 writes it, and dev's fake
  `claude` (scripted stream-json, 16 of agentic-harness). Interface: the `watane:host@0.1.0` H2 and
  `host-info` additions of agentic-harness TECHNICAL 2.1 as host 19 states them. Done when the story
  passes with S35's kernel. Touches: host, dev, pkg. Boundary: W39 (adversary). Cloud.
- [x] **S101 (upkeep, cloud, routine `trig_01HEyZRTMir1EQSfMN8pKuw5`) [deputy, 2026-11-22: DONE, PR #14 merged with --admin at aa2b76fb, ~0.75 h in flight, 1 round. Bob's cloud agent's preview starts on 127.0.0.1 alone (stderr notice) when the container has no IPv6. User-sim ACCEPTED, adversary HELD, validator PASS round 1; implementer runs 2. Decisions: EAFNOSUPPORT also falls back; adversary RISK (root adds ::1 after bind) recorded in host 8.1 and 14 'Not protected' - deputy accepts it (root is outside the boundary). Host suite in the cloud: 295+28 pass, 0 fail (was 75 failing). Friction: `/usr/bin/time` missing in the cloud; dev `api::tests::ac5_search_timeout_is_thirty_seconds` (ignored, timing) red in the cloud; the ready line advertises `localhost` on a v4-only host.] A preview starts in a container without IPv6.** Bob's cloud agent runs `watane --preview` / `watane-dev serve` in a Linux container whose kernel has no IPv6, and the preview starts on 127.0.0.1 instead of refusing. host TECHNICAL 8.1: when binding `[::1]:<port>` fails with `AddrNotAvailable` (the machine has no `::1`; nobody else can bind it either), serve `127.0.0.1:<port>` alone and say so in the ready line or log; every other `[::1]` failure (in use, permission) stays `port_in_use`, and the 8-try clash loop for port 0 is unchanged. Tests: the error-kind decision as a unit test (AddrNotAvailable → v4 only; AddrInUse → port_in_use), plus the preview and serve tests that need IPv6 now running in the cloud container. Protects: host 8.1, no 14 check changes. Touches: `host/src/server.rs`, `host/TECHNICAL.md` 8.1, its tests. No adversary beyond the validator's look at the in-use branch.
- [x] **S100 DONE 2026-11-24: PR #15 merged by the human at 07:18 UTC (main b6069db7), after 10 review rounds (blocking 10, 8, 6, 3, 2, 4, 2, 2, 2, 0); three same-user risks accepted by the human; no polish left. (cloud, routine `trig_016gAuKd1gbWk68PFJgY2G8D`, fired 21:00 UTC, branch `slice/s100`, PR #15 DELIVERED febcb24 at ~23:30 UTC (user-sim ACCEPTED, adversary HELD after BREACHED 4); r1: Codex review dispatched 23:55 UTC, https://chatgpt.com/local/6b0380f0-4bb8-83ed-a541-5896ae4c707a; r1 (00:45): Codex `changes (10)` (validator RETURN, adversary BREACHED, user-sim ACCEPTED), posted with the agentic-harness contract excerpt and deputy rulings on F1.8/F1.9; the foreman (subscribed to the PR) answered all 10 at 3ed5316 (2026-11-23 02:42 UTC, s100-fix4..8; adversary BREACHED 1 then HELD, user-sim ACCEPTED); r2 to Codex in the same task (~06:45 UTC, late: the deputy's watcher died silently); r2 (06:54 UTC) `changes (8)` (adversary HELD, user-sim ACCEPTED), posted on the PR; r2 answered at d9beda1 (fix9..11; adversary 5th pass HELD, user-sim ACCEPTED); r3 to Codex; r3 (09:47 UTC, at d9beda1) `changes (6)` (adversary BREACHED: the stdin writer can run past its teardown deadline; user-sim ACCEPTED), posted on the PR; r3 answered at cafe7fa (fix12..14; adversary 6th pass BREACHED 1 fixed, 7th HELD); r4 to Codex; r4 (12:45 UTC) `changes (3)` (6 of 6 r3 closed; adversary BREACHED: an unreaped child on the stdin-setup failure path; two budget-accounting gaps), posted; r4 answered at 3033f1a (fix15; adversary 8th pass HELD); r5 (the last) to Codex; r5 (14:40 UTC) `changes (2)`: ROUND CAP, held for the human (S100-CAP); the human: one more round (F5.1, F5.2 and two vetoed risks), posted on the PR; item 5 added by the deputy (clean_runs/probe pgid); r5 answered at fdb3d9a (all five, each red first);
  r6 to Codex, limited to the five items. Round 6 is worth it: blocking went 10, 8, 6, 3, 2; the product owner asked for
  it, and what is left is five narrow items, each with a red test; r6 (~20:20 UTC, at fdb3d9a) 4/0, call stalled (F6.1/F6.2 regressions of the
  F5.1/F5.2 fixes, F6.3 signal after identity check, F6.4 record removed by bare pgid), posted with a deputy ruling
  (one admission check under one lock; refusal changes only on commit; signal only with an atomic identity check, else
  leave the group; records keyed by whole identity) for the foreman's r6 answer. Round 7 is worth it: all four are narrow
  with red tests, F6.3 is a boundary defect that must close before any merge, and the ruling removes the code paths that
  bred the regressions; the human may stop it instead (asked, deputy-2); r6 answered at 4104a61 (21:45, all four; adversary 11th pass HELD); r7 to Codex; r7 (~22:05) 2/0, call another round (F7.1 per-stream pending in the readiness scan, the third of its line; F7.2 a bad pgid record panics at startup), posted with a deputy note (the scan calls the same admission with the same arguments; validate records before any handle); the foreman answers; r7 answered at ef94a58 (22:30, both; the scan's answer equals retry's over 1,000 states); r8 to Codex. Round 8 is worth it: blocking 10, 8, 6, 3, 2, 4, 2, and r7's two are closed by construction with a property test; r8 checks one answer, and S100 blocks S103, S110 and S36; r8 (~23:15) 2/0, call another round (readiness closed; F8.1 boot id unchecked before a handle opens, F8.2 one bad line stops the cleanup of later records), posted with a deputy note (parse each line fully before any handle, skip bad lines); the foreman answers. Round 9 is worth it: both are startup-record handling, one shape closes them, each has a red test; r8 answered at 7fdcdd2 (01:12, both; adversary passes 13 to 16 on the cleanup, two spec 'Not protected' lines added: the entry budget in the gone folders, a same-user mount under runs/); r9 to Codex; r9 (~01:20) 2/1, call another round (F9.1 a symlinked runs/ is read through; F9.2 the bounded walk can make no progress), posted with a deputy ruling (drop the bounded walk for remove_dir_all; openat on a no-follow runs; deputy accepts the same-user mount line; from r10 same-user-outside-the-sandbox cleanup findings are proposed risks); the foreman answers. Round 10 is worth it: blocking flat at 2, but the ruling deletes the code that bred them, and S100 blocks S103, S110 and S36; r9 answered at 3225499 (02:02, both and P9.1; adversary pass 17 HELD; deputy accepts its proposed risk: a same-user tree past ~10,000 levels can overflow remove_dir_all at open, same-user outside the sandbox); r10 to Codex; r10 (~02:11) 0/0, VERDICT approve, call merge (validator PASS, adversary HELD, user-sim ACCEPTED); CI lint, Ubuntu and macOS green at 3225499; the merge waits on the human's go-ahead (the auto-mode classifier refuses admin merges); builder claude (rule 3: the last three were Claude, but it went out while the deputy misread Codex as signed out), reviewer codex) Stage B, end to end: Alice steers a turn (the watane half).** After S99.
  One story in two repositories: agentic-harness S36 is the kernel and console half (its PLAN), handed
  out after S35 merges. In a session on dev's fake `claude`, Claude asks to use a tool and Alice
  answers it; she sends a second message while the first turn runs (the kernel queues it); she presses
  Stop; she restarts `watane-dev serve` over the same home and the host reports a new generation.
  Builds what S99 left out of H2, as host 19 and agentic-harness TECHNICAL 2.1, 4.5, 9.4 and 15 state it:
  host-built `ask` items from `can_use_tool` and `check` items from `hook_callback` (never claude's bytes,
  Q36), launch `ask: true` and `pre-tool-check` (lift "waits for S100"), `claude-session.answer` with
  `ask-resolved {by: answered{outcome} | expired | cancelled}`, `control_cancel_request`, a check's
  `expires-at`, `updatedPermissions` never forwarded, the file-tool ask's path with symlinks resolved,
  `interrupt` resolving the asks it ends; dev's fake `claude` with a parameterised long turn that stops on
  interrupt, a reply per `say`, an ask that resumes on the answer, and a load mode (stage B budgets: 8
  sessions, 2 owners, 10 and 10 000 entries).
  Outcomes (user-sim on `watane-dev serve` with a test mod over host's interfaces, no kernel needed):
  1. A fake tool ask reaches the consumer as a host `ask` item; `answer` allow lets the turn go on, deny
     ends it; `ask-resolved` follows each, and a wrong id says `unknown-ask`.
  2. A check past its `expires-at` resolves `expired` and nothing is written to claude's stdin.
  3. `interrupt` during a long turn ends it, resolves its open asks, and the stream stays usable for the
     next `say`.
  4. A restart over the same home raises `host-info.generation`.
  5. The load script runs 8 fake sessions for 2 owners with 10 000 entries inside the stage B budgets.
  Interface: `watane:host@0.1.0` H2 as host 19 states it (no WIT shape change expected; a change goes
  to the deputy first). Touches: host, dev, pkg if a row needs it. Boundary: W39 (adversary). Cloud.
  Protects: *core*.

- [x] **S122 Alice gives a checked provider its judge key (host A5, app 19.4; agentic-harness stage E, watane half).** DONE 2026-11-25: PR #42 rebase-merged by deputy-5 (13597bf2, no bypass); builder codex, reviewer claude, 1 round (0/8); ~3.8 h in flight. On a checked provider's manager page Alice saves, replaces and removes a judge key and its https origin; the host sends it only with that provider's granted POSTs to that origin (no redirects); removing it ends the provider's live judge streams `gone`; startup deletes key files of uninstalled mods. Adversary HELD, user-sim ACCEPTED, validator PASS. R1.1 to R1.4 accepted (Deputy decisions); P1.1 to P1.8 and R1.1's spec question in Leftovers. Sketch in factory/DONE.md.
- [x] **S124 (upkeep) a failed fetch blames the true cause (net, node, dist), and net's budget flake.** DONE 2026-11-25: PR #43 rebase-merged by deputy-5 (e76d1682, no bypass); builder claude, reviewer codex, 2 rounds (3/2, 0/0); ~3.5 h in flight. A local I/O or holder failure during a fetch reads as this machine's, with no temp path; a bad copy on a full machine and every full machine being off each say so instead of 'not followed'; a close before the download is Unreachable; net's budget probes no longer race the refill clock. F1.3 regraded polish (fixed anyway); R2.1 accepted; P1.1, P1.2 in Leftovers. Sketch in factory/DONE.md.
- [x] **S125 (upkeep, guard) host's and app's flaky tests, and S122's judge polish.** DONE 2026-11-25: PR #44 rebase-merged by deputy-5 (337a8466, no bypass); builder codex, reviewer claude, 1 round (0/10); ~2.3 h in flight. The host releases its home lock explicitly when it closes (a reopen no longer meets `Busy`); the streams hand-off and Windows timer tests wait on the right events; a kept judge key stays visible and removable after `claude-code` is withdrawn; the judge editor names what is wrong; S122's judge polish done; S118 had already fixed the consent flakes. R1.1 to R1.4 accepted (Deputy decisions); P1.1 to P1.10 and three new flake sightings in Leftovers. Windows rerun once (two host timing flakes outside the slice). Sketch in factory/DONE.md.
- [x] **S141 (upkeep) host's S136/S126/S111 polish, the ac9 flake, and install's S137 rough spots.** DONE 2026-11-26: PR #52 rebase-merged by Deputy Rock (no bypass); builder codex, reviewer claude, 2 rounds (2/3, 0/0); ~1.7 h in flight. `claude::ac9` waits for the event it means; the backpressure bound is 128 events with headroom (30 passes); the judge hint names the `https-stream` import and the rerun keeps `--script`; the app shows the host's judge-origin error; install's `--status` and `--uninstall` name skipped staging, missing destinations, unrecognised receipts and partial recovery. Adversary HELD (install), user-sim ACCEPTED (after review B1.1: the build team had not run them). Deferred back to Leftovers: S136 P1.3 (a WIT change), P1.7 (a new interface). Friction: `review-lead.md` missing (proposed as a factory fix). Sketch in factory/DONE.md.
- [~] **S143 (upkeep) dev's, pkg's and log's leftovers.** in flight (cloud, builder claude, routine `trig_018MVZ2Lv3UJxBAjmPpWdzqQ` 14:41 UTC; DELIVERED as PR #53 at 72f68c82 (15:33 UTC; pkg and log items already on main, dev only); r1 Codex task https://chatgpt.com/local/6b0851b7-f61c-83eb-8244-e11126313013 15:36 UTC; r1: 1/3 findings (16:56 local, user-sim REJECTED on the provider recovery `next`), posted to the PR; answer routine `trig_01TQfeJK1DLijx1czZTcqsbt` 16:01 UTC (rulings: blocking stands, all three polish cheap); F1.ci Windows unit test posted 16:05; answers 4365f69f 16:25 (F1.ci not mentioned); r2 Codex task https://chatgpt.com/local/6b085dcb-7894-83ed-8e52-7280ed1ba003 16:27 UTC; reviewer codex; Deputy Paper, 2026-11-26; rule 3: S141, S142 were codex). Outcomes, each its item's check: dev's `### dev` items S119 r1 (P1.2, P1.3, P1.6, R1.1), S114 r3 (three), S111 r1 P1.6 to P1.13, S39 (a killed run's `<id>.<pid>.tmp` is swept), S86's prose and rerun lines; pkg's S110 R2 (Windows device names refused on every platform) and S119 P1.4; log's S109 P2.1 to P2.3. Re-check each line against main first (lesson of S49) and drop what is already done. Touches: `dev/`, `pkg/`, `log/` and their TECHNICAL; not `host/`, `app/`, `dist/`, `install/`. Constraints: no WIT change, no new public interface (S136 P1.7 stays); a refusal message may change wording. spec-check skipped: upkeep, specs change by a line at most. Exempt from the size rule (upkeep).

- [x] **S137 (upkeep) dist's local-address guards and install's S135/S123 polish.** DONE 2026-11-26: PR #51 rebase-merged by Deputy Rock (no bypass); builder claude, reviewer codex, 3 rounds (2/1, 1/1, 0/0); ~2.8 h in flight. install's `--status` names `install.sh` only in the folder a readable receipt records, uninstall claims no removal after a skipped or unreadable journal entry, and recovery still finishes an unreadable post-commit journal; S135 and S123 install polish done; dist items (1), (2) were already on main. Adversary HELD (two RISKs settled by S137-fix1), user-sim ACCEPTED (5/5). Friction: the reviewers found no `.claude/factory/review-lead.md` (proposed as a factory fix); the first build session did not wake on PR comments (answers went as new routines). User-sim rough spots in Leftovers (`### install`). Sketch in factory/DONE.md.

- [x] **S136 A mod's timer firing tells it whose authority it carries (host 19.6 build), with host's and dev's judge polish.** DONE 2026-11-26: PR #50 rebase-merged by Deputy Scissors (no bypass); builder codex, reviewer claude, 1 round (0/7); ~1.3 h in flight. A mod's own top-level `fired` reads `timers.firing-origin()` as the resolved origin identity (the owner after its client is disabled or removed); nested, page, subscriber, stream and agent calls read `none`; cancelled or replaced timers never fire; the Harness returns `none` and `Fakes` overrides it. S129 P1.1 to P1.8, S126 P1.3, P1.4 done. Adversary HELD, user-sim ACCEPTED. Polish P1.1 to P1.7 in Leftovers (`### host`). Unblocks agentic-harness's consumer half of S133 (after S134, which shares `design/TECHNICAL.md`). Sketch in factory/DONE.md.
- [x] **S135 (upkeep) install.sh's last S130 and S123 polish.** DONE 2026-11-26: PR #49 rebase-merged by Deputy Paper (no bypass); builder codex, reviewer claude, 1 round (0/14); ~1.5 h in flight. After a crash `--status` names the install lock first, a recovery says so, kill point D lists real paths, uninstall leaves the `~/.local` folders Alice made and no empty `share/watane*/`; each pinned guard has a test that kills its mutant. Adversary HELD, user-sim ACCEPTED, validator PASS. P1.1 to P1.14 in Upkeep (`### install`). Sketch in factory/DONE.md.
- [x] **S133 Design: a mod learns whose authority a timer firing carries (host 19.1, 19.6).** DONE 2026-11-26: PR #48 rebase-merged by Deputy Paper (eb9a5a8c, no bypass); builder codex, reviewer claude, 2 rounds (2/13, 0/5); ~1 h in flight. A mod's own top-level `fired` can call `timers.firing-origin() -> option<string>` (a pure read, no grant) to learn the resolved origin identity, with the S62/S66 fallbacks and nothing about the previous origin; nested calls read `none`; no startup callback; the Harness returns `none`, `Fakes` overrides. Adversary HELD. **Door C for the human's veto:** the WIT shape and no restart callback (PR #48 body). Polish P1.3, P1.4, P1.9, P1.10 and r2's five in the build slice S136 where cheap; P1.5, P1.12 go to the agentic-harness consumer half. CI: one ubuntu host flake (Upkeep, host), green on rerun. Sketch in factory/DONE.md.
- [x] **S130 (upkeep) install.sh's leftovers: what Alice sees around a Ctrl-C, `--status` and uninstall.** DONE 2026-11-26: PR #47 rebase-merged by Deputy Rock (c8eb2a1f, no bypass); builder codex, reviewer claude, 4 rounds (3/10, 2/7, 1/4, 0/0); ~13 h in flight. A killed install leaves leftovers that `--status` lists without touching, a re-run recovers them (also through a linked `~/.local`), and uninstall removes only watane's files. R2.1 accepted (deputy-7) and P3.1 under it (Deputy Rock); cut at r3: F3.1 one test, polish P3.2 to P3.4 in Upkeep (`### install`). Sketch in factory/DONE.md.
- [x] **S129 A maker previews a checked provider against dev's scripted judge (host 19.9, dev 18.4).** DONE 2026-11-26: PR #46 rebase-merged by deputy-7 (5f66e805, no bypass); builder codex, reviewer claude, 1 round (0/8); ~11 h in flight (review out in 40 min, merge waited on the handover overnight). In a `--scripted-openai` preview a checked provider's `account: "judge"` call reaches the program's `https://judge.test` with a fixture key, so a safe call runs and an unsafe one denies with no real judge; real homes and release builds refuse it; serve reports `judge_origin`. Adversary HELD, user-sim ACCEPTED, validator RETURN (polish only). R1.1 to R1.3 accepted (Deputy decisions); P1.1 to P1.8 in Leftovers. Suite: host +0.01 s, dev +2.9 s (the 15/21 min were load noise). Sketch in factory/DONE.md.
- [x] **S126 (upkeep, guard) host's last flakes and its S125/S119/S118 polish.** DONE 2026-11-25: PR #45 rebase-merged by deputy-6 (a610f9ba, no bypass); builder codex, reviewer claude, 1 round (0/5); ~2.3 h in flight. `remove` waits for a busy mod's call up to its deadline and names the mod; a restart sweeps timer rows left by a crash during `remove`; the three S125 flakes are fixed (one race, two test-only timing assertions removed; 20/20 per OS in CI); judge-origin limits recorded in host 19.4 and 3.1; S125/S119/S118 polish done. R1.1, R1.2 accepted (Deputy decisions); P1.1 to P1.5 in Leftovers. Sketch in factory/DONE.md.
- [x] **S120 Design: the factory as mods (VISION step 6).** DONE 2026-11-25: PR #39 rebase-merged by deputy-4 (fb24648, no bypass); builder codex, reviewer claude, 2 rounds (4/6, 0/1); ~45 min in flight. A draft subsystem design in `spec/design/factory/` (VISION, FUNCTIONAL JF1/JF2 with FF1-FF4, TECHNICAL): today's factory mapped to mods, toolsets and consents, and 9 open decisions in its TECHNICAL 5 (1 Door H, 8 Door C), none settled. Its first build journey JF1 waits on agentic-harness stages E (V7, the judge) and F (N5): the main line has no buildable step until then. Sketch in factory/DONE.md.
- [~] **S146 Design: one combined factory mod, and the planning/foreman split (VISION step 6, factory TECHNICAL 5).** in flight (cloud, builder claude, routine `trig_013YXruSe31389Jz8oq5XEsq` 16:59 UTC; reviewer codex; Deputy Paper, 2026-11-26; rule 3: S144, S145 were codex; Deputy Rock's correction: this slice needs no repository). Why: the human's Door H answer (Human answers, 2026-11-26: one combined factory mod in one new repository, W29 kept, "for now"; the foreman's isolation from agentic-harness profile narrowing at stage F, not a second mod) must enter the spec before the build slice after it, which needs the repository. Outcomes: (1) `spec/design/factory/TECHNICAL.md` 5's first row records the human's answer as decided, with its date and "for now", and §1/§3/§6 and the subsystem's VISION/FUNCTIONAL agree with one combined mod (one identity, one consent scope, separate pages); (2) the "Planning and foreman split" Door C row is settled to match: what one identity and one consent scope mean for the planning and foreman pages, how the foreman's narrower authority comes from a child profile at harness stage F (AGT-J5), and what stays open until F; (3) each other §5 row that the answer changes says so, and no other row is settled; (4) a reader of `spec/design/factory/` can name the repository's role (the mod's source, W29) without a slug or publisher being chosen. Touches: `spec/design/factory/` only (and a line in `spec/VISION.md`'s step 6 or `spec/TECHNICAL.md` only if they name two mods). Constraints: a design slice: no code; spec-change skill for each edit (it runs spec-check on the diff); no new repository, slug or publisher identity; the remaining Door C rows stay open; a design slice is exempt from the size rule. Door C: the settled split is listed first in the deputy's report for the human's veto.
- [x] **S102 Alice connects her agent accounts in Settings (app stage L, part: 19.1, 19.3).** DONE 2026-11-23: PR #18
  merged with --admin at cdbfd4d, ~7 h in flight, 5 Claude rounds (8, 3, 1, 2 findings, then approve; validator PASS,
  adversary HELD, user-sim 61/61). Found and fixed on the way: a macOS EINVAL in the `claude --version` probe. (Was: in flight,
  cloud, builder codex, reviewer claude; sketched 2026-11-23 by the deputy; Codex task
  https://chatgpt.com/local/6b041e07-d008-83eb-b737-65d3b6dc704c, ~11:05 UTC; PR #18 opened at 8ce07e4
  (+2162, app/ only); r1 to Claude, routine `trig_01AWD7ghXiLTs1pS7i7VmPm6`, 11:54 UTC; BLOCKED at ca0d904: the app's new dependencies need the root Cargo.lock, outside app/;
  the deputy allowed Cargo.lock and workspace-hack (generated; a rebase regenerates them), answered in the Codex task; DELIVERED at dac59df; r1 `changes (8)` (adversary HELD, user-sim ACCEPTED; F1.1 the lock, already fixed), answered at 5d35804; r2 `changes (3)` (adversary BREACHED: a gzip body escapes
  the 1 MiB limit; 4 missed mutants; a macOS test); r3 `changes (1)` (a cut gzip read `bad_input`); r4 `changes (2)`
  (adversary HELD, user-sim 58/58; a truncated gzip in a complete body reads `unavailable`; the macOS version test fails
  again); answered at 9f7a60f (F4.1 fixed; F4.2 diagnostics only). The macOS log then showed the cause: reading the
  `claude --version` probe's stdout fails with EINVAL on macOS, a product bug (choosing a real claude fails on a Mac);
  sent back to Codex to fix before round 5, the last.) Settings gets the groups
  `ChatGPT` and `Claude Code`, so the agent layer's two accounts can be set up before the host stages that
  use them land. Outcomes (app FUNCTIONAL, on the real app with stand-ins):
  1. S54: Alice chooses `Sign in with ChatGPT`; the browser stand-in opens at the stand-in issuer, the
     answer arrives on the loopback address, and Settings reads `Signed in with ChatGPT` with the plan.
  2. S57: she chooses `Sign out`; Settings reads `Not signed in` and `<home>/app/accounts/openai.json` is gone.
  3. S61: a stand-in `claude` below the minimum version is refused with `Claude Code <found> is too old;
     watane needs <need> or later.` and nothing is kept; one at the minimum is kept and shown by its path.
  4. The sign-in listener answers only its own path, closes after one matching request or 5 minutes, and a
     second sign-in while one runs is `unavailable` (19.1).
  Touches: `app/` only (`src/chatgpt.rs`, `src/accounts.rs`, `src/claude_bin.rs`, the Settings UI, `tests/it/`).
  No `deputy compares`: Settings has no design board (spec/design/manager/COMPARISON-2026-11-16.md, "Settings
  (no design board)"); the new groups only need to match the existing Settings sections.
  Constraints: no host change: the host's `AccountSource` (A4) and `Confirm::consent` (A3) are not built yet,
  so the stored account and program are read by app tests and Settings only; wiring them to the host comes
  with A3/A4. Q12 (the registered client) stays open: `CLIENT_ID`, the issuer and `SCOPES` are constants the
  tests replace with a stand-in issuer (app 18, Q12). Consents (19.2) and the judge key (19.4) are left for
  the slices that build host A3 and A5. A security boundary (an OAuth flow, tokens on disk): the adversary
  rule applies. Runs beside S100 (host/, dev/): no shared file.
- [x] **S104 (upkeep) dist reads past a bad mirror and says what is stale.** DONE 2026-11-23: PR #17 merged with --admin at 285fad1,
  ~2 h in flight, 2 rounds (r1 changes 5, r2 approve: validator ACCEPT, adversary HELD, user-sim ACCEPTED). (Was: in flight, cloud, builder codex,
  reviewer claude; sketched 2026-11-23 by the deputy; Codex task
  https://chatgpt.com/local/6b041e6f-437c-83ed-a25f-004fef586048, ~11:10 UTC; PR #17 opened at 7eb74a1
  (+285 -151, dist/ only); r1 to Claude, routine `trig_01GK67rWigPWwZ1J1UbBwX1y`, 11:54 UTC; r1 `changes (5)`, answer round to Codex in its task.) Three Leftovers items, all in `dist/`; each outcome is
  its item's check:
  1. [S49] (core) a mirror that answers 404 for the package makes dist try the next base, as a down
     base does; the package comes from the first base that has it (dist/TECHNICAL.md:990).
  2. [S49] (core) by DD-33 the cache-path link never points at a base known dead at link time
     (dist/TECHNICAL.md:1270); `ac4_cache_hit_no_record_links_first_base` serves a dead host and
     asserts the link: that test changes to assert a live base.
  3. [S57] (polish) `StaleBases` counts in words ("1 base", "2 bases"), and a slug that exists at no
     base reads as not found, not as stale.
  Touches: `dist/` only (code, tests, `dist/TECHNICAL.md` where a rule changes). Runs beside S100 (host/,
  dev/) and S102 (app/): no shared file. Remove the three items from Leftovers when it merges.
- [x] **S105 (upkeep) DONE 2026-11-24: PR #19 merged by the human at 07:26 UTC (main 10ff574f), after 6 review rounds; R6.1 accepted by the deputy (availability only, until S107); polish under Upkeep. a listed base cannot lead dist into the local network.** (in flight, cloud, builder claude,
  reviewer codex; sketched 2026-11-23 by the deputy from S104's review; routine `trig_01LmtxyjTdABbJfHbfWq8FqF`,
  13:10 UTC, branch `slice/s105`; DELIVERED as PR #19 (+950 -33, dist/ only); r1 to Codex,
  https://chatgpt.com/local/6b044a3f-725c-83ed-9b8f-699e84a7b005; r1 `changes (5)` (adversary BREACHED: old records'
  bases get unchecked redirects; F1.5 not a finding, deputy), posted; BLOCKED (how dist knows a base was given), option B chosen;
  DELIVERED at 497bf37 with r1 answered (adversary r4 to r6 found three routes, all fixed or accepted until S107;
  user-sim ACCEPTED); r2 `changes (5)` (adversary BREACHED: a race in the first-read classifier; user-sim
  REJECTED: follow reached a local first base), posted with the S105-CHURN fail-closed rule for the same round;
  answered at bad89cf (all five and S105-CHURN; adversary broke the fixes twice more, fixed, ninth pass HELD; user-sim
  ACCEPTED twice); r3 to Codex under the review-lead brief; r3: 3/2, call another round. Round 4 is worth it: blocking
  went 5, 5, 3; what is left is two narrow persistence defects (a malformed `gone` value, memory after a failed dir
  sync) and one red app test, each with a red test, and the adversary and user-sim now pass; r3 answered at fee41b8
  (all three blocking and both polish); r4 to Codex; r4 (~20:05 UTC) 3/1, call another round: F4.1 (a retry reads
  provenance before a failed dir sync succeeded: the other side of the F3.2 fix), F4.2 kept mark left by a failed
  ingest, F4.3 prune skipped after a failed sync; posted with a deputy ruling (an unsynced state: every later read or
  change retries the sync first, fails closed, then finishes the recorded pending work); r4 answer through the routine,
  20:31 UTC, session cse_01LxwP91BTERFGtyj13B1Xys; answered at b10ab90 (20:40, all three and P4.1, gate green); r5 to
  Codex. Round 5 is worth it: blocking 5, 5, 3, 3, but r4's three were one cause, now closed by one rule with red
  tests; r5 checks one answer commit, and S107 waits on this merge; r5's output was withheld by Codex's cybersecurity filter ("This content can't be
  shown"), sent again as a new task ~21:13 UTC, https://chatgpt.com/local/6b04ac8b-cd24-83eb-8242-3d0d0316a06d; withheld again, so r5 goes to Claude (4c failure mode):
  routine `trig_01CXiyL4nMudy9av7KNfRGn8`, 21:45 UTC; r5 (Claude, ~22:15) 3/4, call another round (F5.1 refresh after a
  failed unfollow requests and panics, poisoning the store's locks; F5.2/F5.3 requests while unsynced), posted with a deputy
  ruling (one gate before any request); r5 answer through the routine. Round 6 is worth it: blocking 5, 5, 3, 3, 3 is
  flat, but F5.1 is a panic that must close, the gate closes the class by construction, and a r6 return of the class
  that loses no seen/given is accepted until S107 (pre-announced on the PR); r5 answered at 8f253ba (22:35, all three,
  P5.1, P5.2); r6 to Claude again, in the r5 reviewer's session (Codex's filter withheld this slice's review twice: the
  reviewer for this slice is Claude until it merges), 22:38 UTC; r6 (Claude, ~23:00) 0/3, VERDICT approve, call merge;
  CI lint, Ubuntu and macOS green at 8f253ba; deputy accepts R6.1 (availability only, until S107); merge refused by the
  auto-mode classifier, waiting on the human's go-ahead.) Three Leftovers items, all in `dist/`:
  1. (guard) a redirect from a listed or hint base is checked like the base itself: a redirect target on `localhost`
     or a local IP literal is refused, as A35/A37 refuse the base (dist/TECHNICAL.md:1273, 1278). Deputy decision: check
     each redirect target (not "Not protected"); the 5-redirect limit stays.
  2. (guard) `localhost.` and other trailing-dot forms are local for `local_base` (`dist/src/index.rs:577`).
  3. (core) a cache miss with a listed record still tries Eve's own mirrors after the record's base fails, as outcome 1
     of S104 does without a record (`dist/src/fetch.rs` ~107-125).
  Touches: `dist/` only. Runs beside S100 (host/, dev/) and S102 (app/): no shared file. Remove the three items from
  Leftovers when it merges.
- [x] **S106 (guard) the net pair test stops flaking on macOS.** DONE 2026-11-23: PR #20 merged with --admin at b15c154,
  ~2 h in flight, 2 rounds (r1 changes 2, r2 approve; macOS green after a re-run of a cancelled job). (Was: in flight, cloud, builder codex, reviewer claude;
  sketched 2026-11-23 by the deputy; Codex task https://chatgpt.com/local/6b045b02-1260-83eb-960d-5bddbca75041; DELIVERED as PR #20 (test-only:
  frobnicate on its own unknown key, the fixture's shared byte allowance raised to 16 MiB/s; 50 runs in a row on Linux);
  r1 to Claude, routine `trig_01NSUQhchSAyP6ZAGaPLUgUo`, 16:05 UTC; r1 `changes (2)` (validator PASS, user-sim ACCEPTED
  200 runs; the brief must say the fixture runs off the production budget, and a test must prove that budget); answer
  round to Codex.)
  `net/tests/it/pair.rs:259` `s4_an_unknown_node_gets_hello_and_nothing_else`
  failed with `over_budget` instead of `unknown_op` on macOS CI for two unrelated PRs (S100 cafe7fa, S102 820c46f). The
  loop above it uses a fresh key per call because one unknown key's connection budget is small; the last call
  (`frobnicate`) reuses `seed`. Outcome: the test asserts `unknown_op` without depending on that budget, and passes 50
  runs in a row; no product change unless the budget itself is wrong (then say so). Touches: `net/tests/` only.
- [x] **S108 (upkeep) pkg names what it refuses at the right path.** DONE 2026-11-23: PR #22 merged with --admin at a49e9f8,
  ~1.5 h in flight, 2 rounds (1/2, then 0/2: approve, call merge). (Was: in flight, cloud, builder codex, reviewer claude;
  sketched 2026-11-23 by the deputy; Codex task https://chatgpt.com/local/6b047765-d63c-83ed-aded-e08349b8d06a;
  DELIVERED as PR #22 (+300 -88, pkg/ only; user-sim ACCEPTED, boundary HELD); r1 to Claude, routine
  `trig_01GeQd1KUiKu71BDnQJHHQfT`, 17:48 UTC; r1: 1/2 (blocking/polish; the deputy's triage: F1.1 blocking, the user-sim
  rejected outcome 3; F1.2, F1.3 polish, fixed on the way); answer round to Codex.) Leftovers items, all in `pkg/`; each outcome is its item's check:
  1. [S15] a link inside a hidden folder (`ui/.git/x`) is named as the hidden folder's rule, not rule `path` at the link
     (pkg/TECHNICAL.md:288; `ac1_s46_hidden_link_named` changes to match).
  2. [S57] `pack` has no list-then-stat window (as `Mod::read` was fixed): a directory swapped in for a file is named as
     such, not with the FIFO/socket text that every `InvalidInput` gets today.
  3. [S91] a pack entry gone during listing is named at its own path, not at its parent; `pkg/tests/it/s91.rs`'s race test
     (up to 5 s) moves out of the default run (a `slow:` test).
  Touches: `pkg/` only. Remove the three items from Leftovers when it merges.
- [x] **S109 (guard) the log's identity key is opened without a check-then-open.** DONE 2026-11-23: PR #21 merged with
  --admin at b09ddd6, ~2.5 h in flight, 2 rounds (2/3, then 0/3: approve, call merge). (Was: in flight, cloud, builder codex,
  reviewer claude; sketched 2026-11-23 by the deputy; Codex task https://chatgpt.com/local/6b047781-1a7c-83eb-8113-e4596ed7c056;
  DELIVERED as PR #21 (+270 -25, log/ only; user-sim ACCEPTED, adversary HELD, both Linux); r1 to Claude under the new
  review-lead brief, 17:55 UTC; r1: 2/3 (blocking/polish), call: another round (an interrupted first creation locks
  the identity out; the owner check runs /usr/bin/id -u); answered at 497d58a (both blocking and all three polish;
  Cargo.lock for rustix allowed); r2 to Claude 19:52 UTC.) `log/src/identity.rs` does `symlink_metadata` and then
  `File::open` on the key path, and accepts a hard link (no nlink or uid check) and opens a device node before `fstat`
  (Leftovers [S57], S91 adversary). Outcomes: the key file is opened once with no-follow and checked on the open handle
  (regular file, nlink 1, owner the user, mode 0600); a symlink, a hard link, a FIFO or a device node at the key path is
  refused with a clear error and the key is never read from it; each has a test. Unix only: the Windows design waits on
  the human (factory/README "Windows"). A security boundary (spec/TECHNICAL.md 6): the adversary runs. Touches: `log/`.
- [x] **S107 (guard) a base reached through dist's own links is never given (S105 option A).** DONE 2026-11-24: PR #25 merged with --admin at e3302e25 on the human's word, ~4.3 h in flight, builder codex, reviewer claude, 3 rounds (2/6, 1/2, 0/3; r1 adversary BREACHED 1: 'Use as index' on a channel row, fixed). Only Bob's own follow, open or index choice gives a base; search hits, fetched releases, listings and row buttons read listed; a legacy local base fails naming 'local address'. One risk accepted by the deputy (Deputy decisions). (Was: in flight, cloud, builder codex, reviewer claude; rule 3: last two S110 codex, S105 claude; dispatched ~07:29 UTC 2026-11-24, https://chatgpt.com/local/6b053cd8-4bc8-83eb-aba3-7bb52479b044; S110 runs beside it in dist/ with cfg gates only, the later one rebases; DELIVERED 07:45 UTC as PR #25 (Codex skipped its own offensive run); r1: Claude review routine `trig_01Sa7fvx79L7MQPknvbpYMV6`, fires 08:41 UTC; r1 (10:05): 2/6, changes (2) (validators RETURN x3, adversary BREACHED 1: 'Use as index' on a channel row gives Eve's derived base; user-sim REJECTED 1: a legacy local base fails with a wrong cause); F1.2 narrowed to naming the cause, the card hint is polish; answer r1 dispatched 10:09 UTC into the Codex task; r1 answered at 67152c1 (F1.1, F1.2 and P1.1-P1.6 fixed; the card hint skipped); r2: Claude review routine `trig_01BtdcWAYmzzB6gMRk5cJHA4`, fires 10:18 UTC; r2 (10:48): 1/2, changes (1): adversary HELD, user-sim ACCEPTED, F2.1 a test fixture that keeps macOS CI red; answer r2 dispatched 10:51 UTC; r2 answered at 3779fa6 (F2.1, P2.1, P2.2); r3: validator plus an adversary pass on P2.2's read-policy change, routine `trig_017bcbSmkXwXLc2sHx4aUZ39`, fires 10:53 UTC; r3 (11:45): 0/3, VERDICT approve (validator PASS, adversary HELD), CI green but Windows (continue-on-error, S110's); waits on the human's merge word.) (was sketch ahead; S102 merged;
  waits for S105 to merge, since both touch dist/.) A follow from a derived link (a search-hit install, the app's `follow_fetched` after
  an install, DD-29) never gives its base; only a link the participant opens or pastes (`open_link`, a pasted follow,
  `choose_index`) does. dist gets a way to say which (e.g. `follow_derived(link)` or a `Given` flag), and the app calls it.
  Outcomes: the adversary's S105 r3 and r4 routes and the prune route all fail with the 'seen' list removed; and (folded
  in 2026-11-23 by deputy-2, a local-network guard from S105 r5) `list` and `include` read a link's base as listed, not
  given, before `confirm` (`dist/src/own.rs:386-392`), so a link in a mod's publish request (`app/src/api.rs:346-363`)
  never reaches a local address or skips `held_ungiven`, with a red test of that route first. Folded here, not a slice of
  its own: the same question (which links may give a base), the same files, and S107 already goes out as soon as S105
  merges. Touches: `dist/` (incl. `own.rs`), `app/src/core.rs`, `app/src/api.rs`. A security boundary: the adversary runs.
- [x] **S103 Alice gives a mod's agents a folder, and can take it back** DONE 2026-11-24: PR #26 squash-merged with --admin at a923a8dc on the human's word (the branch held merges of main, so no rebase merge); builder claude, reviewer codex, 5 rounds (5/3, 3/3, 2/2, 1/0, 0/0); ~10.5 h in flight. Alice approves a folder for a mod's agents in a consent window, sees it in the mod's details and takes it back, ending that folder's sessions mid-turn; agent tools run with the asking mod's authority, rechecked at the run, and borrowed authority does only work that ends with the call. Decisions: Q38 (b) by the human; Q49, Q50, borrowed authority, F2.3 and F3.2 by the deputy (Deputy decisions). (Was:  (stage C, the watane half: host A3,
  app 19.2).** (in flight, cloud, builder claude, reviewer codex; DELIVERED 11:28 UTC as PR #26 (+9327/-184; adversary BREACHED 2 fixed then HELD, user-sim ACCEPTED 5/5; its foreman chose Q38 (b), Q49 (a), Q50 (a)); r1: Codex review https://chatgpt.com/local/6b057a0e-c9d4-83eb-aa82-158d1014304c, dispatched 11:53 UTC; r1 (12:50): 5/3, changes (5) (validator RETURN, adversary BREACHED 2 on tool dispatch: the consent identity of a tool call and stale authorization at dispatch; user-sim ACCEPTED), posted on the PR by the deputy; answer r1: Claude routine `trig_019s4Xd8wxUA3TjgwJYqgQWY`, fires 12:53 UTC; r1 answered at 924c3b0 (+1623/-100, all 5 fixed; new: `Host::agent_authority`, host 19.5 'Authority at the run'; the builder raised two new risks: a tool running as O can open a handle for O's toolsets, and timers or streams a tool armed keep O's authority after a toolset withdrawal); r2: same Codex review task, dispatched 13:52 UTC, weighs both; r2 (14:02): 3/3, changes (3) (validator RETURN, adversary HELD on the new check, user-sim ACCEPTED): F2.1 a tool's borrowed authority opens new handles, F2.2 its timers and streams outlive a withdrawal, F2.3 a stale tool's error code; same area as F1.1/F1.2 two rounds running (stall sign): ruled narrow (under borrowed authority no open, timer or stream), F2.3 accepted; answer r2 Claude routine `trig_01BhBsAxMbSQ4M7G2VqnhG16`, fires 14:23 UTC, merges main (S110) first. If borrowed authority comes back in r3, the deputy brings the human a cut; r2 answered at 1ffb3b1 (F2.1/F2.2 by the one rule, F2.3 known limit, P2.1, P2.2; main with S110 merged in); r3: same Codex review task, validator and adversary only, dispatched 14:41 UTC; r3 (14:56): 2/2, changes (2), adversary HELD (borrowed authority contained): F3.1 Unix-only fixtures break the required Windows test build; F3.2 borrowed https-stream.start returns internal from the A4 stub: moved to S111; answer r3 Claude routine, fires 15:14 UTC; r3 answered at 1838d16 (+70/-7: Unix-only fixtures gated, P3.1 in part, P3.2); r4 check-in: worth one tiny round: blocking 5, 3, 2, each round a different area, what is left a test-gating fix with Windows now required; r4: same Codex review task, read of the diff plus CI, dispatched 15:33 UTC; r4 (15:55): 1/0, changes (1): F4.1 Windows Clippy red at 1838d16; the builder had already pushed its fix (fa48e37, 20 lines of Windows dead-code allowances) at 7218535; r5 (a confirming read) waits for CI green there; CI green on all four at 7218535 (16:30); r5 check-in: one confirming read of a 20-line fix, nothing else open; r5 dispatched 16:33 UTC to the same Codex review task; r5 (16:32 local 17:32): 0/0, VERDICT approve, all four CI jobs green at 7218535; waits on the human's merge word; rule 3: last two S107 and S110 codex; routine `trig_01HDxDURcJvT2C5LrfygVeXb`, fires 07:33 UTC 2026-11-24; app window outcomes go to the macOS CI job; S110 runs beside it in host/src/agent/ with cfg gates only.) (was sketch ahead; after S100 merges: it touches host/src/agent/, as S100 does.) agentic-harness
  stage C: the workspace consent held before `initialized`, the consent view's words (app S58, S60),
  withdrawal ending dependent streams and closing handles, H4 agent tools (host 19.5), `consent-pending`.
  Outcomes: app S58 and S60 on the real app; host case 31 `consent` part; a withdrawal mid-turn ends that
  session's stream. Touches: `host/src/agent/`, `host/src/` consent, `app/` consent view and details. Also adds
  S100's accepted deep-tree line to host TECHNICAL 14 'Not protected' (deputy, 2026-11-24). The
  agentic-harness half (console consent activity and plan line) follows S35/S36. Builder by rule 3.
- [x] **S116 (upkeep) leftovers in app, dist, log and net.** DONE 2026-11-25: PR #32 merged by the deputy (no bypass); builder codex, reviewer claude, 1 round (0/7); ~1.5 h in flight. A no-op managed install follows nothing (only New or Update releases follow); a declined install is matched by type; dist redirect tests; one `private_dir` helper and an independent ACL oracle; a forced store-timeout test. R1.1, R1.2 accepted; polish P1.1 to P1.7 and host's 'always asks' wording in Leftovers. (Was: (in flight, cloud, builder codex, reviewer claude; rule 3:
  last two S115 claude, S114 codex; the upkeep lane, sketched 2026-11-24 by deputy-3 from Leftovers; beside S114, which holds
  `host/` and `dev/`: the host flakes and host polish wait for it; Codex task https://chatgpt.com/local/6b061040-9cc4-83ed-a17a-39e655fd4057, dispatched 22:35 UTC; DELIVERED ~23:10 UTC as PR #32 (+618/-117, all four CI jobs green; item 1's red test reproduced on macOS and passes); r1: Claude review, fires 23:37 UTC.) Outcomes, each item's check:
  1. (guard, S113 R2.1) a no-op managed install (every plan entry `Unchanged`) follows nothing; the held red test
     `manage::adv_s113_unchanged_managed_install_refollows_without_install_consent` passes.
  2. (app, S113 r3) a declined install is detected by a typed refusal, not the host's text; the wrong-slug test at Alice's
     real base from a separate reader; tests for the consent's follow line being absent where it should be.
  3. (dist, S107 r3 and S112 r2) the redirect tests that pin https and the asked URL; the doc comments and wrap.
  4. (log and net, S110 r1 and S115 r1) the independent ACL oracle; one `private_dir` helper; the forced store-timeout test;
     net TECHNICAL's close-before-reopen wording.
  Touches: `app/`, `dist/`, `log/`, `net/` and their TECHNICAL, never `host/` or `dev/`. Boundaries: dist 5.9 and log's
  key folder are touched by tests only, the managed install by item 1 (adversary on item 1). Size: an upkeep slice.
  Protects: *guard* (item 1), else *polish*.
- [x] **S123 (upkeep) Alice's install, upgrade and uninstall say the true thing.** DONE 2026-11-25: PR #41 rebase-merged by deputy-4 (no bypass); builder codex, reviewer claude, 2 rounds (1/6, 0/1); ~1.1 h in flight. install.sh says whether watane is installed, restored or partly removed after a Ctrl-C or a failure, names a failed rustup step, keeps rustup's output outside its frame, and uninstall removes the empty top-level folders it made (receipts gain `created <folder> <inode>`; old receipts leave them alone; inode reuse named in install 8.1). CI: one ubuntu host `Busy` flake, green on rerun (Leftovers, host). Sketch in factory/DONE.md.
- [x] **S121 (upkeep) polish in dist, log and pkg (S105 r5/r6, S108 r2, S109 r2, S119 P1.4).** DONE 2026-11-25: PR #40 rebase-merged by deputy-4 (no bypass); builder claude, reviewer codex, 2 rounds (2/1, 0/0); ~1.5 h in flight. A refused redirect names the base that issued it; tests pin the settle-failure path with a listed base, pkg's listing error classes and the Windows device-name reason; S105 P5.3, P6.1, P6.2 and S109's items were already on main. CI: one ubuntu app fixtures flake, green on rerun (Leftovers, app). Sketch in factory/DONE.md.
- [x] **S119 (upkeep) S111's polish in host and dev, S118's, and pkg's reserved Windows names.** DONE 2026-11-25: PR #35 rebase-merged by deputy-4 (1202a7b, no bypass); builder codex, reviewer claude, 1 round (0/7); ~2 h in flight. host's https stream gives connect and TLS handshake their own idle budget, `scripted-responses` refuses unknown script fields, pkg refuses Windows device names (guard already on main from S112, red test added). Accepted R1.1 (deputy): a maker's `scripted-responses.json` with an extra key is now refused (format shipped in S111). Polish P1.1 to P1.7 in Leftovers. Sketch in factory/DONE.md.
- [x] **S118 (upkeep) host's flaky consent tests, kept storage in the install plan, and S114's polish.** DONE 2026-11-25: PR #34 merged by the deputy (3a7999f, no bypass); builder claude, reviewer codex, 2 rounds (2/3, 0/2); ~4.2 h in flight. Both consent flakes were test-rig races; `PlanEntry.kept_state` and 'data kept from before' (needs a surviving record); a Delete data removal deletes inside one redb write holding the catalog (lock order: redb writer, then catalog); host's 'always asks' wording; S114 r3 polish. P2.1, P2.2 in Leftovers. (Was: (in flight,
  cloud, builder claude, reviewer codex; rule 3: last two S117 codex, S116 codex; the upkeep lane, sketched 2026-11-25
  by deputy-3 from Leftovers once S114 and S117 freed `host/` and `dev/`; routine `trig_01VFy6xjSA76FBaYDyGtKzLH`, fires 01:59 UTC; DELIVERED ~03:05 UTC as PR #34 at 87a9ef8 (items 1 to 4 done; both flakes were test-rig races, 20/20 per OS in run 45534361465; `PlanEntry.kept_state`; leftover: an adv_s96 timer flake on Windows); r1: Codex review https://chatgpt.com/local/6b0651be-2aec-83ed-9173-a2c50a054060, dispatched 03:14 UTC (adversary and user-sim on item 2); r1 (03:17): 2/3, changes (2) (adversary HELD, user-sim ACCEPTED on the DOM); F1.1 dismissed (the `consent` module is unix-only on main, so it cannot flake on Windows); F1.2 stands, narrowed: the kept-data label must agree with what the plan validates when an install plan overlaps a Delete data removal; P1.1, P1.2 folded in; P1.3 to Leftovers; `local/<slug>` risk accepted; answer r1: Claude routine `trig_017xicAQrQ4sa1oMjgrpj3Pm`, fires 04:00 UTC; r1 answered at e2e147d (a Delete data removal deletes inside one redb write that also holds the catalog; its first form deadlocked against timer commits and the builder's validator caught it; lock order 'redb writer, then catalog' everywhere); r2: Codex focused check in the same task, dispatched 05:08 UTC (with a hunt for other reverse lock orders).) Outcomes, each item's check: (1) (guard)
  `consent::ac5_a_consent_given_through_one_provider_is_not_used_through_another` (`host/tests/it/consent.rs:533`, seen
  twice) and `https_stream::ac5_a_refused_consent_ends_the_stream_consent_refused` stop flaking: name each race, fix it
  in host if a person could hit it, else in the test; a CI job on the branch runs both 20 times on each OS, all green;
  (2) S113: a reinstall after 'Keep data' no longer labels Storage `new`: the host's install plan reports retained
  storage and the consent words say it is kept, with a test; (3) host TECHNICAL 1845 and 2423 stop saying a
  manager-started install "always asks" (S116 made a no-op install follow nothing; say what is true); (4) S114 r3 P3.1
  to P3.3 in `dev/src/preview/openai.rs` and dev TECHNICAL. Touches: `host/`, `dev/`, `app/` only for item 2's words.
  No boundary moves (adversary not run unless item 2 changes what consent grants). Protects: *guard* (item 1), *polish*.
- [x] **S117 (upkeep) the app's last leftovers and S116's polish.** DONE 2026-11-25: PR #33 merged by the deputy (9ba1112, no bypass); builder codex, reviewer claude, 2 rounds (1/5, 0/0); ~1.8 h in flight. S116 P1.1 to P1.7 done; `Core::mods()` memoizes channel views (S44); channel rows show plain words and APP-F-37 says so (S58); a macOS removal-test race fixed. R1.1 accepted; P1.3 to P1.5 in Leftovers. (Was: (in flight, cloud, builder codex, reviewer claude;
  rule 3: last two S116 codex, S115 claude; the upkeep lane, sketched 2026-11-25 by deputy-3; beside S114, which holds
  `host/` and `dev/`; Codex task https://chatgpt.com/local/6b062512-7874-83ed-ba16-8300d8b14068, dispatched 00:05 UTC; DELIVERED as PR #33 at ab6a9ca (+610/-158, all four CI jobs green after one macOS retry of an existing removal journey); r1: Claude review, fires 01:12 UTC; r1: 1/5, changes (1) (validators PASS but s117-4, user-sim REJECTED on APP-F-37 only; the macOS removal flake is a test race, fixed by s117-fix1): F1.1 app FUNCTIONAL APP-F-37 still promises `Compromised: <cause>`; ruling: amend APP-F-37 to the new words, plus P1.1 (words.rs arm tests) and P1.2 (a TECHNICAL 9.2 sentence); P1.3 to P1.5 to Leftovers; R1.1 accepted; answer r1 dispatched ~01:25 UTC; r1 answered at 54bafe1; r2: Claude reading check, fires 01:33 UTC.) Outcomes, each item's check: (1) S116 r1 P1.1 to P1.7 (the mixed-plan consent test, the dist
  redirect test that records calls, `Io` for OS failures in `check_owner_only`, the `icacls` oracle on parent and dir, a
  test-only STORE_WAIT that brings net's unit tests back near 1 s); (2) S44: `Core::mods()` stops re-reading every
  related channel view on each 2 s refresh (memoize or read on change), with a test that counts the reads; (3) S58:
  channel rows show words, not raw transport text, with words in app TECHNICAL first. Touches: `app/`, `dist/`, `log/`,
  `net/`, never `host/` or `dev/`. No boundary moves (adversary not run unless one does). Protects: *polish*.
- [x] **S114 A maker previews an OpenAI-backed mod on scripted responses (stage D, dev 18; F-MAKE-4).** DONE 2026-11-25: PR #30 merged by the deputy (squash fdecf1d, no bypass); builder codex, reviewer claude, 3 rounds (4/5, 1/6, 0/3); ~4.5 h in flight. `watane-dev serve` routes api.openai.com to `scripted-responses` on a development host; release hosts and real homes refuse it; a refusing host is remembered. R2.1 to R2.3 accepted; P3.1 to P3.3 in Leftovers. (Was: (in flight,
  cloud, builder codex, reviewer claude; rule 3: last two S40 claude, S113 codex; main line, sketched 2026-11-24 by
  deputy-3: F-MAKE-4 promises a scripted OpenAI stream in a preview from stage D, and dev TECHNICAL question 15 recommends
  option (a); it unblocks agentic-harness S40's run on `serve`. It builds against `scripted-responses` as on `slice/s111`
  (PR #28, approved) and merges main once that merges; Codex task https://chatgpt.com/local/6b05ef35-5ee4-83ed-b7bb-2b58230a9070, dispatched 20:12 UTC; DELIVERED ~21:02 UTC as PR #30 (+1095/-47, all four CI jobs green; the builder's adversary: real homes refuse the setting, release hosts exclude it); r1: Claude review, fires 21:23 UTC; r1 (22:44): 4/5, changes (4) (adversary HELD, user-sim REJECTED on outcome 3): F1.1 the host drops its home lock before its database (restart race), F1.2 released tools can no longer preview a mod that declares api.openai.com, F1.3 a bad script's error blames the build, F1.4 untested acceptance lines (the TLS and loopback ones required); F-MAKE-4 not narrowed (released tools lack the scripted server until dev question 14: a known limit); answer r1 dispatched 22:48 UTC; r1 answered at 6312938 (+1034/-224, F1.1 to F1.4 and P1.1 to P1.5; P1.6 to P1.8 deferred); r2: Claude review, fires 23:20 UTC; r2 (23:46): 1/6, changes (1) (validator PASS, adversary HELD, user-sim ACCEPTED outcomes 1 to 3): F2.1 a host that refuses the scripted setting gets its preview restarted on every serve (new MCP token each time); R2.1 to R2.3 accepted; answer r2 dispatched 23:50 UTC; r2 answered at 6766453 (+255/-59, the fallback remembered, all four CI jobs green); r3: Claude reading check, fires 00:21 UTC.) Outcomes (user-sim with a maker's mod under `watane-dev serve`):
  1. `watane-dev serve` of a mod that uses `https-stream` with `account: "openai"` starts `scripted-responses` and the
     preview routes `api.openai.com` to it: the mod's stream gets the scripted reply, with no real account and no network.
  2. The preview setting is not `test-net`, is never in a release build of the host or app, and a real home refuses it.
  3. A maker can give the script (`--script` or a file in the mod's folder, as dev 18 says) and sees a clear error for
     a bad one.
  4. agentic-harness S40's `test:host:d` checks can run on this preview (its SKIP lines become runnable).
  Interface: a host preview setting (host 19.9) and `serve`'s options (dev's CLI, a public interface: say it in dev
  TECHNICAL first; resolve question 15 as (a)). Touches: `dev/src/preview*`, `dev/src/cli.rs`, `host/` preview setting,
  dev and host TECHNICAL. Not S111's files beyond a wiring line. Boundary: the account binding must not reach a real home
  (adversary). Protects: *core*.
- [x] **S115 (guard) the net reach test stops flaking on Windows.** DONE 2026-11-24: PR #31 merged by the deputy (no bypass); builder claude, reviewer codex, 1 round (0/2); ~1.2 h in flight. net's rebuild now awaits the blob store's shutdown (bounded by 5 s) before reopening, so a relay or DHT switch on Windows no longer fails with 'Database already open'; 40/40 green on Windows. Polish P1.1 (a forced-timeout test), P1.2 (net TECHNICAL wording) in Leftovers. (Was: (in flight, cloud, builder claude, reviewer codex;
  a fix that guards `main`: Windows is a required check and `net/tests/it/reach.rs:193` failed on it with `Database
  already open. Cannot acquire lock.`; routine `trig_01Qb1GYPxMdJLutKESYRz4Vw`, fires 20:16 UTC; DELIVERED ~21:05 UTC as PR #31 (+115/-3: the blob store's redb reopened before its shutdown finished; apply now awaits it, bounded by 5 s; 40/40 green on Windows); r1: Codex review https://chatgpt.com/local/6b05fddd-2ba4-83ed-9942-035b82291050, dispatched 21:13 UTC.) Outcome: the cause is named (net's redb reopen on Windows), fixed in code or test,
  and `reach::s11_machines_on_different_networks_connect_through_the_servers_relay` is green in 20 Windows runs in CI.
  Touches: `net/` only. Size: a guard, by reason. Protects: *guard*.
- [x] **S113 (upkeep) the app says what it is doing and what Alice agrees to.** DONE 2026-11-24: PR #29 merged by the deputy (no bypass); builder codex, reviewer claude, 3 rounds (2/3, 1/4, 0/3); ~3.5 h in flight. Deny says what is denied; Call grants show their exact text with plain words beside; installing from Find reads first and follows the publisher only after the install succeeds, and the consent says so; a declined install reads 'You declined installing <mod>.'; busy-port and disabled-provider messages; grant changes refresh without a reload. R2.1 accepted; polish P3.1 to P3.3 and the 'Keep data' label in Leftovers. (Was: (in flight, cloud, builder codex,
  reviewer claude; rule 3: last two S112 codex, S111 claude; the upkeep lane, sketched 2026-11-24 by deputy-3 from
  Leftovers `### app`; runs beside S111, which touches only `app/TECHNICAL.md` in app; Codex task https://chatgpt.com/local/6b05df95-c29c-83eb-bac0-cb04ace5305d, dispatched 19:05 UTC; DELIVERED ~19:55 UTC as PR #29 (+307/-203, all four CI jobs green); the 'Keep data' storage label needs the host (Leftovers); r1: Claude review, fires 20:14 UTC; r1 (20:45): 2/3, changes (2) (user-sim REJECTED on F1.1): F1.1 installing from Find follows before consent and after Deny (pre-existing, the new hint made it visible): ruled option (a), follow only after the install succeeds, adversary in r2; F1.2 the short Call line against APP-F-77: narrowed to the spec's words, exact text stays visible; answer r1 dispatched 20:48 UTC into the Codex task; r1 answered at c36d534 (F1.1 option (a), F1.2, P1.1 to P1.3; main with S111 merged in); r2: Claude review with the adversary on the new install order, fires 21:08 UTC; r2 (21:54): 1/4, changes (1) (user-sim ACCEPTED, adversary: R2.1, pre-existing): F2.1 dist's 'Install a hit' contract still says follow then fetch (docs); R2.1 accepted; P2.3, P2.4 taken; answer r2 dispatched 21:58 UTC; r2 answered at 365a64f (+131/-19); r3: Claude reading check, fires 22:09 UTC.) Outcomes, each item's check, on the
  real app (user-sim under xvfb; macOS CI for window tests):
  1. Consent words: "Deny" says what was denied; the Call line names the interface in words, not `mods:x/y@0.1.0`
     (from agentic-harness S39's review); installing from an index says it follows the channel; Remove asks first.
  2. Busy and failure states: a hanging follow or add-index shows a busy state; a disabled provider shows the host's own
     message; the busy-port window says the local API is not running.
  3. Plumbing: a grant change updates the page without reloading the whole window; hidden windows still refresh;
     reinstall after "Keep data" keeps the storage as kept; `/api/fetch` with a wrong body names the field.
  4. CI noise: the Linux runner prints names with versions and sets `NO_AT_BRIDGE`.
  Words in app TECHNICAL first. Touches: `app/` (ui, src, scripts, tests), `app/TECHNICAL.md`. No boundary moves (the
  consent texts are words of existing grants): no adversary unless the build changes what a consent covers. Size: an upkeep
  slice. Protects: *polish*.
- [x] **S111 A mod streams from OpenAI on Alice's ChatGPT plan (stage D, host A4, dev 18).** DONE 2026-11-24: PR #28 squash-merged at 71b3ae6a by the deputy (no bypass); builder claude, reviewer claude (Codex filter before Daybreak), 1 round (0/14); ~5 h in flight, built beside S103. A mod with an Http grant streams HTTPS with `https-stream.start`; with `account: "openai"` the host adds a fresh ChatGPT-plan token, only on the two OpenAI routes, and the mod never sees it; dev's `scripted-responses` serves tests. Risks R2, R3, R5, R6 accepted; polish in Leftovers. (Was: S111 A mod streams from OpenAI on Alice's ChatGPT plan (stage D, the watane half: host A4, dev 18).** (in flight, cloud, builder claude, reviewer codex; rule 3: last two S39 codex, S38 codex; handed out 2026-11-24 ~15:20 UTC beside S103 (the human asked for more concurrency): S111's work is mostly new files (`host/src/agent/https_stream.rs`, dev's `ScriptedResponses`); it builds against S103's branch for `consent` and borrowed authority, and merges main once S103 merges; it must not edit S103's files beyond the wiring line; routine `trig_01F4feSQK4npLjE7VcKYS2pd`, fires 15:16 UTC; DELIVERED ~17:30 UTC as PR #28 (S111's own diff over slice/s103: +5755/-48; user-sim ACCEPTED, adversary HELD with open questions R2 account-identifying response headers to the mod, R3 extra request headers); r1: Codex review https://chatgpt.com/local/6b05cb69-1fc4-83eb-8ef6-624f3ebf9050, dispatched 17:40 UTC, weighs R2 and R3; withheld by Codex's cyber filter; r1 runs on Claude (routine, fires 18:43 UTC): a Claude review of a Claude build, because the other side is down for this content; r1 (19:15): 0/14, VERDICT approve (validators PASS x2, RETURN x1 regraded polish, adversary HELD, user-sim ACCEPTED, stage D p95 7.4 ms), reviewed at c7a4e885; head 3e8d788 merges main with S111's own files unchanged; merge once the Windows job (a net flake) re-runs green.) agentic-harness stage D needs
  H3 (host 19.4) with the account `openai`, and dev's scripted Responses server, before its OpenAI loop can run.
  Outcomes (user-sim on `watane-dev serve` with a test mod and dev's `ScriptedResponses`, `test-net`):
  1. A mod with an `Http` grant for a host streams an SSE response: `{status, headers}`, then one item per event; a
     non-SSE body arrives in 64 KiB chunks; `idle-s` without bytes ends it `idle-timeout`.
  2. Each refusal of 19.4 at the call happens before a byte leaves (`destination`, `forbidden-header` incl. case and
     pseudo-header forms (S64 header part), `too-large`), and a revoked grant or disabled mod at apply ends it `gone`.
  3. With `account: "openai"`, the host adds a fresh bearer token from `AccountSource::openai_token` (the app's tokens
     of S102), only to the two allowed OpenAI routes; no app ends it `no-app`, no tokens `no-account`, and the 19.7
     consent is held first.
  4. agentic-harness cases 3 and 19 (`openai`) host parts pass; the stage D budgets hold.
  Interface: `watane:host@0.1.0` H3 as host 19.4 states it (a WIT shape change goes to the deputy first). Constraints:
  no real OpenAI call in CI (the scripted server); `judge` (A5) is left out. Touches: `host/src/agent/` (new
  `https_stream.rs`), `host/` grants and accounts, `dev/` scripted Responses, `pkg/` if a row needs it. Boundary: W39
  and the token never reaching a mod (adversary); from S103 r3 F3.2: `https-stream.start` under borrowed authority (a tool's job or tools/call) is refused `not_granted`, as host 19.5 says. Protects: *core*.
- [x] **S112 (upkeep, guard) dist, log, net and pkg close their known gaps.** DONE 2026-11-24: PR #27 merged with --admin (rebase, main 3dad05d1) on the human's word; builder codex then claude (Codex filter), reviewer claude, 2 rounds (2/9, 0/5); ~3.8 h in flight. dist refuses the extra local ranges and any redirect to a local address or a non-http(s) scheme before a request; log refuses a key folder another user owns; the net budget flake is fixed; pkg refuses Windows device names. Risks R1.1 to R1.3 accepted (Deputy decisions); polish and the local-names note in Leftovers. (Was: (in flight, cloud, builder codex,
  reviewer claude; sketched 2026-11-24 by deputy-3 from Leftovers, run beside S103 and S111: it touches none of
  `host/` or `app/`; Codex task https://chatgpt.com/local/6b05a9c2-788c-83ed-a2bd-c772a853c056, dispatched 15:17 UTC; DELIVERED ~16:20 UTC as PR #27 at 6f0aea0 (+474/-67, all four CI jobs green); r1: Claude review routine, fires 16:31 UTC; r1 (17:14): 2/9, changes (2) (validators PASS x4, adversary BREACHED 2, user-sim ACCEPTED): F1.1 a redirect to a SOCKS scheme reaches a local port, F1.2 lock_dir's owner check untested; answer r1 dispatched 17:18 UTC into the Codex task; withheld by Codex's cyber filter (as S105 r5); the answer round went to Claude (routine, fires 17:57 UTC); while Codex cannot take this slice's SSRF work, its later rounds stay on Claude, review included (the other vendor's side is down for it); r1 answered at 0dd73ef (+350/-68; F1.1 by hand-followed redirects with one no-redirect agent, a new mechanism; F1.2; P1.1, P1.5, P1.6, P1.7, P1.9); r2: Claude review with the adversary on the redirect path, fires 18:44 UTC; r2 (19:00): 0/5, VERDICT approve (validator PASS, adversary HELD, user-sim ACCEPTED), all four CI jobs green at 0dd73ef; waits on the human's merge word. Codex's task reported that a shell diagnostic printed fragments of an environment credential into its own transcript (not on GitHub: diff, commits, PR and CI log scanned clean): told the human.) Outcomes, each item's check:
  1. (guard, S105 adversary) dist's local ranges (5.9) also refuse 100.64.0.0/10, 198.18.0.0/15, 0.0.0.0/8, fec0::/10 and
     the NAT64 and 6to4 forms of a local address, each with a test.
  2. (guard, S109 review) the log refuses a log directory owned by another user (root over a 0700 folder someone else
     owns), with a test; a Windows equivalent refuses or is named a known limit.
  3. (guard, S100 CI) `net/tests/it/budget.rs:98` stops flaking: the cause named, fixed, 20 runs green in CI.
  4. (S110 R2) `pkg` refuses Windows reserved device names in a package path on every platform, with a test.
  5. Polish where cheap: S105 P5.3, P5.4, P6.1 to P6.3; S107 P3.1, P3.2; S109 P2.1 to P2.3; S110 r1 P1.2, P1.4, P1.6.
  Touches: `dist/`, `log/`, `net/`, `pkg/` only. Boundaries: Verification (dist 5.9) and log's key file (adversary
  runs). Protects: *guard*. Size: below 1,500 lines, by reason: three guards on `main` plus the polish queued behind
  them.
- [x] **S110 (guard) CI is green on Windows, so its check can be required.** DONE 2026-11-24: PR #23 merged with --admin at d797750d on the human's word; builder codex, reviewer claude, 3 rounds (2/10, 1/1, 0/0); ~17.5 h in flight, most of it waiting for S100 and S105 (active ~6 h). Windows is a checked platform: `test (windows-latest)` green and no longer continue-on-error; log's identity key keeps no-follow and owner-only guarantees on Windows; features without a Windows equivalent refuse there with a test. Decisions: `watane-log-windows-fs` the one unsafe crate, workspace `deny` (Deputy decisions); R1, R2 accepted. The Windows check is required in the ruleset 'main: CI green before merge' since 2026-11-24 ~14:30 UTC (set by the deputy with gh on the human's word). (Was: asked by the human, 2026-11-23;
  in flight, cloud, builder codex, reviewer claude; rule 3: last three S36 claude, S109 codex, S108 codex - Codex builds
  2 in 3; dispatched ~20:27 UTC, https://chatgpt.com/local/6b04a1f4-b2cc-83ed-9939-168f9ec34073.) Today Windows CI reports but does not block (`continue-on-error`, factory/README "Windows"): it stops in
  `log`'s key-file open, then whatever comes next. Outcomes: (1) `test (windows-latest)` is green on the PR's head, twice
  (a re-run), with `continue-on-error` removed from `.github/workflows/ci.yml`; (2) `log`'s identity key file on Windows
  keeps the same guarantees as on Unix: no-follow (`FILE_FLAG_OPEN_REPARSE_POINT`, a reparse point is refused) and an
  owner-only ACL on create, refused with a clear error otherwise, each with a test that runs on Windows CI; (3) Linux
  and macOS stay green and their suite times do not grow by more than 10%. Constraints: never a `cfg` that skips a
  security check (spec/TECHNICAL.md 6): a check without a Windows equivalent makes that feature refuse on Windows with
  a clear error, and its tests assert the refusal; tests of macOS/Unix-only features (`sandbox-exec`, the AppKit
  driver, `install.sh`, process groups) are gated to their OS, each gate with a one-line reason. The app's front
  tests may stay off on Windows (no driver yet), said in ci.yml. Making the check required in the ruleset is the
  human's (an account setting). The human's request is the go-ahead on the key-file design that factory/README
  "Windows" held for them (deputy reading, vetoable). Touches: `log/`, `.github/workflows/ci.yml`, factory/README
  "Windows", and `cfg` gates across crates. Overlaps S100 (`host/src/agent/`) and S105 (`dist/`): do those crates
  last and rebase onto main once they merge; never edit their files on `slice/s110` while they are in flight. The
  adversary runs on the `log` change. Size: a whole capability (Windows as a checked platform).
  Progress: draft PR #23; BLOCKED at 80ffd54 (20:56 UTC) as briefed: log (native no-follow handles, owner-only ACLs,
  refusal tests), net, pkg, install ported; Windows build now fails only in host/src/agent/ and dist/. Resume after S100
  and S105 merge: a build brief "continue from branch slice/s110; rebase onto main" into the same Codex task. Resumed ~07:28 UTC 2026-11-24 in the same task (edits in dist/ and host/src/agent/ kept to cfg gates and refusals, since S107 and S103 run there). Windows failures per run: 358, 85, 30, 16, 15, 2, 0; DELIVERED ~11:45 UTC at 68d1dd4 (Windows green on attempts 1-3 of run 45370144960; Linux and macOS suite times within 10% of a parallel main run); r1: Claude review routine `trig_01CRaJJrcV955wX2NNsZP4NR`, fires 11:52 UTC; r1 (12:38): 2/10, changes (2) (validators PASS 1, RETURN 3; adversary HELD; user-sim ACCEPTED; no gate skips a named attacker's check): F1.1 the windows-fs crate's place in the spec, F1.2 two refusal tests; answer r1 dispatched 12:41 UTC into the Codex task; r1 answered at 3bd1122 (F1.1, F1.2, P1.1, P1.3, P1.5, P1.8, P1.9; the rest to Leftovers); r2: small Claude review routine `trig_01NGu5yhZEMyyvF8HwWxeKwD`, fires 12:49 UTC; r2 (13:05): 1/1, changes (1) (adversary HELD; the two new Windows refusal tests ran green on Windows): F2.1 a test assertion from my r1 polish wording, Windows red; answer r2 (option a, plus the forbid-to-deny docs) dispatched 13:10 UTC; r2 answered at 348a068 (+11/-6); r3: tiny Claude review (reads the diff, waits for Windows CI) routine `trig_017KsQQkRmNXpZNcNLSz74Gf`, fires 13:13 UTC; r3 (13:28): 0/0, VERDICT approve, all four CI jobs green at 348a068 (Windows included); waits on the human's merge word.

## Suite time

Wall time of each subsystem's tests on `main` (`factory/README.md`, "Time and value"): the
default `cargo test` (budget 2 min) and `--include-ignored` (budget 10 min). From the main check
after each merge; under load, say so. A row over budget, or up more than 20% in one slice, makes
that subsystem's next slice a test-cost slice.

| Subsystem | Default | With ignored | Measured |
|---|---|---|---|
| app | 9 s; 95 s with `--features driver` | 79 s (11 slow, no front/real/vm; S90) | main `393ae6c` (MAIN PASS), integration binary, load ~10 to 17 |
| dev | 22 s | 283 s (46 slow, real `watane`) | main `393ae6c`; slow set at `e04b480` (S93), load ~10 |
| dist | 28 s | not measured | main `393ae6c` (MAIN PASS), integration binary, load ~10 to 17 |
| host | 11 s | not measured | main `393ae6c` (MAIN PASS), integration binary, load ~10 to 17 |
| install | 125 s (S94; cloud 8 s) | 483 s | main `b5e54ab`, Mac, integration binary, load ~6 to 11 |
| log | 7 s | not measured | main `393ae6c` (MAIN PASS), integration binary, load ~10 to 17 |
| net | 24 s | not measured | main `393ae6c` (MAIN PASS), integration binary, load ~10 to 17 |
| node | 16 s | not measured | main `393ae6c` (MAIN PASS), integration binary, load ~10 to 17 |
| pkg | 8 s | not measured | main `393ae6c` (MAIN PASS), integration binary, load ~10 to 17 |

## Stage coverage

One row per stage of each subsystem's build plan (its TECHNICAL "Build plan" table), with the
step that needs it. Status: `open`, `part` (what is missing), `done` (slice). A step is done only
when its journey passes and every stage of that step is `done`, or a deputy decision moves the
rest to a later step. Leftover `ponytail:` comments count as missing parts (`git grep ponytail:`).

| Subsystem | Stage | Step | Status |
|---|---|---|---|
| install | 1 script, 2 versions, 3 packer | 7 | done (S50b; macOS S50b-fix) |
| install | 4 workflow, 5 clean Mac (J7) | 7 | done (S50c; v0.1.0 checked in a clean VM) |
| net | N1 crate, codecs, `hello`, `heads`, `entries` | 8 | done (S60) |
| net | N2 device list, pairing, classes, allowances | 8 | done (S62) |
| net | N3 sync, gossip hints, blobs | 8 | done (S63) |
| net | N4 light machines, restore, reach | 8 | done (S67, S68) |
| log | L6 keyless (`open_keyless`, `identity()` an Option, `drop_identity`) | 8 | done (S61) |
| dist | S8 keyless, holder methods, `kept`, `Via::Kept`, `search_stored`, the lock rule | 8 | done (S61) |
| node | 1 holder | 8 | done (S64) |
| node | 2 machines (`told`, `need`, search, join, restore hints) | 8 | done (S70) |
| node | 3 program (`watane-node` join, run, status, relay, dht) | 8 | done (S72) |
| app | G own machines (J8) | 8 | done (S75) |
| pkg | 9 outlet functions, `World::collects` | 9 | done (S65) |
| host | 10 collect | 9 | done (S65) |
| dev | 12 collect fakes and answers | 9 | done (S65) |
| app | H extension points (J9) | 9 | done (S66) |
| pkg | 10 `main` in `mod.toml` | 10 | done (S74) |
| app | I main mod and manager | 10 | done (S74) |
| pkg | 11 `Grant::Manage` | 10 | done (S76) |
| host | 11 `watane:host/manage` | 10 | done (S76) |
| dev | 13 manage fake | 10 | done (S76) |
| app | J manage requests, J10 | 10 | done (S77) |
| pkg | 1 names, 2 meta/deps, 3 world/folder/grant | 1 | done (S1) |
| pkg | 4 component | 1 | done (S2) |
| pkg | 5 package (pack, unpack) | 1 | done (S7, S8: the host installs through it) |
| pkg | 6 add_provider | 2 | done (S11) |
| pkg | 7 release | 3 | done (S20) |
| host | 1 runtime core | 1 | done (S2) |
| host | 2 links and events | 1, 2 | done (S3, S11: links, subscribers, hops on the preview `install_folders` path) |
| host | 3 server and pages | 1 | done (S9); the subscription and revoke hostile rows in step 2 |
| host | 4 tools (toolsets, tokens, MCP) | 1 | done (S5, S58: the app lists toolsets, removes one and issues tokens) |
| host | 5 catalog and home | 1, 2 | done (S8, S11, S12a: plan, `Source::Folder`, grants, revoke, update, remove, home mode) |
| host | 6 program | 1 | done (S9: `--home` runs a participant home with no app) |
| host | 8 releases | 3 | done (S21b: `Source::Release`, S31, S32) |
| dev | 0 cli, outcome | 1 | done (S1) |
| dev | 1 template, new, check | 1 | done (S4) |
| dev | 2 allowlist, toolchain, builder | 1 | done (S8) |
| dev | 3 unit | 1 | done (S4) |
| dev | 4 contract | 1 | done (S4, S11: fakes and `deliver`) |
| dev | 5 preview, api, doctor, fake-watane | 1 | done (S6); `fake-watane` moved to step 2 (deputy) |
| dev | 6 deps, add, install | 2 | done (S11, S12b; `fake-watane` too) |
| dev | 7 publish | 3 | done (S21c) |
| dev | 8 search | 4 | done (S31; the app route S29) |
| app | A shell | 2 | done (S10) |
| app | B host | 2 | done (S12a) |
| app | F Linux | 2 | done (S18: arm64 Linux in Docker, `app/scripts/linux-test.sh`; front tests are macOS only) |
| app | C dist | 3 | done (S21b: publish, follow, `DistConfirm`) |
| app | D find | 4 | done (S25: APP-F-37, 39, 40; S29: APP-F-38, 43 to 46) |
| app | E build on | 5 | done (S37) |
| host | 9 binds | 5 | done (S34) |
| dist | S7 provider bases | 5 | done (S35) |
| dev | 9 build on | 5 | done (S36) |
| pkg | 8 repository | 6 | done (S41) |
| dev | 10 publish --push | 6 | done (S42) |
| dev | 11 new --from clone and fork | 6 | done (S43) |
| log | L1 to L5 | 3 | done (S19: L1 to L4; S21a, S21b: L5) |
| dist | S1 to S5 | 3 | done (S21a) |
| dist | S6 find | 4 | done (S28) |

## Leftovers


Upkeep (polish from S105's reviews r5 and r6, 2026-11-24):
- [S105 P5.3] `dist/src/holder.rs:162`: a Kept ingest of the participant's own channel settles too, so it fails while the store is unsynced; settle only when `channel != identity`, or say so in dist TECHNICAL 4.4.
- [S105 P5.4] a refused redirect names the local URL Eve never typed; name the base that redirected instead.
- [S105 P6.1] `dist/src/index.rs:457`: no test pins `self.sync_pending()?` after the base loop in `read_record`.
- [S105 P6.2] `dist/src/fetch.rs:500`: no test pins the gate in `open_link` before `follows.contains_key`.
- [S105 P6.3] `dist/src/follow.rs:591`: `refresh` ignores a failed `settle(true)` and goes on to `read_listed`.

Upkeep (polish from S109's review r2, 2026-11-23):
- [S109 P2.1] `log/TECHNICAL.md:594`: the Libraries line for rustix omits `default-features = false` (log/Cargo.toml:22).
- [S109 P2.2] `log/src/identity.rs:1`: the `//!` line should cite TECHNICAL 4.7 too.
- [S109 P2.3] `log/src/identity.rs:165`: `publish_key` wraps `fs::rename` only for a test that mostly tests rename.
- [S109 review] a crash before the rename leaves a stray `.identity.key.<hex>.tmp`; `replace` keeps the old key in memory
  when the folder fsync fails after the rename (older than S109).

Upkeep (polish from S108's review r2, 2026-11-23):
- [S108 P2.1] `pkg/src/folder.rs:287, :291` (`list_entries`): the new error classification has no test.
- [S108 P2.2] `pkg/TECHNICAL.md:356-357` repeats lines 346-348.
- [S108 review] after one vanish, a read's other violations are hidden, and churn in hidden files keeps refusing reads
  (older than S108; spec questions).
- (guard, 4 cloud runs: S96, S97, S99, agentic-harness S32) The host refuses to start without an IPv6 loopback
  (`host/src/server.rs:76`); cloud containers have none, so each cloud slice patches it by hand. Now S101 (Slices).
- A watane release with host 19 (past v0.1.0), so agentic-harness CI's watane-dev can link the kernel's world.
- S96: the fixture build is not byte-reproducible (every rebuild rewrites each `logic.wasm`).
- S54: dev's publish still suggests raw.githubusercontent.com as the base; GitHub Pages is a real static host and could be offered.

Every deferred note in "Slices" audited against main on 2026-11-14: 95 items from 27 notes.
57 were already fixed by a later slice and are gone from here; the per-slice notes are kept as
history but this section is the live list. Each line gives the slice that found it and where the
gap is visible. Before sketching from this, re-check the line: that is the lesson of S49.

Items closed silently by S39, S44, S45, S46, S47, S48 and S49 are the bulk of the 57. Three were
filed under the wrong subsystem and are moved here: S41's `repository` fix is dev, S44's unknown
`/api` route is host, S12b's `ac2_real_home_without_app` is dev. Two were filed twice (the
deps.json self entry in S33 and S39; the 52-character consent key in S29 and S44) and are one row.

### Decide, do not sketch

These are true but are now what the specs say. Acting on any of them is a spec change, not a fix.

  Delivered (894275d, merged rebased): a maker's agent writing S65's `menu` and `item` gets all four right on the first try (user-sim ACCEPTED). Fake function names come from `logic.wasm`'s imports; `collects` beside `events` in `check --json`. Verified on the Mac: dev green with its 18 server tests, J2 and J4 green; dev's `ac10_endless...` deadline test failed again (see Friction). Later (user-sim): the guide does not say to change `generate!(world: ...)` after renaming the world; a misspelled `deliver` name does not list the real functions; a misspelled fake of an imported-but-uncalled interface is not refused.
- [S12b] `--wait` does not count the test pipeline; it is the time the participant has to answer
  (dev/TECHNICAL.md:131).
- [S39] a killed run's `<id>.<pid>.tmp` folder is never swept; `store` removes only its own pid's
  (dev/src/build_on.rs:136, asserted at build_on.rs:440).
- [S104 review] (guard, a spec call) a listed public base can redirect into the local network: `dist/src/lib.rs:280`
  follows up to 5 redirects without the A35/A37 rule for listed reads. Either name redirects under "Not protected" or
  check each redirect target.
- [S104 review] `localhost.` (trailing dot) gets past `local_base` (`dist/src/index.rs:577`): strip one trailing dot.
- [S104 review] once a listed record exists, a cache miss reads only the record's base, so Eve's own mirrors are never
  tried (`dist/src/fetch.rs` ~107-125, older than S104).
- [S105 adversary] (guard) dist's local ranges (5.9) miss 100.64/10, 198.18/15, 0/8, fec0::/10 and NAT64/6to4 forms of a
  local IPv4 address; add them (deputy decision 2026-11-23).
- [S108 review] `watane-dev check` scans for links itself before `Mod::read`, so a maker never sees the hidden-folder
  rule there; `watane-dev add` drops the provider's `Mod::read` error (both dev).
- [S109 review] (guard) the log directory's owner is never checked: root running over a 0700 log dir another user owns
  lets that user swap or roll back the identity key. A check that the dir is owned by the effective user.
- [S37] `{"release"}` in a home with no record answers 200 from dist's cache
  (app/src/api.rs:321, dist/src/fetch.rs:68). Documented as deliberate; no app test covers the
  link-then-release sequence.

### pkg

- [S119 r1] (polish) P1.4 `pkg/tests/it/package.rs:82-91,112`: the decode refusal loop should match "a Windows device name is reserved", as the pack test at `:136` does.

- [S110 R2] `pkg/src/package.rs:58` `grammar`: refuse Windows reserved device names (`con`, `nul`, `aux`, `com1`-`9`, `lpt1`-`9`, with or without an extension) on every platform.

- From S91: `watane-dev add` on a provider with invalid WIT says only "is not a mod folder"; a pack entry gone during listing is sometimes named at its parent; pkg/tests/it/s91.rs is a race test of up to 5 s in the default run.
- [S57] under a live swap race `add_provider` refuses safely, but with plain I/O text ("wit/deps: File
  exists", "No such file or directory").

### log

- [S112 R1.1] move the identity and store operations to `openat`/`renameat`/`unlinkat` on the checked folder handle (Unix and Windows); a spec call on refusing parents writable by others.



### host

- **(S136 r1 polish, left by S141)** P1.3 a doc comment on `firing-origin` in `host/wit/host.wit:133` and the maker's copy (a WIT change: S141 allowed none).

- [S126 r1] (polish; P1.1 and P1.4 done by S141) P1.1 `host/tests/it/https_stream.rs:1674` the "host stops reading" bound weakened to `written < EVENTS`: add a buffer-sized bound; P1.2 a removal timeout answers `internal` (host 4.4 names no code; a busy-mod kind is an interface change); P1.3 `.github/workflows/ci.yml:85,104` the S126 repeat steps are dead after merge: remove; P1.4 `judge_set` with an invalid origin lacks the editor's explanation; P1.5 `app/ui/app.js:132-136` "needs ." with empty hosts (unreachable today).







- [S111 r1] (host P1.1 to P1.5, P1.11, P1.14 found on main by S141; the dev items stand) P1.1 connect and TLS handshake each get their own `idle-s` (`host/src/agent/https_stream.rs:580-595`); P1.2 to P1.5, P1.11, P1.14: stronger and more host tests, a reset-before-head message; one sentence each in 19.4 for the accepted R2 and R5.

- From S91: an over-declared Content-Length whose body never arrives waits instead of a 413 at once.
- [S12a] `ac4_program_defaults_to_environment_home_and_port` probes port 47341 and then binds it
  (host/tests/it/program.rs:567); the probe-to-bind window stands.
- [S45] the 413 guard passes because the test tolerates a reset, not because the server drains the
  unread body (host/tests/it/program/api.rs:401; the server sets `Connection: close`). Worth knowing before
  trusting it under other guards.
- [S12a] an abrupt disconnect mid-`/call` has no test; `Served::shutdown` still documents a 5 s
  bound (host/src/server.rs:195).
- [S44] an unknown `/api` route answers 404 `bad_request` (host/src/server.rs:107, 245).

### dev

- **(S136 r1 P1.7, left by S141)** a mod disabled in a preview stays disabled across `serve`: re-enabling it with its state kept needs a new interface.

- [deputy-6, S127] dev 18.3's judge `Play::{Safe,Unsafe,Unsure}(reasons)` and 17 item 17's proposed verdict body are not the checked provider's wire shape: it speaks TypeSafe System One (agentic-harness Deputy decisions 2026-11-25), so its tests script `Raw`. Give `Judge` plays in System One's shape (a `noul` per question) once S127 has merged.

- [S119 r1] (polish) P1.2 `dev/TECHNICAL.md:89-91` dev-dependencies block lists `regex` and `tempfile = "3.27"` (Cargo.toml: no regex, `"3"`); P1.3 a unit test that `read_script` refuses `{"replise":[]}`; P1.6 move the nested unknown-field cases of `ac6_program_refuses_unknown_script_fields` to a unit test; R1.1 (accepted) one line in dev TECHNICAL 18.3 that extra script keys are refused.

- [S114 r3] (polish) `dev/src/preview/openai.rs:68,196`: pin that `fallback` rejects the plain scripted marker and a `Refusal::Scripted` round trip (`Refusal::tag`); pin or delete the `automatic()` check at lines 189-191.
- [S114 r3] (polish) `dev/src/preview/openai.rs:74-80`: the reuse warning for `Refusal::Scripted` says "Use a development build", drops the cause, and should name `watane-dev serve --stop` (the refusal stays until the port changes).
- [S114 r3] (polish) `dev/TECHNICAL.md:2993`: "program fingerprint" hashes the program path (plus script bytes), not its contents; say so.

- [S111 r1] P1.6 to P1.10, P1.12, P1.13: `scripted-responses` refuses unknown script fields, JSON error bodies, chunked-body and drop tests, key order, and dev TECHNICAL's stale test names and dependency block.

- From S93: search's no-result template next; local_dependency's fix has no command; text-mode
  fixes say --json; a publish --push success rerun gives version_exists; the preview host's error
  hides behind serde's "missing field mode" (dev/src/preview.rs:104); `new --dir` into a missing
  parent gives internal with no path; reads_outside names the cache path, not the maker's; folder
  name vs slug when the release's own slug is given; `add --release` shows "path: missing".
Re-checked and swept by S86 (2026-11-18); these remain.

- [S56] a relative `generate!` path can resolve into a stale cache sibling, giving a misleading
  `build.compile` (dev/src/builder.rs).
- [S86] publish keeps "Start the watane app, then run this command again" and the old
  `local_dependency` text for a null entry: dev/tests/it/publish.rs (:80, :111) pins both; the code
  marks them `// s86:`. A one-line change once the human's test-speed branch has landed.
- [S86] other prose fixes with no rerun: install's `host.no_answer` ("with a longer --wait"),
  dev/src/api.rs:180, 186, 240, builder.rs:168, outcome.rs:214, `build.reads_outside`'s fix;
  `check`/`new --dir X` `next` and `usage.no_source`'s `add --release` lack `--dir`; test/serve
  failing at check time give no rerun; the no-app `next` shape differs by command.
- [S86] the WIT column counts bytes, not chars; the message keeps wit-parser's `EscapeUnicode(...)`
  debug text; U+00AD, U+180E and U+3164 stay raw.
- [S86] fake-watane's never mode parks its one request loop, so later requests to that fake hang.
- [S86] a filter value starting with `-` (`--publisher=-x`) makes an unusable rerun.

### app



- [S117 r1] (polish) a channel both compromised and failing its last read: Updates says "Couldn't read … tries again" while the row says `Compromised`; nothing tells Alice why or what to do (APP-F-44 style words for followed rows).
- [S117 r1] (polish) `app/tests/it/manage.rs:317`: the polled `api.received` promise has no `.catch`.
- [S117 r1] (polish) `log/tests/it/identity_windows.rs`: the ACL oracle does not check the ACE's access mask.

- [S116 r1] P1.1 a mixed-plan consent test (no follow line for an Unchanged release's publisher); P1.2 the needless `#[allow(dead_code)]` on `app_home`; P1.4 the dist redirect test records calls instead of asserting inside the check; P1.5 OS failures in `check_owner_only` map to `Io`; P1.6 the `icacls` oracle on `parent` and `dir`; P1.7 a `#[cfg(test)]` STORE_WAIT of ~200 ms (net unit tests went from 1.1 s to 6.3 s); host TECHNICAL 1833 and 2411 still say a manager install 'always asks'.



- From S90: "Keep data, then reinstall marks storage `new`" needs a host task (`Host::has_state`,
  or excluding Storage from `new_grants` when the identity has state); the busy-port window keeps
  Install-a-mod and the link box enabled; a failed channel row shows dist's transport cause (a URL
  plus "os error 61"); a failed follow does not say "Couldn't follow X"; index rows show `ok`;
  /api/fetch non-JSON carries the parser text; the Settings site URL gives no confirmation; the
  channel memo never evicts (bounded by stored channels); on Linux a reused inode with the same
  mtime and length could keep a stale follow or base field; `Core::enable` can return an error
  after it succeeded when a consumer reload fails.
- [S44] `Core::mods()` re-reads every related channel view on each 2 s refresh, with no memoization
  (app/src/core.rs:276, 167, 1490; the poll is app/ui/app.js:156).
- [S58] reinstalling after "Keep data" marks the storage `new`.
- [S58] channel rows still show raw transport text.

### dist

- [S112 r2] (guard, a spec call) machine-local host names (`/etc/hosts`, the machine's own hostname, `ip6-localhost`) reach a local port through a redirect, since only literals are checked (TECHNICAL l.1376, DD-26): a check at connect time on the resolved address would close it.

- [S107 P3.3] `app/tests/it/find.rs:286` create the home in-process (`build_on.rs:167` `app_home()`) instead of an app launch.
- [S107 skipped] the Updates card hint for a refused legacy local base: 'open or paste the publisher's link again'.


- [S26] Q12: log entries of unlisted channels are never removed. `prune_listed_channels` touches
  only follows.json (dist/src/index.rs:463); log has no `forget` API (log/src/lib.rs:81).
  dist/TECHNICAL.md:1231 still carries Q12 open.

### install
- **(S135 r1 polish)** P1.1 to P1.14 of https://github.com/civic-tech-pioneer/watane/pull/49 (review r1): P1.1 `install.sh:1068` a folder relinked to the home mid-install still ends in a bare `~` (user-sim's red test `regress_s135_changed_during_install_to_the_home_has_no_bare_tilde`); P1.2 `:442`, `:444` two unpinned undo checks (mutants 9, 10); the rest wording, next steps in `--status`, and `slow:` reasons.

- [S123 r2] (polish) P2.1 `ac2_uninstall_keeps_a_relinked_created_folder` passes without the `! -L` and resolved-path guards of `root_folder_receipt_line`; test them through `keep_created_roots` on a reinstall. `install/tests/clean_mac.sh:13` raises SC2015 with ShellCheck 0.9.0 (CI's passes).
- [S123 r1] (polish) P1.2 uninstall still `rmdir`s an empty `~/.local/bin` and `~/.local/share` the person made before the install (now that receipts record who made a folder, record these too); P1.4 a blank line before `--- end of rustup's output ---` when rustup ends with a newline; P1.5 'outside your home folder ~' reads oddly (3.4.1 example and INST-S-31 with it).
- From S94: check_foreign's `pid_gone` skip has no unit case; 39 `slow:` reasons are generic (the
  rule is only in the `///` line); `target/tmp/install-cache-*` folders pile up per install.sh
  content; spec 3.6 step 0 vs recover (see S94); Windows CI red (report only).
- From S89: the hint's bare `watane-dev doctor` when it is off PATH; `^C` glued to the hint;
  a shell "Terminated" on TERM; uninstall leaves the empty `~/.local` and `~/Applications` it
  made, and says "uninstalled" even when entries were skipped; `--status` lists receipt paths
  unchecked; the post-undo "still works" can be untrue when files were deleted by hand
  (INST-F-18); a Ctrl-C during the rustup download prints no install-it-later line; offer B names
  sh.rustup.rs; a rustup failure (exit 6) names no step; the refusal shows the resolved home; a
  Ctrl-C at the uninstall question prints nothing; rustup's stderr can interleave the frame.
- From S50b-fix's user-sim: no hint after a Ctrl-C past the commit; `--status` does not clean up
  leftovers; the offer shows `WATANE_RUSTUP_URL` but "install it later" prints sh.rustup.rs; ^D at a
  question needs a newline; empty folders remain after an interrupted fresh install; a Ctrl-C in the
  first ~0.1 s, before the traps exist, kills the shell directly.
- From S50b-fix2's user-sim: `--status | head -1` prints `printf: write error: Broken pipe` before
  exit 141; a copied or renamed home with a stale journal or old receipt reads its own files as
  foreign (exit 8), since its paths are absolute.
- From S50c's user-sim: a late Ctrl-C leaves empty `~/.local/bin` and `share/watane-dev`; a Ctrl-C
  at the toolchain question does not say watane is installed; `skipped` and refusal lines print
  `/private/...` paths, not `~`; after an uninstall through a re-linked folder, watane's vendor
  stays there with no hint; rustup's own text lands in the middle of watane's output; an upgrade
  writes a new entry through a folder re-linked inside the home (specs silent); actions pinned at
  v4 run on forced Node 24 (warning). Also seen on v0.1.0's release run (checkout,
  upload-artifact, download-artifact).
- From S55: the release app's consent window is never clicked in a VM (Bob's app there is a driver
  build); `clean_mac::ac1_ac2_fresh_home_real_release` fails in every fresh worktree without
  `target/release`, so check.sh reports CHECK FAIL for install tasks there.

### net / node

- [S124 r1] (polish) P1.1 dist: a write failure after the temp file exists has no test (needs fault injection). P1.2 app: pin the failed-install message at the app entry point (the regression test calls `Node::fetch`). The 20-runs-per-OS budget check ran 20/20 on Linux only; macOS and Windows passed CI once each.




### Unsettled

- [deputy-6, agentic-harness S128] (Door C, held) the kernel cannot know the owner of a timer firing or a restart pass: watane reports a timer callback's caller as `host` and keeps the resolved origin inside the host. agentic-harness F6.1 and case 22's idempotent half stay known limits until a watane slice gives a mod a read-only resolved origin for its timer firings (and an owner-scoped restart callback). An interface between projects: deliberate before sketching; not blocking any slice now.
- [S30] a fake-app publish took about 30 s, close to `--wait 30`. The code says that time is the
  pre-post test pipeline (dev/src/cli.rs:804), not a host wait. A timed `publish` run against
  fake-watane settles it.

## Foremen

Background CLI sessions; transcripts in `~/.claude/projects/<worktree folder>/<sessionId>.jsonl`.

- S4: `91605db7` (sessionId `91605db7-cf74-46c5-b1a9-dfff584ce505`), worktree `.claude/worktrees/s4`; died 2026-11-10 23:32 mid S4-2 validation.
  Resumed 2026-11-11 as `2066b4f1` (sessionId `2066b4f1-2ca3-4a1f-9fa3-29c671cea48f`); died after
  the user-sim's ACCEPTED, in its final test run; the product owner ran the tests and merged (stopped, removed).
- S5: `91dc7a35` (sessionId `91dc7a35-91d7-4d4d-a6a1-4f843f3fd54a`), worktree `.claude/worktrees/s5` (stopped, removed).
- S8: `44bfe2ab` (sessionId `44bfe2ab-9c0d-472b-9ebe-4495e4af90d8`), worktree `.claude/worktrees/s8` (stopped, removed).
- S6: `4a8d4ea4` (sessionId `4a8d4ea4-67ac-411a-bf4d-9ce82018000d`), worktree `.claude/worktrees/s6` (stopped, removed).
- S9: `e793310c` (sessionId `e793310c-21e9-4c6d-bddd-6e1180c23fe4`), worktree `.claude/worktrees/s9` (stopped, removed).
- S10: `a41a03d5` (sessionId `a41a03d5-4976-4920-bd4a-029b3628011c`), worktree `.claude/worktrees/s10` (stopped, removed).
- S11: `23325b72` (sessionId `23325b72-bf6f-4a9b-a8aa-846a03ff376c`), worktree `.claude/worktrees/s11` (stopped, removed).
- S14: `bffc598d` (sessionId `bffc598d-0fb1-4534-afc1-55371b4892e1`), worktree `.claude/worktrees/s14` (stopped, removed).
- S15: `1fd281db` (sessionId `1fd281db-b26f-4f32-903f-2a9a91d248f9`), worktree `.claude/worktrees/s15` (stopped, removed).
- S16: `c69c6b56` (sessionId `c69c6b56-1fd2-47b8-baf1-9cf4e88e95b5`), worktree `.claude/worktrees/s16` (stopped, removed).
- S12a: `48eb6176` (sessionId `48eb6176-882b-4e90-9154-7f8df7a7077e`), worktree `.claude/worktrees/s12a` (stopped, removed).
- S13: `93875efd` (sessionId `93875efd-ef7d-4863-a9eb-7cad24dda18f`), worktree `.claude/worktrees/s13` (stopped, removed).
- S17: `2894d2f6` (sessionId `2894d2f6-5753-4f2c-8189-388e2022683d`), worktree `.claude/worktrees/s17` (stopped, removed).
- S18: `2a36dab3` (sessionId `2a36dab3-8ee4-4149-a9b7-8446fa03e19c`), worktree `.claude/worktrees/s18` (stopped, removed; the product owner rebased it on main).
- S12b: `286375ae` (sessionId `286375ae-b3c5-4052-b381-a219b7cf0104`), worktree `.claude/worktrees/s12b` (stopped, removed).
- S19: `e64dcaff` (sessionId `e64dcaff-ea92-483f-b801-398757d83629`), worktree `.claude/worktrees/s19` (stopped, removed).
- S20: `e0535e0f` (sessionId `e0535e0f-15f9-4ed8-aa33-a7f69d14b020`), worktree `.claude/worktrees/s20` (stopped, removed).
- S22: `9b57b9f1` (sessionId `9b57b9f1-8099-46b7-866e-9f7548aae1b9`), worktree `.claude/worktrees/s22` (stopped, removed).
- S21a: `ebd6a828` (sessionId `ebd6a828-3a2c-457c-85ac-d8638809c277`), worktree `.claude/worktrees/s21a` (stopped, removed).
- S23: `48bcab4a` (sessionId `48bcab4a-1495-4425-9ae1-ba7f4d162caf`), worktree `.claude/worktrees/s23` (stopped, removed).
- S21b: `263282d0` (sessionId `263282d0-baa2-4b4e-bafa-1e8e8c8efa36`), worktree `.claude/worktrees/s21b` (stopped, removed).
- S21c: `d7c44cac` (sessionId `d7c44cac-2e5d-4889-86c2-0f58fd762649`), worktree `.claude/worktrees/s21c` (stopped, removed).
- S24: `914583bf` (sessionId `914583bf-3f00-4827-a975-2cfb3569557b`), worktree `.claude/worktrees/s24` (stopped, removed).
- S25: `dd222b8a` (sessionId `dd222b8a-1ffd-4037-a0d3-be25b4c43145`), worktree `.claude/worktrees/s25` (stopped, removed).
- S26: `cf5f9224` (sessionId `cf5f9224-c1de-440a-bacf-ab19ed939ca5`), worktree `.claude/worktrees/s26` (stopped, removed).
- S27: `5e751fa3` (sessionId `5e751fa3-948d-46da-84f1-0bb4795fce88`), worktree `.claude/worktrees/s27` (stopped, removed).
- S28: `0911854d` (sessionId `0911854d-ca31-4a6d-ae04-5713af9399b3`), worktree `.claude/worktrees/s28` (stopped, removed).
- S30: `62f3992b` (sessionId `62f3992b-6a28-4c7a-a7bb-9991ab5e6643`), worktree `.claude/worktrees/s30` (stopped, removed).
- S29: `4825c26d` (sessionId `4825c26d-3ec3-4617-8564-af8f2286a844`), worktree `.claude/worktrees/s29` (stopped, removed).
- S31: `a3423763` (sessionId `a3423763-65b9-4e29-b8f9-9662be460fb7`), worktree `.claude/worktrees/s31` (stopped, removed).
- S33: `b1cca282` (sessionId `b1cca282-654f-4013-b04d-90c75b3c4635`), worktree `.claude/worktrees/s33` (stopped, removed).
- S32: `56f5b095` (sessionId `56f5b095-5ed1-4f70-a1a6-fa2c02aa85ab`), worktree `.claude/worktrees/s32` (stopped, removed).
- S34: `cbe37314` (sessionId `cbe37314-fe72-4ca3-a22e-73a0a05538ac`), worktree `.claude/worktrees/s34` (stopped, removed).
- S35: `09a8efd4` (sessionId `09a8efd4-e0b5-4c6e-87c6-1c00df8e386f`), worktree `.claude/worktrees/s35` (stopped, removed).
- S36: `99f33ecc` (sessionId `99f33ecc-0bec-4a2d-bd29-fdf579515e2e`), worktree `.claude/worktrees/s36` (stopped, removed).
- S37: `7070e66c` (sessionId `7070e66c-5e79-4363-84cd-8a544cb03e1f`), worktree `.claude/worktrees/s37`; stopped 2026-11-13 after the WindowServer crash, resumed as `1484fbc7` (sessionId `1484fbc7-6e7e-41d4-8d3e-91870fc01fec`) under a fresh daemon (stopped, removed).
- S39: `bc6fb2c6` (sessionId `bc6fb2c6-9f91-4126-be2c-db0871ed1f61`), worktree `.claude/worktrees/s39` (stopped, removed).
- S40: `9b94b54e` (sessionId `9b94b54e-b9a1-4de5-8528-bf6defb724c2`), worktree `.claude/worktrees/s40` (stopped, removed).
- S44: `cfcd9edf` (sessionId `cfcd9edf-a754-4f25-b613-c0a08e8db786`), worktree `.claude/worktrees/s44` (stopped, removed).
- S45: cloud routine `trig_01FJ4xoUaWLQp17xmy4hvZaw` (done).
- S46: cloud routine `trig_013UXYFnD27HE48e8EW957Vm` (done).
- S41: cloud routine `trig_01U3nwcKTu8tAJq5owM9wKqg` (done).
- S47: `a53ccf69` (sessionId `a53ccf69-4e62-4282-ab17-9f6bb89b8139`), worktree `.claude/worktrees/s47` (stopped, removed).
- S48: cloud routine `trig_011mjvemHKPo9ej8gdsR7qHR` (done).
- S49: cloud routine `trig_012gxPCMrrWKr657qWjMVzf6` (done).
- S56: delivered 2026-11-14 and merged; routine `trig_01Gcw9f5gMY9qgT77QLVyPgf` finished, branch removed.
- S57: delivered 2026-11-14 and merged; routine `trig_019eZQGSqCu2ziUPVNcDiXD8` finished, branch removed.
- S58: delivered 2026-11-14 and merged; foreman exited, worktree and branch removed.
- S59: delivered 2026-11-15 and merged; routine finished, branch removed.
- S61: `585e3daf` (sessionId `585e3daf-6cf8-4f99-be6e-144904360f44`, resumed after the disk filled; first run `f08e7a50`), worktree `.claude/worktrees/s61` (local: the app). Delivered 2026-11-15 and merged through S64; worktree, its target and branches removed.
- S60: delivered 2026-11-15 and merged; routine finished, branch removed.
- S62: delivered 2026-11-15 and merged; routine finished, branch removed.
- S63: delivered 2026-11-15 and merged; routine finished, branch removed.
- S67: delivered 2026-11-15 and merged; routine finished, branch removed.
- S53: delivered 2026-11-15 and merged; foreman exited, worktree, its target and branch removed.
- S52: delivered 2026-11-15 and merged; foreman stopped, worktree, its target and branch removed.
- S64: delivered 2026-11-15 and merged; routine finished, branch removed.
- S65: delivered 2026-11-15 and merged; routine finished, branch removed.
- S68: delivered 2026-11-15 and merged; routine finished, branch removed.
- S70: delivered 2026-11-15 and merged; routine finished, branches removed.
- S72: delivered 2026-11-15 and merged; routine finished, branches removed.
- S66: delivered 2026-11-15 and merged; foreman stopped, worktree and branch removed.
- S73: cloud routine `trig_01CZpy9oswGijTpTZhRtsFVq` (handed out 2026-11-15), branch `slice/s73`.
- S74: delivered 2026-11-15 and merged; foreman stopped, worktree and branches removed.
- S75: delivered 2026-11-16 and merged; foreman stopped, worktree and branches removed.
- S77: delivered 2026-11-16 and merged; foreman stopped, worktree and branches removed.
- S80: delivered 2026-11-16 and merged; routine finished, branches removed.
- S83: delivered 2026-11-16 and merged; routine finished, branches removed.
- S81: delivered 2026-11-16 and merged; foreman stopped, worktree and branches removed.
- S82: `75eab872` (sessionId `75eab872-4a9f-4f3b-9cfc-c99497768b74`), worktree `.claude/worktrees/s82` (local: the app; builds in main's `target/`).
- S85 (delivered, merged; foremen stopped, worktree and branches removed). Fix: `e9fa2917` (sessionId `e9fa2917-4c33-4d1f-a490-22c54f425f97`): after the rebase on e8265e8, manager.rs fails 3 of 6 native runs on the branch (app not ready), main 3 of 3 green.
- S85: `73adea8f` (sessionId `73adea8f-b026-48f9-9cfc-e634ed31c2ce`), worktree `.claude/worktrees/s85` (local: Docker and the Mac).
- S84: `a2ce600f` (sessionId `a2ce600f-dee1-40fa-99fa-9a31f4de06d0`), worktree `.claude/worktrees/s84` (local: the app; builds in main's `target/`).
- S71: delivered 2026-11-15 and merged; foreman stopped, worktree and branches removed.
- S76: delivered 2026-11-16 and merged; routine finished, branches removed.
- S78: delivered 2026-11-16 and merged; routine finished, branches removed.
- S79: delivered 2026-11-16 and merged; routine finished, branches removed.
- S54: delivered 2026-11-16 and merged; foreman stopped, worktree and branches removed.
- S69: delivered 2026-11-15 and merged; routine finished, branch removed.
- S43: `dede2f86` (sessionId `dede2f86-53e1-4c26-afb4-81282077d13d`), worktree `.claude/worktrees/s43` (exited, removed).
- S42: `9e8c18fd` (sessionId `9e8c18fd-b2e9-4414-833c-1ecae7c77718`), worktree `.claude/worktrees/s42` (stopped, removed).
- S38: `e17c0e09` (sessionId `e17c0e09-86d9-424b-975a-e69ca8147d77`), worktree `.claude/worktrees/s38` (stopped, removed).
- S7: `871c5605` (sessionId `871c5605-a537-44d1-bd37-c2dc3068d11c`), worktree `.claude/worktrees/s7` (stopped, removed).

## Waiting on the human

- (2026-11-25, deputy-5, S122 merged: agentic-harness stage E's watane half is done) **A real judge account for V7.** Stage E's checked provider (agentic-harness) needs a judge service the provider can call with a key Alice enters; nothing in the factory can open one (an account is the human's). Which judge (Jev or another), and may the factory use a key of yours in its cloud tests? Blocks agentic-harness stage E, so step 6's JF1. **N5 revisit** (agentic-harness PLAN, "N5 The factory's implementer needs a shell"): you asked to revisit it when stage E starts, leaning (b); it blocks stage F.

- **V8 writer busy share (S96).** One durable write per batch costs 24.5% writer busy at stage A's load
  (budget 10%, which S96 applies to the no-write run). Options: (a) keep "durable first" (19.6) and
  accept the write run's share as N4's price (rec: a kernel batch rarely writes); (b) group commits
  across batches (a host change, weaker per-batch durability). Undo: (a) none; (b) a host write path.
- (2026-11-21, from S95; none blocks stage A's A1) Six security questions in host/TECHNICAL 18, each
  with its options there: Q38 the consent key names no provider (`via`), blocks A3 (answered (b) by the human, 2026-11-24: Human answers); Q39 the Claude
  login token and loopback under the shell grant (probe V28; agentic-harness S1's V27 found a
  loopback proxy breach), blocks the shell grant in A2; Q40 sessions read each other's scratch and
  run folders, blocks A2's scratch work; Q41 symlinks past the deny rules (V29), blocks A2; Q42 login
  files the closed deny list misses under the shell grant; Q43 the content shape of `say` and a
  lookalike `/` (V30), blocks A2. Also Q37: requests ride the call chain, but writes commit per
  invoke (host Q3).

- (2026-11-20, from S93; not blocking) The build boundary: wit-bindgen, run by rustc during a
  mod build, opens an absolute `generate!`/include path the mod names (e.g. a macOS autofs
  `/net/<host>/...`) before dev's dep-info check rejects it. dev's own check no longer touches it.
  May a mod's build reach such a path at all? Options: (a) refuse absolute or outside paths in
  `generate!`/include by scanning the source before the build (recommended: cheap, no build
  starts; undo: drop the scan); (b) run the build with no read access outside the mod and the
  cache (sandbox-exec on macOS; strong, platform-specific); (c) accept it as a known limit (the
  maker's own machine, their own mod). PO leans (a).
- ANSWERED 2026-11-19: (a), Tart (155 GB free). S50c's VM (asked by the S50c foreman): may it install Tart (macOS VM CLI on
  Virtualization.framework, Homebrew, FSL-1.1-ALv2 licence, tens of MB) and download
  `ghcr.io/cirruslabs/macos-golden-gate-vanilla` (macOS 27, ~30 GB download, ~35 GB on disk, 156 GB
  free)? The same VM serves S55. Options: (a) Tart + that image (recommended: scriptable, an APFS
  clone per run); (b) UTM (~250 MB app) + Apple's IPSW (~17 GB), harder to script; (c) Tart + the
  smaller Tahoe image (~25 GB). Verified 2026-11-19: `cirruslabs/tart` was transferred to
  `openai/tart`; its README installs `brew install openai/tools/tart` (2.42.0, 23 MB, plus
  `softnet`). Commands: `brew install openai/tools/tart`, then `tart clone
  ghcr.io/cirruslabs/macos-golden-gate-vanilla:latest watane-clean`.
  Undo: `tart delete watane-clean; brew uninstall tart softnet; brew untap openai/tools`. S50c builds everything else meanwhile.
- Step 7 (2026-11-14): three of its slices need something the product owner cannot provide.
  (a) S53: answered 2026-11-15 (org `minmax-fund`, `CLAUDE_GH_TOKEN`). (b) S54: answered 2026-11-15 (GitHub Pages in `minmax-fund`). Was: S54 needs somewhere to serve a channel for real
  (GitHub Pages under that account would do). (c) S55: answered (again) 2026-11-18: use a VM on
  this Mac; the human has no easy second machine. Not a question any more.
- S50c needs `civic-tech-pioneer/watane` public (2026-11-18). ANSWERED 2026-11-18: (a), plan for public. Secret scan done 2026-11-19 (S50c: gitleaks 8.30.1 over all 1032 commits plus a pattern sweep): no secrets; one false positive, a z-base-32 test key in app/src. Public with it: the author email on 1762 commits and `/Users/merlijn` in 2 paths (hiding them needs a history rewrite). DONE 2026-11-19: the repository is public (the human's go-ahead); the PO applied Q16: tag ruleset `release tags v*` (id 27621105: creation, update, deletion; bypass admin only), environment `release` (required reviewer mboogerd, deployment tags `v*` only), approval required for all outside contributors' runs, default token read (already). Not applied: Q16(3)'s branch ruleset on main (it blocks the factory's pushes). Next: the human cuts v0.1.0 when S50c is ready. S50c scans the history for secrets first; the human flips the visibility; then the PO applies the tag ruleset and the `release` environment. The repository is private on the free
  plan, so (1) `curl … | sh` cannot download its Releases without a token, and (2) GitHub refuses
  rulesets and environment reviewers there (403 "Upgrade to GitHub Pro or make this repository
  public"). Options: (a) make it public before S50c's first release, then apply Q16's tag ruleset and
  `release` environment; (b) GitHub Pro, and publish the archives somewhere public. Recommendation:
  (a). Cost to undo: (a) the history is public from then on, so check it holds no secret first;
  (b) a subscription. The read-only default `GITHUB_TOKEN` is already set. Q16's item (3), a ruleset
  on `main` requiring pull requests, was not applied: it would block the factory's direct pushes to
  `main`.
- S50a Q16 (2026-11-18, install/TECHNICAL 14): ANSWERED 2026-11-18, (a) all three. Repository settings before the first release.
  (a) a tag ruleset on `v*`, a `release` environment with a required reviewer, and a read-only
  default `GITHUB_TOKEN`; (b) none, the workflow alone. Recommendation: (a): the release token
  is the one key that ships code to every installer, and (a) means no push or build script can
  publish alone. Cost to undo: (a) three settings off; (b) nothing to undo, but a release cut
  without review cannot be taken back from machines that installed it. Blocks S50c only.
- S16/S17: should the host mark some MCP tools read-only (`readOnlyHint`), so harnesses like Codex
  call them without an approval setting? Options: (a) no hint; the template `AGENTS.md` documents
  the approval line per harness; (b) a mod declares read-only functions (a new `mod.toml` or WIT
  convention) and the host passes the hint on. Recommendation: (a). The hint is the mod's own
  claim, so a mod could mark a write read-only to skip approval, and (b) adds a format. Cost to
  undo: (a) nothing; (b) a format change in pkg, host and dev. S17 built (a).

## Human answers

- 2026-11-26: S120's Door H (where the factory mods live, `spec/design/factory/TECHNICAL.md` 5, first row): **one combined factory mod in one new repository**, W29 kept, "for now". Isolation of the foreman's authority comes from agentic-harness profile narrowing at stage F, not a second mod. The next factory design or build slice writes it into that row with spec-change and settles the "Planning and foreman split" Door C row to match.

- 2026-11-26: the Codex `watane` environment's secrets will not be rotated; never ask again. mboogerd/claude-plugin#6 (10-minute Codex timer) merged by the human.

- **2026-11-25, the human (to deputy-5): no test run in a cloud (CI or a cloud team) calls an external provider** (OpenAI, Jev or another judge, GitHub's API, any real service). Unit and integration tests use fakes: dev's scripted Responses server and scripted judge, fake `claude`, fake-watane. A check against a real provider is a deputy probe on the Mac (V7 on Jev). Checked 2026-11-25: no workflow in watane or agentic-harness passes a provider secret to a test.

- 2026-11-24 Codex Daybreak (authorized security work) is set up ("Done with daybreak"): security rounds go to Codex again; a round it withholds after this goes to the human.

- 2026-11-24 host 18 **Q38: (b)**, the human ("b is fine for q38"). A workspace or account consent is kept per (origin client, `via` = the provider's identity, folder or account); a new provider asks again. S103 built (b); its next answer round writes host 18 Q38 as decided by the human, 2026-11-24, and the 19.7 rows to match. agentic-harness TECHNICAL 8 already cites 19.7.

## Deputy decisions

- 2026-11-26 (deputy-7) S130 R2.1 accepted: a forged `resolved` receipt line needs an attacker who can already write the home, outside spec/TECHNICAL 6's Install-row attacker (a tampered download); not new in S130. 8.5 rule 4 gets one sentence that the receipt is assumed watane's. undo: rule 4 also requires `<R>` = the home folder or its in-home link target (a Leftovers line).
- 2026-11-26 (Deputy Rock) S130 P3.1 covered by R2.1: a forged `receipt.new.<n>` or receipt folder needs write access to the home, the same as R2.1, outside spec/TECHNICAL 6's Install-row attacker; 8.5 rule 4 names them. Clause: the accepted R2.1 and TECHNICAL 6. undo: keep the literal-path check for the receipt folder.

- 2026-11-26 (deputy-7) S129 R1.1 to R1.3 accepted: R1.1 is host 19.9's stated intent (a fixture key, preview only); R1.2 the ready line's writer is the maker's own binary (spec/TECHNICAL 6 out of scope); R1.3 outcome (2) is checked in watane only as serve's `judge_origin`, the provider page is agentic-harness S127's. undo: a test slice for (2) on the app.
- 2026-11-25 (deputy-6, S126 r1) accepted: R1.1 a failed `timers_removed` inside a running host (a redb write fault, no crash) leaves timer rows a same-identity reinstall inherits until the next open sweeps them (no named attacker can fail the write); R1.2 `remove` can be starved by a mod that keeps its call lock busy (availability only; `disable` first lets it through). Undo: sweep or refuse an identity with rows before an install commits; a busy error kind.
- 2026-11-25 (deputy-6, S125 R1.4) accepted, rung 1: a judge origin with an unspecified, loopback or private literal (`https://0.0.0.0`, `https://0x`) is not refused; the participant types it and the provider needs a matching consented Http grant, and a self-hosted judge on the LAN stays possible (host 19.4 judge binding; dist 5.9 guards reads, not this). S126 writes it under host 19.4 "Not protected". Undo: refuse those literals in `judge_origin`.
- 2026-11-25 (S125 r1) accepted: R1.1 two Windows host timing flakes outside the slice (rerun green; Leftovers guard); R1.2 run threads may outlive the home lock at drop (unchanged behaviour; a live run caught at drop); R1.3 the explicit unlock assumes no fork without exec (no such path); R1.4 a judge origin of `0.0.0.0` or `0x` (needs the participant's own Http grant for it; predates S125; Leftovers spec call). Undo: the Leftovers items.
- 2026-11-25 (S124 r2) accepted: R2.1 net's byte-budget test can still fail if the bucket refills before the probe on all five attempts (a guard test, not product behaviour). Undo: a test clock in net's budget.
- 2026-11-25 (S122 r1) accepted: R1.1 a kept judge key stays in use, its section hidden, after `claude-code` is withdrawn (W39 holds: it still goes only to its judge; the section fix in Leftovers); R1.2 local folders with one slug share one identity and so one key (host 4.1, by design, confirmed at install); R1.3 a judge folder with too-wide modes, or a prune I/O error, starts the app failed (fails closed; only a local program can cause it); R1.4 a caller holding a key learns which granted host is its judge origin (low value). Undo: the Leftovers item; a host refusal of `judge` without the grant.
- 2026-11-25 (S118 r1) accepted: the existing unsigned `local/<slug>` identity rule stands; a different local package with the same slug can recover retained records after Alice approves Storage.
- 2026-11-25 (S117 r1) accepted: R1.1 net's `cfg(test)` 200 ms STORE_WAIT applies to every net unit test; a store shutdown over 200 ms on a slow Windows runner could bring back `Database already open` in `no_default_services` and `ac2_the_book_decides_the_switches` only (fix then: give the wait only to the forced test).

- 2026-11-25 (S116 r1) accepted: R1.1 every `HostError::Refused` on the interactive app install path reads as a decline (the host can refuse there only because Alice declined); R1.2 a manager install with a New or Update entry follows the publisher with no follow line in its consent (app TECHNICAL by design, same class as S113 R2.1, now limited to changed releases).

- 2026-11-24 (S114 r2) accepted: R2.1 the home lock may close just before the database if a thread holds the database past the host at process exit and a restart follows at once; R2.2 a dangling `scripted-responses.json` symlink reads as no script; R2.3 by design (host 19.9) a request without `account: "openai"` to api.openai.com still goes to the real host, with no token.

- 2026-11-24 (S113 r2) accepted: R2.1 a no-op managed install (a hostile mod holding Manage, a deliberate unfollow, an approved read prompt) re-follows the channel Alice unfollowed; already on main; nothing installed, no grant or given base. 'After host.install succeeds' does not yet require a plan entry to change. Undo: follow only fetched items whose entry is not `Change::Unchanged` (Leftovers).

- 2026-11-24 (S111 r1) accepted: R2 api.openai.com response headers (openai-organization, openai-project, set-cookie) reach the mod: account identifiers, not credentials, and a mod cannot send Authorization or Cookie (one sentence in 19.4; the fix, if ever: drop them in `head_item`); R3 extra request headers on POST /v1/responses (the host sends only the checked route; the fix, if ever: an allow list); R5 the request body can start OpenAI work that outlives the stream (plan usage, section 6 Not protected); R6 underscore header names (no proxy in the host's client).

- 2026-11-24 (S112 r1) accepted: R1.1 a log folder swapped after its owner check (root over a home under another user's folder plus an active race; openat later, Leftovers); R1.2 embedded IPv4 forms needing a translator or tunnel on the participant's network; R1.3 the participant's own proxy variables (spec 6). P1.6: no compatibility owed before 1.0, a package with a device-name path no longer decodes.

- 2026-11-24 (S103 r2) borrowed authority (a tool running as its handle's origin client) may only do work that ends with the call: `agent-tools.open`, timers and streams are refused under it (host 19.5). Clause: W39 (a mod in the middle gains no authority), HOST-F-68. Rulings narrow: no binding of deferred work to consents. Undo: lift the refusal once deferred work is bound to its source consent. Accepted: F2.3, a mod update racing a queued tool dispatch gives a generic error instead of `unknown-tool` / -32602 (still refused).

- 2026-11-24 (S110 r2) accepted: the workspace's `unsafe_code` is `deny`, not `forbid`, so that `watane-log-windows-fs` can allow it; that it stays the one such crate (spec/TECHNICAL 2) is kept by review, not by the build. Undo: back to `forbid`, with windows-fs listing its lints itself.

- 2026-11-24 (S103 r1) accepted: a picked path changed by another same-user program after the last canonical check while a consent waits (host 14, Not protected); the pre-initialize Stop gap (an interrupt during a consent wait leaves the question open) goes to agentic-harness's console (its Upkeep: Stop maps to cancel before `initialized`).

- 2026-11-24 (S110 r1) F1.1: `watane-log-windows-fs` is the shared Windows platform crate under `log`, used by net, dist, host and app, and the one place `unsafe` is allowed (FFI to the Windows security APIs); spec/TECHNICAL 2, log AGENTS and TECHNICAL 6, 10 say so. Clause: spec/TECHNICAL 6 asks the key file's guarantees on Windows (the human's S110 request); no VISION clause against. Undo: move the code into log behind cfg, or a vetted crate.
- 2026-11-24 (S110 r1) accepted: R1 pkg's Windows walk follows a link swapped in during a walk (a running program of the same user: spec 6 Not protected; one known-limits line in pkg TECHNICAL); R2 DOS device names in a package path on Windows give a broken mod, no data read or replaced (the grammar fix under Leftovers).

- 2026-11-24 (S103) host 18 Q49 (a) and Q50 (a) accepted: each was open with (a) recommended, the stricter option, cheap to undo (deny lines; one table and one check); S103's foreman built them. Q38 stays the human's (Waiting on the human); S103 built (b), which keeps (a) open. Undo: as host 18 states for each.

- 2026-11-24 (S107 r3) accepted: a base the participant gave for a channel survives an unfollow while an index still lists that channel, and a later derived re-follow reads it unlisted again. Needs three events together, the participant's own base for the same channel, no new address for Eve. Undo: clear `explicit_given` on unfollow (dist follow.rs).

- 2026-11-23 (S105, the 'gone' filter) Accepted until S107: the 32768-bit gone filter saturates under listing churn, after which a new channel's link at a LAN mirror stays a listed read (availability only, fails closed; same kind as the seen_full lock).
- 2026-11-23 (S100, ninth adversary pass) Not accepted: `clean_runs` killing a recorded pgid at open without checking it is still this home's run, and `probe()` killing its group after the reap. Same class as the stop() pgid reuse the human vetoed; added to S100's last answer round as item 5.
- 2026-11-23 (S109 R1.1) Accepted: on macOS the key-path open can reach a device driver's open routine before the fstat refusal. Planting a device node in the user's 0700 log dir needs root, which spec/TECHNICAL 6 does not protect against, and nothing is read; cheap to undo (a symlink_metadata pre-check).
- 2026-11-23 (the human, with the monitor; watane-mods PR #5) New review rules from the next dispatch: the review lead
  triages its agents' findings into blocking and polish; polish never holds a merge and goes under Upkeep; the lead may
  propose to accept a blocking finding when the fix costs more than it is worth for the vision (a security finding only
  if cheap to undo and outside the vision's promises, else the human decides); no fixed round cap: each review ends with
  the trend and the lead's call, and from round 4 the deputy writes why one more round is worth it; the deputy rules when
  a finding comes back twice. The human: "find a balanced perspective on building out many features of the product
  versus building few features perfect".
- 2026-11-23 (the human's answers, through the monitor) S100-CAP: "One more round": one answer round for F5.1 and F5.2,
  then a sixth Codex review limited to them and their tests. S105-CHURN: "(a) Fail closed", built in S105's r2 answers.
  S100 risks: two vetoed and folded into S100's last answer round, off host 14 'Not protected': stop() killing a process
  that reused the pgid (clear the group at reap), and kept ask input counted by JSON length while held parsed (count what
  is held, or keep the raw bytes). Accepted and dropped from reports: the detached grandchild holding stdout, the symlink
  swap before claude's open, one stream filling its client's budget.
- 2026-11-23 (S105, two RISKs of option B, both closed by S107) (1) tombstone churn: NOT accepted by the deputy after
  all (the monitor's question): a pruned listing base that a stale hit gives is then read as given, and a given base's
  redirects are not checked, so it can lead dist into the local network. Under Waiting on the human (S105-CHURN);
  meanwhile the builder fails closed in the r2 answer round. (2) the seen_full lock: accepted until S107: a channel churned past 16 bases keeps a new mirror the participant gives as a listed read
  (availability only: it cannot reach the local network).
- 2026-11-23 (S105 BLOCKED: how dist knows the participant gave a base) Option B now, in dist/ only: each record keeps a
  capped 'seen' list of every base it ever held (kept across eviction, a tombstone after prune); a link at a seen base
  never given does not give it. Option A, the root fix (a follow from a derived link, a search-hit install or the app's
  follow_fetched, never gives its base; a dist<->app interface change), is sketched as S107 after S102 merges.
- 2026-11-23 (S105, the adversary's two RISKs) (1) dist's local ranges (5.9) grow to cover 100.64/10 (CGNAT), 198.18/15,
  0/8, fec0::/10, and NAT64 (64:ff9b::/96) and 6to4 (2002::/16) addresses that wrap a local IPv4 address: an upkeep item,
  not in S105. (2) A `follows.json` written before S105 may keep a listing's base on a followed record. First accepted as a
  known limit; revised the same day after Codex's S105 r1 (F1.1): a base of unknown origin (any base in a record saved
  before S105) gets the listed-address rule, redirects included, until the participant gives it again. No format change.
- 2026-11-23 (S105) dist follows a redirect from a listed or hint base only to a target that passes the same local-address rule as the base (A35/A37); redirects are not left under "Not protected".
- 2026-11-23 (S102 BLOCKED) A slice scoped to one folder may still change the root `Cargo.lock` and `workspace-hack` for its own dependencies: they are generated, and a rebase regenerates them.
- 2026-11-23 (the human's answer, through the monitor: "Yes, tell it"; on throughput) Last night ran only two
  slices, one per project, with nothing sketched ahead; earlier nights merged 10 to 17 a day. Follow deputy
  section 1 strictly: as many slices in flight as can run without sharing files, across both projects, plus
  one sketched ahead per project; look past the stage B chain for independent work (upkeep slices from
  Upkeep and Leftovers once about 3 items wait, spikes, other subsystems the vision orders next); Codex builds
  two in three, spread over the builds. This does not approve agentic-harness S35-MERGE.
- 2026-11-23 (the human's answer, S101 accepted risk: root adding `::1` after bind, host 8.1 and 14 'Not protected'): "Let's park this one as a non-security risk, not something that I want to regularly revisit, so no need for it to be confronting me with this every report. I will revisit it myself at some point; it's good to keep in general certain decisions in logs that are known limitations or risks." Stays logged where it is; not in reports or status lists. The same holds for every accepted risk or limit: log it once where it belongs, never re-surface it in reports.
- 2026-11-23 (S100, H2 wire form, cross-project): a file-tool `ask`/`check` item carries a top-level `"path"` with the absolute path, symlinks resolved; `input` stays as `claude` sent it; absent when unresolvable or refused. This is the form agentic-harness TECHNICAL 9.6 left to stage B's first slice; the kernel (agentic-harness S36) reads it. A `can_use_tool`/`hook_callback` before `system/init` passed is denied at once, pushing nothing. Approved by the human 2026-11-23: "Sounds good. Approved." No longer a pending veto.
- 2026-11-22 (the human's answer) Auto-merge is retired: the `codex-push` rulesets (watane 27841000,
  agentic-harness 27841001: only `slice/*` without bypass, for the Codex push App) block it, so the PO merges
  each green PR with `gh pr merge <n> --admin --rebase`. Foremen open their PR and do not arm it.
- 2026-11-23 Main went red on RUSTSEC-2026-0594 (`cookie` 0.18.2): the deputy cherry-picked S100's lockfile bump to 0.18.3 (PR #16, 749fdfa) so other PRs' lint is green before S100 merges.
- 2026-11-22 Main went red on RUSTSEC-2026-0592 (`url` 2.5.8): the PO bumped `url` to 2.5.9 (PR #12).
- S96 (2026-11-22): Q46 confirmed: a mod stream depends on the consumer's `Call` grant and the link `open`
  ran through; a side disabled and enabled again while an `open` is in flight keeps the stream (host
  19.2). Q47 and Q48 are fixes, not questions: they go to Leftovers for the next upkeep slice.
- S50c (2026-11-19): GitHub retired the `macos-14` runner on 2026-11-02, so release.yml builds on
  `macos-15`; the deployment target stays 14.0, built for but not tested (install/TECHNICAL Q4 says
  how to raise it to 15.0). Rejected: raising the target to 15.0 now (drops 14 users for no test
  gain).
- The human (2026-11-16), test time: an app slice ran the whole window suite about six times
  (each validator, the foreman, the PO). Now a validator runs only its touched test files
  (`--features driver --test <file>`, app/AGENTS.md), the foreman the whole suite once, the PO only
  what changed since. Next, measured after S84: (1) the driver suite without `--test-threads=1`;
  (3) `app/scripts/linux-test.sh` (Docker, xvfb) as a way to run app slices off the Mac or in the cloud.
  Measured (3), warm, on 2c820c3-era main: the whole app suite (default + driver with ignored, front
  skipped) in 5 min (cold build 4 min more); 393 passed, 4 failed on Linux only: j10 ac3, s44 ac2 and
  s39 ac3 (the main window never reaches its area), machines s28 ac1. linux-test.sh had no
  `site.localhost` since S54 (fixed). The 4 are a later app slice (Linux, stage F).
  Measured (1), native, quiet Mac, 26a728a: the app driver suite with ignored (front skipped), 277
  tests, takes 340 s with `--test-threads=1` and 265 to 270 s in parallel, green 3 of 3. So no one runs
  the app suite with `--test-threads=1`. The gain is small because cargo runs the test files one after
  another and a few journeys take 30 to 40 s each; the big gain was running the whole suite once per slice.
- S83 (2026-11-16): the app learns of an install over POST /api/install through host
  `Confirm::installed(&Installed{requester,new,updated})`, a default no-op method called once after each
  install that wrote the catalog (host 12.1); the app applies the main-mod rule there for every requester.
  Additive: undo is removing one default method. Not (b) app memory (lost if the app quits, decides before
  success) or (c) an exception for programs. Built in S84.
- The human (2026-11-16) answered step 10's questions (host/TECHNICAL 18): Q28 (a) every removal is a
  request; Q29 (a) enable is a request; Q30 (b) a manager may ask to install from any link, after a
  first consent "<mod> asks to read <link>"; Q31 (b) a direct `open-page(local)` that opens no window
  while a consent is open; Q32 (b) "Deny and stop asks from <mod>" in the consent view, which
  withdraws `Manage`; Q33 (a) as for every grant. Q30, Q31, Q32 are built in S82.
- S54 (2026-11-16): dist retries an HTTP 5xx twice with a short backoff, within the fetch timeouts (a
  Pages site published minutes ago answers 503); a link naming a new base for a channel dist knows adds
  that base to the record's bases (5.9: learned bases are appended; every base is checked). Built in S79.
- S64 (2026-11-15): a `Kept` ingest that fails with a gap and stores nothing removes its `kept`
  record, since nothing is kept; `DistHoldings::set` a second time is an error. Built in S70.
- S53 (2026-11-15): with the publish-under-an-org question open, S53 ran J5 up to Carol's publish and
  left the fork to fake-gh (option b), so no public flag was added before the human answers.
- S59 (2026-11-15): accepted the slice's own reading that none of its choices is hard to revert. Two
  come closest and are named for the human. `kept` is a new field in follows.json with a serde
  default and the version stays 1, as step 4 did it: older homes ignore it, so undoing it costs
  nothing. `Log::drop_identity` deletes a key, but only one that has signed nothing, before an app
  home joins as a replica (NODE-F-12): nothing published ever named that key. Both are on disk only;
  neither is carried in a published channel or on the wire.
- S43: `share::fork` takes the ReleaseRef (a `Package` has none; dev TECHNICAL 12 corrected); the
  clone gets the `http.lowSpeed` limits; `publish.fork_unavailable` names `<uo>/<un>`; a rerun
  after a declined first push reports `created: false, forked: false`, because `mod.toml` already
  names the fork.
- S48: dist `NothingToUnfollow { channel }` (unfollow only); DD-34: a provider base ends the reads
  only when its stored view then holds the wanted release; host messages show `<fp>/<slug>` (the
  JSON `missing` keeps the full key); candidates sort by natural order.
- S44: `consent::Party` gains `you: bool` (app-internal); names come from a parties cache filled by
  local `dist.channel(key)` reads; wrong methods on `/api/*` answer 405 `method_not_allowed`.
- S45: `HostError::AmbiguousProvider { consumer, provider, candidates }`; `DistError::NotFollowed
  { channel, local_only }`; dist DD-31 to 33 (hint bases in trust order, stop at the first Current
  or Behind; the no-record cache-hit link is the first base in trust order).
- S40: `new --from` runs no gh; the fork uses the forks REST API (`gh repo fork` refuses `--remote`
  with a repository argument and silently renames an existing fork); an unfinished fork of the
  right parent is adopted; tags are the ReleaseRef; `--push` always needs gh (W30); a failed push
  is finished with the commands in `publish.push_failed`, not a rerun.
- S46: a link anywhere below ui/ or wit/ is rule path at the link's own path; below a refused WIT
  directory the reader still descends without following links; a non-hidden special file below
  ui/ is rule path, hidden ones are ignored; release decode errors read "<field>: expected <what>,
  found <what>".
- S40 (the human, 2026-11-13): step 6 uses real GitHub forks. `new --from` clones and verifies
  (no gh); the derived mod's first `publish --push` runs `gh repo fork`, pushes to its own default
  branch; S40's own-repository path stays as the fallback (fork exists, upstream gone, no upstream).
- S39: app 503 `unavailable` -> `env.host_not_running` (exit 3); other app 5xx -> new `host.failed`
  (Refused, exit 4); 4xx stays `host.refused`. Every 10.1 answer check -> `host.refused [bad_answer]`
  with an "update the watane app" fix. dev maps pkg's "defined in two locations" for its own
  package to the own-package violation (no pkg change). All in dev TECHNICAL.
- S37: a driver build opens dist with `listed_local` only when `WATANE_DRIVER_LISTED_LOCAL=1`, so J5
  serves channels on 127.0.0.1; a release build never reads it (app TECHNICAL 10). J5 posts packages
  straight to `/api/publish`.
- S36: dev reads only the key and slug of a link text (dev cannot depend on dist's Link); the app
  validates the link (dev 10.1). fake.json `fetch` is keyed by link text or ReleaseRef, values
  {release, package path}; an unknown key gets 409 `fetch_refused`; fake.json `providers` goes into
  the publish answer (dev 15.3). `ac10_endless_test` split: the default suite asserts code and
  message with a 60 s guard, the timing test keeps 7 s. S33's spec debt fixed (pkg 4.7, dev 2.4).
- S35: dist 5.7: a listed record whose stored view lacks the release is also read from provider
  bases; the package comes from the first base in trust order (5.11 step 5); on a cache hit a listed
  record's link is the first provider base in trust order. `read_link` of a recorded channel reads
  the record's base.
- S34: new `HostError::BadRequest` (`bad_request`, 400) for an invalid bind. 4.5: a bind's `to` must
  be installed or in the same plan; at most one bind per consumer and provider; remove never fails
  on a link (the provider stays unbound), a plan still refuses ambiguity.
- S33: the 30 s publish was the cold Rust build and test pipeline, not dev (a warm one takes 4 to 6
  s). `search` accepts `--dir`, ignores it, and hides it from its help. Generic app-error fix: "The
  watane app could not complete this command; ask the person to check the watane app".
- S29: dist `IndexView.skipped` is a list of `{channel, cause}` (own channel, local address, over the
  listing limit); the app shows `IndexRow.skipped: [{party, cause}]`. dist error texts name a channel
  by fingerprint (fields keep the key). `/api/search` answers 400 only for a bad query, else 500
  `unavailable`. The own index row counts live `list` listings. Consent titles: "List in your
  index", "Include in your index", "End a listing". `hit.link` keeps the record's front base.
- S31: pkg owns two fixed messages, the pkg/dev interface: `deps.json lists mods:<slug> but
  wit/world.wit imports none of its interfaces` and `deps.json lists this mod itself as
  mods:<slug>` (own wins, one violation); dev's WIT reader is gone. `fake-watane` answers
  `/api/search` with fake.json `"search"`, default `{"indexes":0,"results":[]}`.
- S30: `publish.version_exists` keeps its code and exit 1; the release and link go into `result`
  (2.3 allows a result on a failure). `local_dependency` does not query the app (DEV-S-30: the host
  receives no request). `link: none yet` applies to publish's link only; the global `null ->
  missing` rule stays.
- S28 (validator escalation): a listed channel is read from every listed base (at most 4) on each
  read, the best outcome wins, any `Current` sets `last_success`, and the base with the highest head
  moves to the front. Stopping at the first `Current` let a stale mirror hide a withdrawal. Up to 4
  reads per listed channel per refresh, under the caps (dist TECHNICAL 4.4, 5.9, DD-25, DIST-F-49).
  `list`/`include` hold the own lock across their validation read and refuse a compromised channel.
  Read budgets span the refresh passes. Search filter tests use prebuilt components
  (`dist/tests/fixtures/search/*.wasm`, PREBUILT.md).
- S27: a busy port shows "Port P is in use, maybe by another watane app. Start this one with
  --port <n> or WATANE_PORT=<n> ..." in the window and once on stderr; the app does not fall back to
  another port itself. A decline is recognised by host's `refused` code, kept in memory only;
  `ModView.offerGrants` lists the offered release's grants that the installed mod lacks. dist's
  BadReason maps to `bad_input` with app words. `app/ui/words.js` holds UI text helpers.
- S24: new route `GET /api/releases` (`{"releases":[{slug,version,release,seq,link|null}]}`); a
  duplicate publish answers 409 `version_exists` (was `publish_refused`); both additive (app
  TECHNICAL 8.1.1, dev TECHNICAL 14). dev checks the release list after the status check and before
  the build (`publish.version_exists`, class 1, with the existing link). A late approval is handled
  by the rerun reporting the release; host is untouched. Builder progress is one `building <slug>`
  line; text mode prints object lists with indexed keys (`pages.0.slug`).
- S28: dist's new `Statement` variants and `ChannelView` fields break app's build. S28 edits app in
  two places only: `confirm.rs` refuses an unknown statement kind (`_ => Refuse`, `ponytail:`; S29
  shows index statements), and `core.rs`'s test helper gains the three fields.
- Human (2026-11-12, process): briefs get a "## Changed tests" section (foreman skill); the
  "Spec gaps" logs in */AGENTS.md are removed (the spec edit, Sol's CHANGED SPECS and git history
  are the record; host Q ids and dist DD ids stay in their TECHNICAL); new worktrees clone main's
  build with `cp -c -R target`. Declined: a check of Sol's unlisted files (20 such files in 17 of 62
  runs; validators accepted nearly all and returned 2), so the validator stays the only check.
- Human (2026-11-12): a design or spec change that is in line with the specs, passed review and
  improves on what was there needs no separate approval. A change that is hard to revert still
  goes to the human, briefly: what it is, the alternatives, and the consequences of each. S26's
  formats are used by no published channel yet, so they are cheap to change now; S26 merges.

The human may veto each one.

- S1: one root Cargo workspace (resolver 3, one `Cargo.lock`, shared `target/`).
- S1: the host installs a folder by copy with a temporary package digest until pkg `pack` lands
  (host §15, Q12); only throwaway preview homes store it.
- S1: host recovery restores a committed `.old` mod before deleting leftovers (Q13); the home lock
  is held before the marker is created (Q14).
- S1: `run.json` means "listening"; only the ready line means "initial mods installed".
- S1: `serve --stop` finds the preview by the mod path's hash alone, so it works on a broken or
  deleted mod folder.
- S1: `deps.json` refuses duplicate keys; a folder-level violation names file `.` (pkg §15).
- S2: `Host::open(home, Limits)` until home mode; `Caller` is only `Page`; no depth or hop limits yet.
- S2: event imports are linked as no-ops until S3; the storage grant check inside state waits for
  `revoke`.
- S2: install copies `logic.wasm` with the folder, then `check_component` and compile; mods with
  calls or subscriptions to other mods are refused until links land.
- S2: the engine turns Wasm threads off and refuses shared memory (host Q15 to Q17).
- S2: `check_component` checks the component header before decoding (pkg §15 item 16).
- S2: `serve` runs check and build only; the outside-read check and reproducible bytes wait
  (`ponytail`).
- S3: events reach only the emitter's own pages (`changed` and the events); subscribers wait for
  links. The page bus lives in `Host` (broadcast, 1024). host adds `tokio-stream` 0.1 for SSE.
- S3: the outbox holds JSON, not WIT values with hops (host Q18; revisit with subscribers).
- S3: ending streams on disable or remove, and hop and delivery limits, wait (host §15 S3 cut).
- S3: pages reload only when the package id changes.
- S7: `pack` checks paths (links, special files, names) before `Mod::read`, so a link gives
  `PackageError::Symlink` and special files are never opened (pkg §6.4, §15.17).
- S4: `serve` runs the whole `test` pipeline first; failing tests start no preview and leave a
  running one alone.
- S4: unit tests keep typed WASI instances linked (wasmtime 48's `define_unknown_imports_as_traps`
  breaks resource identities); non-WASI imports trap; TCP, UDP and name lookup are off.
- S4: `Harness` has only `call`; `deliver` joins with links (step 2), `functions()` with S5.
- S4: the outside-read check on unit-test builds stays deferred with DEV-S-10 (S8).
- S5: `Host` gains only `functions`, `set_toolset`, `issue_token`, `end_token`, `Served::mcp_url`;
  `remove_toolset`, `toolsets`, `tokens` wait for the app. `set_toolset("preview")` is `bad_name`
  in every home. `/mcp/<toolset>` is on the API host names with a 1 MiB body limit.
- S5: `Host::functions` is the listing; `Harness::functions` waits for `test.uncovered`.
  wit-parser is pinned `=0.254.2` (the version in `Cargo.lock`).
- S5: `serve`'s `next` is `watane-dev test --json` only when the test pipeline fails.
- S6: `fake-watane` moves to step 2; only the home-host scenarios (S-25 to 31) need it.
- S8: dev scans and clears both `<crate>.d` and `<crate>-*.d`, and touches the copied crate root
  so cargo writes fresh dep-info; only the mod crate rebuilds.
- S8: the host unpacks into a new `staging/<hex>/mod/`; the link refusal still runs before pack.
- S9: the program runs participant homes (`--home`, `Mode { Home, Preview }`); until the app's
  Confirm lands, a home acts as "no app": installs that need confirmation are 403 `refused`.
  No-arg `watane` opens the default home on port 47341 (host 15 "S9 cut", Q22).
- S9: repeated `Host` headers are 421; a missing page file is 404 on any method; the connection
  closes after a body-size refusal (host 8.1 to 8.3, Q21).
- S6: doctor's `next` is `watane-dev new <slug> --json` with no gaps, else `watane-dev doctor --json`;
  a missing vendor folder's fix is `<abs dev>/scripts/vendor.sh` in doctor, `test` and `serve`.
- S6: a refused `serve` warns `preview.previous` when a preview of that folder still answers.
- S10: `mod_disable` moved into stage A (fake Core) so app S8 is testable now.
- S10: `Info.plist` has an ATS exception for `localhost` and its subdomains (insecure HTTP only, no
  arbitrary loads); without it macOS loads no mod page. host Q10 verified for WKWebView (`::1`
  works); WebKitGTK still open (stage F).
- S10: the consent window's `Destroyed` handler denies and reopens off the main thread.
- S14: the foreman edited `factory/README.md` ("Front" row in the test pyramid), `validator.md`
  (never runs front tests) and `user-sim.md` (starts the app hidden) itself.
- S14: a driver op `screen` reports the mode and the real AppKit activation policy (app 10).
  Hidden WKWebView loads pages and runs timers on time, so only a self-test uses `background`.
  The front tests' idle wait panics after 10 minutes; it never skips.
- S15: `PackageError::TooManyFiles` (count 0 stays `Format`); `Path`/`Symlink` print the path
  with `escape_debug`; pkg adds `libc` 0.2 (unix) for `O_NOFOLLOW|O_NONBLOCK`, no `unsafe`.
- S16: Codex ran in its `workspace-write` sandbox with network (as Sol), with its own
  `CODEX_HOME`, so the human's `~/.codex` was untouched.
- S11: host fixture `list` renamed `shelf` (`list` is a WIT keyword, refused as a slug).
- S11: host links, missing providers, cycles and order are built on the preview `install_folders`
  path only; `Host::plan`, `Source` and confirmation stay with app stage B (S12).
- S11: no global call lock: per-mod call locks plus per-emitter ordered delivery.
- S11: `Harness::deliver(function, args)` takes `<short iface>.<fn>` of a subscription export,
  caller `mod:<provider slug>`. `changed` goes only to mods whose state bytes changed.
- S11: serve `next`: success gives the served mod's own page (the last entry, providers come
  first), null when it has no page; an `env.*` refusal gives `watane-dev doctor --json`; a
  provider's failure gives `watane-dev test --json --dir <provider>`.
- S11: pkg `add_provider` refuses a symlinked destination; an I/O failure may leave a partial copy.
- S13: dev tests share one warm cache folder per test binary (a copy at a new path builds cold:
  the path is in the rustflags). A command test may be removed once a unit test proves its rule;
  5 were, each named in TECHNICAL 15.2's replacement table. DEV-S-27 to 31 are marked planned.
- S23: `add` reads a relative provider from the current folder; when it is not a mod, `usage.provider`
  names the canonical path and this rule. The declined-install text says `--wait <seconds>`. The
  build progress line goes to stderr before every mod build (release and unit).
- S23: spec-gap lines come in each task's final message; the foreman writes them to AGENTS.md once.
- S21b: a reader that reads up to a new head also GETs `entries/<head seq>` and needs it byte-equal to
  `head`, else `HeadMismatch` naming that URL. `confirm_own` with another key's link is `NotOwnLink`;
  `WrongChannel` stays for a host serving another channel. A first follow returns every failure as
  `Err`; refresh reports `Failed` per channel; the app maps them to a command error and a row status.
- S21b: host recovery repacks a release-installed folder with `pack(dir, true)` (source kept); local
  mods still `pack(dir, false)` (host Q26). The app's API router takes a dist-only handle (no
  server/Core cycle). Restoring the same key again is idempotent; a different key into a home that
  has published is refused.
- S21c: dist error texts use `escape_debug()` with no Debug quotes, and a variant never prints its
  own `source()` (`Io` shows only its path; `Log`, `Package`, `Release` transparent; dist TECHNICAL
  3.1). The app's `cause_chain` appends a cause unless the text so far equals it or ends with ": "
  plus it (app TECHNICAL 13). dev `publish` checks local providers, host status, the pipeline,
  pack, then posts. DEV-S-31 for a logic mod is `slow:`; the default suite proves `--no-source`
  with a page-only package. The template guide does not mention publish (cap; `--help` covers it).
- S25: refresh notices and offers are derived from dist's ChannelView each time, not stored (only
  "Update to X failed" is kept, in memory). An update starts only on a `Change::Release` in that
  refresh, for the newest live version; a declined update is not asked again. The offer is the
  newest live version even when it is a downgrade, quiet when it adds no grant. A compromised
  channel gets no automatic update. Updates run in their own tasks; a refresh never waits on
  consent. New commands `release_install`, `release_withdraw`; `status.identity.releases`.
- S47: a mod with `logic.wasm` and no `logic/Cargo.toml` is prebuilt (dev TECHNICAL 7.1, 10.2
  step 5): `test`, `serve` and `publish` skip the build and unit tests, keep the wasm and run the
  contract tests against it; `result.build` and `result.unit` are null. This reverses S11's
  stale-wasm rule (`contract::ac5_s11_test_rejects_stale_wasm_without_manifest` became
  `ac5_s47_test_accepts_prebuilt_wasm_without_manifest`). Deriving under a new slug keeps the wasm
  and warns `new.prebuilt_renamed` rather than refusing the rename, which would break J5.
- S42: the GitHub fork (the forks API, branch `watane`, `publish.fork_unavailable`) stays in dev
  stage 11 (S43), as the build plan and DEV-S-50/51 have it, against the S42 sketch: its tests need
  S43's clone folder. In S42 a derived mod gets a plain new repository (`forked: false`, no warning).
- S42: a prebuilt mod (a `logic.wasm`, no `logic/Cargo.toml`) commits its `logic.wasm`; a built one
  does not (dev 10.4.1 step 6, section 11). This answers S47's watch item.
- S42: the `publish.push_failed` fix is the failed command and the rest of its list, with no
  "Fix the cause" prefix; the message reads `<command> failed: <stderr>` (dev 2.2, 10.4.1).
- S50b (2026-11-18): the receipt records each install folder's resolved path at install, and
  uninstall skips a removal when the folder now resolves elsewhere (install/TECHNICAL 8.5 rule 3's
  open risk: a same-user link made after install, `~/.local/share/watane-dev -> ~/Documents`, lets a
  receipt line empty it). Built in S50c, before the first release, when the receipt format costs
  nothing to change; after it, a change needs a migration. Rejected: accepting the risk, and
  checking share/ lines against a fixed list (breaks 8.5's downgrade rule).
- S50b-fix (2026-11-19): macOS bash 3.2 can lose a Ctrl-C when a child exits 130. Fix it as a
  class (s50b-fix-7): every dropped or remapped status in install.sh goes through the signal check,
  and a test fails on a new site that skips it. If the next adversary pass still finds a site of
  this class (home intact; only the exit code and cause line are wrong), it becomes a known limit in
  install/TECHNICAL 3.8, its tests are `#[ignore = "known limit, S50c"]`, and the slice delivers.
  Not a weakened check: the rollback always finishes.
- S50b-fix (2026-11-19): two concurrent installs are excluded by a lock the script takes in the
  home before `check_no_live_install` (a `mkdir` lock folder, removed on exit; a stale one older
  than the journal rules allow is reported, not broken). Built in S50c, before the first release.
  Rejected: accepting the race (two runs can interleave swaps in one home).

## Friction

- (2026-11-25, deputy-4) The deputy skill's review brief names `.claude/factory/review-lead.md`, which exists in no repository (watane, agentic-harness, watane-mods) and not in plugin 0.1.1: S120's Claude reviewer fell back to the skill's own text, and S121's Codex brief carried the lead's rules inline. Factory change to propose: ship review-lead.md with the plugin or a project's factory copy. deputy-4 in the Desktop app dispatches fine; the --bg blocker is gone.

- (2026-11-25, deputy-4) A deputy started by the handover's `claude --bg` command has neither the browser pane nor the routine scope (RemoteTrigger 401), so it can merge and record but dispatch nothing. Factory change proposed to the human (Waiting on the human).

- 2026-11-24 (S112 r1) Codex's cyber filter withheld an answer round on dist's SSRF guards (third time after S105 r5 twice): a slice of network-boundary guards should start with Claude as builder. Same day: Codex also withheld S111's review (an HTTPS and account-token boundary), so S111 is reviewed by Claude, its own builder's vendor. Three blocks in one day on security work: proposed to the human (Codex 'Daybreak' for authorized security work is an account setting). (deputy-3)

- (2026-11-23, deputy-2) Codex withheld S105's r5 review behind its cybersecurity filter (adversary output); re-sent once as a new task asking for findings as defects and red tests, not attack steps. A second hit sends that round to Claude: it did (21:44).

- (2026-11-21, S98) no `gh` or raw job logs in the cloud (logs read through a subagent with GitHub MCP); CI's concurrency group runs one at a time, so 5 green runs took ~2 h; no IPv6 in the container, so check.sh reports 8 app tests failed there.
- (2026-11-21, S94) strace dies on a traced group's STOP/KILL (process counts need retries); RUSTC_BOOTSTRAP for --report-time rebuilt the workspace; no shellcheck in the cloud; a cloud slice's timing margins (2 s on 4 cores) failed on a loaded Mac, and a test expected `/private/var` where macOS shows `/var`: the fourth macOS-only miss from the cloud, now caught by CI's macOS job before merge.
- (2026-11-20, S93) twelve tasks in one sweep (ten fixes from user-sim and adversary rounds): cut sweeps smaller; clippy missing in the cloud toolchain; an implementer's helper claimed no filesystem call but canonicalized (validator caught it); 18 real-`watane` dev tests cannot run in the cloud (no IPv6 loopback).
- (2026-11-20, S90) parallel task worktrees pass alone and conflict when merged (app/NOTES.md, TECHNICAL edits): each runner should append to NOTES under its own heading; mutants on the whole suite stop at the baseline when a window test fails under load (fell back to `--lib`); Sol's sandbox never sees GUI regressions, only check.sh's slow j2 run does.
- (2026-11-20, S89) four adversary rounds from failure texts that each made a nearby line false; the cloud runs as root with dash, so two install tests always fail there and macOS sh is unseen; `/dev/full` is Linux-only.
- (2026-11-20, PO) main check at `5899450`: three Linux-only or load-only failures. log `s91_2_ac2_socket_key_is_not_plain` failed on every macOS run (a socket open is EOPNOTSUPP there, ENXIO on Linux; PO fix `b0c4357`, a cloud slice cannot see it); node `causes::ac2_a_bad_copy_on_the_server_is_not_read_as_not_followed` got `Ok` once under load (a good package, likely the anchor answering before its net turned off: a test race, not a verification gap; 3/3 alone); app `manage::ac2_s47` once (2/2 alone). S91's `mkfifoat` was Linux-only too (PO fix `5899450`). Cloud slices touching OS calls need a Mac run before merge.
- (2026-11-20, S91) the cloud container lacked clippy, nightly rustfmt, cargo-deny and cargo-hakari (installed by hand) and has no IPv6, so host's server tests need a local shim there; the app cannot build there (GTK); one IPv6 test's changed expectation surfaced only on the Mac.
- (2026-11-20, PO) main check at `c224dd3` (S92) under load ~30 (S90 building): `j2::ac7` failed again (S90 reworks it) and net `regress_s75::ac1_ac2_light_reaches_the_full_machine_with_the_anchor_closed` once (*core*); the latter passed 5/5 alone at load 33. app's integration run took 206 s against 70 s earlier: Suite time figures need a quiet Mac.
- (2026-11-20, S92) the cloud toolchain lacked clippy (`rustup component add clippy`); parallel tasks sharing an interface each passed alone and broke only merged (check slice/<id> after each fast-forward); three cold builds at once made each Sol run 15 to 20 min.
- (2026-11-20, PO) main check at `501c123` under load ~40: host `program::catalog::ac1_installed_and_persisted_ids_match_pack_for_both_parts` got `Busy(<tmp home>)` once (*core*: a home lock); passes alone. Watch for a repeat.
- (2026-11-20, PO) main's check after S88: `j2::ac7_idle_polling…` saw fewer than 2 polls in 5 s once in the full suite (load ~20 with S90 building); 3/3 alone. S90 owns the poll code. deny: RUSTSEC-2026-0587 (wasmtime-wasi 48.0.9, Windows-only), fixed by 48.0.10 on main.
- (2026-11-20, S88) Sol's sandbox cannot start the GUI app, so the foreman ran every window test and trace itself; Sol's heavy instrumentation hid the race (60/60 with it); three fixes were wrong before traces showed the cause; a 50-run full driver suite takes about an hour.
- (2026-11-20, S55) the Mac slept overnight and cut off a validator mid-run (long steps now under `caffeinate -i`); a widened diagnostic wait plus a build under load looked like a hang to the PO; local tauri-cli 2.12.5 (CI pins 2.12.4) rewrites app/Cargo.toml's tauri features on `cargo tauri build`; the auto-mode classifier refused a `push -f` of a branch not yet on the remote.
- (2026-11-19, S87) iroh-blobs copies blobs up to 16 KiB into its store instead of referencing the file: a test of a changed referenced file needs a bigger blob. No full run had covered the driver window tests since the test-speed merges.
- (2026-11-19, S50c) `workflow_dispatch` cannot dry-run a workflow not yet on main (ran as a push to a throwaway branch); Homebrew needs `brew trust` per formula for third-party taps; the macos-14 runner was retired under the slice; main's tests/it move mid-slice cost a layout commit.
- (2026-11-19, S50b-fix2) a slice id that equals an old task id collides with its brief (`factory/tasks/s50b-fix2.md`); the foreman used `s50b-fix2-1`. Pick slice ids that no task brief uses.
- (2026-11-19, S50b-fix) this Mac's endpoint security (ManageEngine, Defender) slows each exec 50 to 400 times under load: a full install test run takes 20 to 40 min, and timing races show only here.
- (2026-11-19, S50b-fix) a peer session killed the foreman's check.sh test binary (now a rule); `impl.sh &` from a shell makes INT ignored in the tests; a reused task id overwrote a finished brief for a while (new ids use a dash).
- (2026-11-18, PO) main's check failed ~15 dev tests (`env.allowlist`, paths into `watane-test-speed`): a worktree shares `target/` and `target/tmp`, and dev/tests/support/mod.rs's unlocked `remove_dir_all` of its warm cache lets two concurrent runs delete each other's. They pass alone. The human's chore/test-speed branch adds locks; recheck after it lands.
- (2026-11-18, PO) S86's `install::fake::ac3_never_answer_is_no_answer` asserts under 10 s and failed once in the full parallel suite; 3 s alone. A candidate for the timing `#[ignore]`.
- (2026-11-18, PO) the main check's `dev publish ac4_accepted_fake_publish_completes_under_five_seconds` (5.9 s) and `node program s10` failed under load, passed alone.
- (S85) macOS window tests miss the app's 30 s ready timeout when the Mac is loaded (load 11 to 24 from parallel jobs); compare slice and main only when both run at once. Options: a longer driver TIMEOUT, or one GUI suite at a time.
- (S85) factory/check.sh's app clippy failed on sibling log/pkg code under clippy 1.99 (check again after e8265e8).

- S60: a test that reads `/proc/self/fd` passed in the cloud and panicked on the Mac. The cloud is
  Linux and the human's machine is macOS: a test that leans on a Linux-only path passes where it is
  written and fails where it is used. Briefs for cloud foremen should say so.
- S60: the cloud's shared target reached 28 GB and filled the disk during user-sim (S56 too). The
  foreman built with `CARGO_PROFILE_DEV_DEBUG=0` after a clean. Worth putting in every cloud brief.
- S60: `cargo test` stops at the first failing binary without `--no-fail-fast`, hiding later ones.
- S59: the product owner's brief fenced S59 out of net/TECHNICAL.md except Q2 and Q3, to keep it off
  S60's lines. So four net gaps it found went into node's documents as questions instead of being
  fixed in net. Fence by section, but let a design slice add a question row to the subsystem it
  found the gap in.
- S56, S57: both delivered by 11:30 UTC and sat unmerged for about eight hours. The product owner
  started `factory/watch-cloud.sh` with `s56` and `s57`, and its `--grep` was case-sensitive against
  `DELIVERED S56`. Fixed (`-i`) in `3a39a6f`.
- S56: three parallel builds on hard-linked target copies plus leaked test temp dirs (about 600 MB
  each in /tmp) filled the cloud's 20 GB disk; two implementers and a validator hit "No space left
  on device". S57 needed a 3.9 GB `cp -R` of target per worktree. Cargo also reused a stale binary
  when the target was shared with another worktree (mtime fingerprints), costing a false bisect.
- S56: `api::tests::ac5_search_timeout_is_thirty_seconds` (0.5 s margin) fails in the cloud but not
  on the Mac. A brief for a cloud foreman should say this one is the Mac's to judge.
- S57: three items in one pkg brief, and the brief did not say which side (read or write) had the
  gap, so the swap ACs were hard to make fail first.
- S58: a bare `target/debug/watane` with no arguments serves the real home instead of printing
  usage. user-sim ran one to see the usage text and created `~/.watane` (home.json, an empty lock,
  mods/, staging/) on the human's machine at 12:40 on 2026-11-14; it is left in place, `rm -r
  ~/.watane` if unused. A program that silently takes over a person's real home when an agent runs
  it to read its usage is a trap worth closing.
- S58: Sol's Codex sandbox cannot start the GUI, so every driver test (J2, J4, find) had to be run
  by the foreman and the validator outside it. Third time (S43, S47): a brief whose outcome is a
  driver or journey test must name who runs it.
- S43: Sol cannot run the GUI journeys or `ac6_build_with_network_denied` in Codex's sandbox, so
  S43-2 reached the validator unrun. Same as S47's. A brief whose outcome is a journey test should
  say who runs it.
- S43: leftover `pgrep` polling loops in the tests matched their own command line.
- `dev --test publish ac1_publish_raced_version_and_no_answer_keep_canonical_retry_paths` fails on a
  loaded machine: it runs the client with `--wait 1` against a server with a 10 s `recv_timeout`,
  in a target where another test runs over 60 s. Alone it takes 5.6 s and passes; the whole target
  with `--test-threads=1` is green. Second load-sensitive test after S49's fixture race. Raising
  the test's own budget would fix it.

- S12a: giving back a withdrawn grant asks no consent (app TECHNICAL 7); app FUNCTIONAL S9 changed to match.
- S12a: the app fixtures ship a committed `logic.wasm` (`tests/fixtures/build.sh`); no app test builds a mod.
- S12a: the app CLI in stage B: `--home` defaults to `default_home()`, `--port` to `WATANE_PORT`, then `DEFAULT_PORT`.
- S12a: `Source` has only `Folder`; `Release` comes with stage C. `install_folders` stays for previews and dev (host Q24).
- The human (2026-11-11): foremen may push slice branches to `origin`; the product owner deletes
  the remote branch after the merge.
- S17: dev finds the vendor folder last at `<cache>/vendor` (checksummed like any other); the
  `env.allowlist` fix is `'<dev>/scripts/vendor.sh' '<folder>'`, which works from a sandbox.
- S17: text output flattens nested results to dotted keys; `test.contract` reports the failing
  step's line; `preview.previous` names the page URL (the API only for a mod with no page).
- S17: MCP read-only hint: option (a) built; the template documents Claude's `--allowedTools` and
  Codex's `default_tools_approval_mode = "approve"`, and that the token changes per preview.
- S18: stage F runs on arm64 Linux in Docker under Xvfb with Openbox (focus needs a window
  manager); WebKitGTK resolves `<local>.localhost`; `webkit2gtk` is a Linux driver-only dependency.
- S18: `tools_list` returns `{entry, tool}`; the app shows the host's MCP tool name (APP-F-25).
- S20: the name/summary rule (pkg 4.2) lives once in `meta::text_violation`, shared by `meta::read`
  and `Release::decode`; release tests build `logic.wasm` in a tempdir (no prebuilt fixture).
- S19: `status()` read-checks every stored entry (log TECHNICAL 12); append and ingest check only
  the fork row inside the write transaction (a full scan made append O(n²)).
- S19: `append` may extend a chain whose lower entries are corrupt on disk (log 5.1). Deputy: dist
  calls `status()` before it publishes and refuses with its cause; log stays as specified.
- S22: pkg opens each path part below the mod folder with no-follow (`rustix` openat, already in
  the lock file; the direct `libc` dependency is dropped; no `unsafe`); the mod folder itself may be
  a link. `world.rs` parses WIT from the checked bytes, not `Resolve::push_dir`. The fixtures
  `reading-list` and `recommendations` commit their `logic.wasm`; `hello` lost `ui/app.js`, and
  pkg 6.5 names the on-disk fixture.
- S12b (host Q25): a page gets only its own mod's events; subscriber pages no longer get the
  provider's raw `event`, only `changed` when their state changes (host 8.4, 9.2, HOST-F-13).
  Narrows what a page sees; undo: restore one send.
- S12b: dev's `fake-host` is a default feature (a distribution build uses `--no-default-features`);
  `install` checks `/api/status` before the test pipeline; `host.declined` has `next` null;
  `add <provider>` resolves from the current folder.
- S21a: `<home>/dist/site/` is 0755 with 0644 files; other dist folders 0700. The managed
  `site_dir()` removes a withdrawn own release's package; `export` elsewhere deletes nothing.
  `confirm_own` also fetches and verifies the own live packages. dist calls `log.status(own)`
  before it signs. (dist TECHNICAL 12, 13)
- S12a: host shutdown waits for every server-side holder of the host, at most 5 s (host 12.2 unchanged).
- S1: each foreman turn's closing text reached the product owner as a progress update. Fixed by
  the human's rule (1eed839): foremen report only when blocked or delivered.
- S2: a brief left out the subsystem's error-code module (`dev/src/outcome.rs`); Sol stopped
  halfway. Briefs should always allow it.
- S2: Sol's sandbox refuses nested `sandbox-exec`, so DEV-S-13 runs only in the validator.
- S2, S3: host program tests with 2 s and 10 s limits fail now and then under heavy load (2x).
- S3: the brief did not say S2's `/events`-404 tests would change; Sol left them red.
- S3: nested `sandbox-exec` still runs only in the validator (2x).
- S4: the foreman's process died mid-validation (the battery ran out); `watch.sh` was not
  running, so nobody woke. Not process friction.
- S7: the foreman tried `git push`; the repository has no remote.
- S7: there is no command that only builds `logic.wasm`; and Homebrew's `cargo` comes before
  rustup's on this machine's PATH, without `wasm32-wasip2`.
- S4: the foreman died after acceptance, in its last test run (a forced OS update). Not process
  friction.
- S4: contract `test.contract` errors carry `line: null`; `emits` mismatches show the full event
  name in `got` and the short one in `expected`; one template bug fails a unit and a contract test.
- S5: Sol's sandbox cannot run dev's ignored tests; a task that needs another's host ends PARTIAL
  and the validator runs the real tests.
- S5: a fresh worktree has no `dev/allowlist/vendor`; 4 builder tests fail until
  `scripts/vendor.sh` runs (into S6 `doctor`).
- S5: dev's `ac10` unit-limit test (7 s) fails under parallel load; it gets the timing ignore (S8).
- S5: a refused `serve` leaves the old preview up without saying so (into S6); a trap's text is a
  long wasm backtrace with the panic message at the end; dev keeps the preview token in
  `<cache>/previews/*/ready.json` (0600, dev Q6), which reads awkwardly beside host 10.2.
- S2, S3, S8: nested `sandbox-exec` (DEV-S-13) runs only in the validator (3x). The human
  accepted it (factory/README.md); no longer friction.
- S8: S8-1's Sol run took 77 min, much of it full-suite reruns under S5's load.
- S8: dev `ac10` (timing-ignored now) failed twice on `main` under S6's load: 8.4 s against 7 s,
  the time includes rebuilding the changed source. Rerun when quiet; if it fails then, the timer
  should start after the build.
- S8 user-sim: when only the unit-test build reads outside, `result.build` still names the removed
  `logic.wasm`; `build.reads_outside` has `line: null`; `serve --help`/`new --help` show no options.
- S9: two parallel tasks both took spec gap id Q21; the foreman should hand out Q ids.
- S9: Sol stopped PARTIAL on an old S1-cut test; a brief that changes a cut should name the tests
  it supersedes.
- S9 user-sim: a huge declared `Content-Length` with no body waits instead of 413 at once;
  duplicate keys in a `/call` body are accepted (last wins); `mode_mismatch` does not name the
  asked mode; `GET /api/mods` is 404 (S1 cut); HTTP-layer 400s lack the 8.3 headers.
- S6: the full dev run with `--include-ignored` takes about 12 min, over one 10-min shell limit.
- S6: parallel tasks all append to dev/AGENTS.md "Spec gaps", so each rebase conflicts there
  (trivially).
- S6 user-sim: `preview.previous` names the API URL, not the page; a missing vendor folder reads
  as "differs from this watane-dev"; `mod.invalid` WIT errors have `line: null`; doctor's text mode
  prints raw JSON; a raw `/call` without `args` is `bad_request`; headless Chrome hung on the
  page's open `/events`, so the form was not driven.
- S10: Sol's sandbox has no GUI session; app window tests run only in the validator, which found
  the real bugs (ATS, consent reopen). App briefs must say so.
- S10: window tests get focus only while the terminal is in front (macOS 27); unattended runs can
  time out on app S3 to S5.
- S10: Tauri embeds `Info.plist` without a rebuild trigger; a `cargo clean -p watane-app` dropped
  2.5 GiB of the shared `target/`.
- S7, S14: the foreman tried `git push`; there is no remote (2x).
- S14: app `support_selftest` "port refused after drop" checks fail now and then under parallel
  load (pre-existing; a fix task for a later app slice).
- S14: "user-sim runs no suite" did not fit a slice whose story is the suite; the foreman made a
  one-off exception. Minor: `App::start`/`background`/`front` lack doc comments;
  `ac3_consent_takes_focus` passes a misleading `("WATANE_DRIVER_SCREEN","hidden")` to `App::front`.
- S9, S15: a brief kept "existing tests stay green" while it changed what an old test asserts;
  Sol stopped or needed a resume (2x).
- S15: watane-dev has no `pack` command, so pack is reached only through the host or pkg tests.
- S16: codex-cli 0.163 has no `--full-auto` (use `-s workspace-write`); `codex exec` needs
  `--skip-git-repo-check` outside a git repo; S6 kept no Claude command log to compare with.
- S11: a spec fixture name (`list`) was never checked against the slug grammar; S11-2's first run
  stopped BLOCKED on it.
- S11: Sol twice added an unlisted product change to make its tests easy (a host global call
  lock; a dev "built-only" `test` that would let `serve` install an unbuilt `logic.wasm`).
  Validators caught both.
- S11: `--include-ignored` now also runs main's intentionally red S16 test, so a foreman's final
  run needs a `--skip`.
- S11: dev's full slow run took about 30 min (serve.rs 11, unit_tests.rs 10): S13's input.
- S13: a fresh worktree has no `dev/allowlist/vendor`; tests fail with `env.allowlist` until
  `scripts/vendor.sh` runs. Foremen should run it first.
- S13: the 2-min default budget rests on two binaries (contract, unit_tests) that each pay one
  cold build of about 35 s; little headroom as tests are added.
- S13: some `slow:` reasons say why a test is slow but not what it needs.
- S12a: Sol cannot run window tests, so app tasks end PARTIAL and the validator finds the GUI
  failures (S10, S12a: 2x).
- S12a: the foreman pushed `slice/s12a` to origin (once the remote existed); now told: slice
  branches stay local. Earlier push attempts: S7, S14 (3x in all; process change to propose).
- S11, S17: Sol made a change outside its brief's file list and called it requested (host global
  lock, dev built-only `test`, two factory docs); validators caught each (3x; process change to propose).
- S17: the first default dev `cargo test` after a merge took 3:22 (a cold template build of 152 s);
  warm 81 to 100 s. The 2-min budget has little headroom (S13 too, 2x).
- S6, S17: parallel tasks in one subsystem conflict on `AGENTS.md` "Spec gaps" lines (2x).
- S18: app j2 `ac8_remove_defaults_to_keep_data_and_unknown_keys_fail` failed 1 in 9 runs under
  load ("DOM timed out"); watch it on main.
- S18: the foreman ignored the "you may push" correction because a peer relayed it; fine, pushing is optional.
- S12b, S19, S20 (2026-11-12 night): load reached about 140 with three slices building at once
  next to the Mac's endpoint scanners (Defender, ManageEngine); one Sol run sat 8 h without
  progress and validators were cut off. The laptop slept and clocked down on low battery (external); the human: three slices are fine.
- S19: a validator died 3 times on "computer went to sleep"; a fresh validator under `caffeinate -i` worked.
- S22: the rm guard hook refuses deletes under `$CLAUDE_JOB_DIR/tmp` and in the worktree, so
  user-sim probe crates stay behind.
- S12b: a Codex run hung 6.5 h after a test command at load 140 to 170; Sol spent about 7 h in
  dev's `--ignored` set. Briefs should say: Sol runs the default tests only (README, "Test pyramid").
- S12b: Sol edited files outside its brief again (host FUNCTIONAL, dev `journey_s11.rs`, dev
  `AGENTS.md`); needed consequences this time, accepted (4x; proposal pending).
- S6, S17, S12b: rebase conflicts on dev `AGENTS.md` "Spec gaps" lines between parallel tasks (3x).
- S9, S15, S21a: a brief changed a behaviour an earlier task's test asserts without naming that
  test; Sol stopped with a question (3x). Proposal pending below.
- S12a: a test flaked 1 in 16 runs after the rebase: a server task kept `Arc<Host>` after
  shutdown. Fixed with a bounded wait (S12a-fix1).
- S23: a stale slow-test expectation surfaced only at delivery, because Sol runs the default tests
  only; one extra fix task. Same root as "briefs not naming changed tests" (4x).
- S21b: Sol cannot run window tests, so an app task's main oracle (J3) first runs in the validator;
  window-test flakes showed only in the foreman's serial full run, after acceptance (one fix task).
- S21c: `scripts/vendor.sh` is at `dev/scripts/vendor.sh` (hand-out prompts named the wrong path).
  The user-sim died once on an API ENOTFOUND; resuming it worked.
- S21b, S25: Sol's sandbox cannot run the macOS window tests (S25 ran them on Linux in Docker); the
  validator is the first to run them on macOS (2x).
- PO: three PLAN commits (c51f210, 68124ae, 07b82e4) used `git commit -a` and swept in another
  session's uncommitted skill and `impl.sh` edits. PLAN commits now add `factory/PLAN.md` by name.
- S24: Sol's sandbox refuses nested `sandbox-exec`, so dev's slow `ac6_build_with_network_denied`
  always fails for Sol; Sol also ran the driver GUI tests, which time out in its sandbox. The
  foreman force-pushed `slice/s24` with a lease after a rebase, since the branch was on origin.
- S24: the S24 foreman did not run app's driver tests; the PO ran `--features driver --test api`
  after the merge (green).
- S27: `--features driver --include-ignored` also runs the `front:` tests, so a background foreman
  skips them by name and the PO runs them after the merge. One api.rs run failed on ENOBUFS (passed
  on rerun).
- PO, after S27: the `front:` tests fail on a locked screen (`gui_isolation` ac1: the hostile page
  never gets focus; same on main before S27). `App::front` waits for an idle human but not for an
  unlocked screen; it should wait for, or skip with a clear message on, `CGSSessionScreenIsLocked`.
  The front tests of main `292cd7f` still need one run on an unlocked screen.
- S28: step-3 HTTP tests flake under load on the 300 ms `entry_timeout` of `fast_options`; give only
  the timeout tests a short timeout. A rebased pushed branch needs a force-push, which auto mode
  denies (S24 forced with a lease, S28 was denied): foremen push only after the delivery rebase.
- S28: running dependent tasks ahead of their dependency's validation, each in its own worktree,
  saved about one validator round per task; the rebases were clean.
- S30: dev's `doctor::ac6_real_preview_home_is_running_without_app` panics without `WATANE_BIN`; the
  foreman skill's `--include-ignored` step should say to set it. `ac10_endless_test` took 7.85 s
  against its 7 s bound under parallel builds.
- S31: Sol reports PARTIAL when Codex's sandbox cannot run `sandbox-exec`
  (`ac6_build_with_network_denied`); the validator runs it. One Sol run took about 33 minutes,
  mostly slow builder tests.
- S29: Sol's sandbox cannot start the macOS driver (XPC): app window tests run only in Docker until
  the validator runs them natively. dist `restore.rs` ac11 flakes under parallel load (line 78).
- S34: adding a field to a pub host struct forces one-line edits in app and dev, which "host only"
  slices forbid; constructors in tests would avoid it. dev's suite fails in a worktree without
  `vendor.sh`, which can hide regressions.
- S35: dist network tests time out under parallel load on `fast_options` (300 ms): also S28, and S29's
  `restore.rs` ac11. Third time: a slice gives only the timeout tests a short timeout (S38).
- S37: WindowServer crashed 2026-11-13 04:37, after about 40 h of window-polling loops that S14's
  user-sim had left in `/tmp/s14sim` (Nov 11). The background `claude` daemon (pid 27153) stayed
  on the dead GUI session, so the foremen's window tests could not launch the app; S37 blamed the
  sandbox. Fix: restart the daemon after a GUI re-login. The human approved a rule: a user-sim stops
  every process it started before it reports (`.claude/agents/user-sim.md` step 5).
- S36: the validator's guide fix broke a second limit that only the full lib run catches (two
  limits for one fact; now one: TECHNICAL 4.1's 150 lines). The foreman's push failed on DNS.
- S37: Sol cannot start the macOS driver, so every window and journey test first runs in the
  validator; all three S37-3 window tests and J5 had test bugs found only then. A transient API 403
  ended a turn.
- S39: two Sol runs in parallel on one subsystem loaded the machine (both PARTIAL on the 7 s
  timing and nested-sandbox tests); parallel tasks conflicted at the end of tests/envelope.rs.
- S46: the sketch repeated S22 follow-ups that S22 had partly fixed; the foreman probed first.
- S40: the product owner rebased slice/s40 locally, so its remote diverged and only a force-push
  could update it. Rebase a foreman's branch only after it has stopped, or ask the foreman.
- S45: the cloud has no IPv6 loopback (67 host tests cannot run) and no GTK (app cannot build), so
  a host or dist slice in the cloud is checked on the Mac at merge.
- S41: a cloud worktree lacks the gitignored `dev/allowlist/vendor`; copy it with `target`. No Sol
  ledger in the cloud.
- S39/S44: the product owner merged S39 after dev's tests only; J5 broke unseen until S44. A merge
  that touches `new --from`, `add --release` or publish runs J3 and J5 too.
- S47: S44 changed what the app shows and never ran J5, so the breakage surfaced a slice later.
  A cloud slice that touches the app gets J5 run on the Mac before merge.
- S47: the task brief said "Changed tests: none" while the S11 test asserted the very deletion the
  slice removes; that cost a resume. A brief that reverses a rule names the test that holds it.
- S42: Codex's sandbox cannot run the macOS GUI journey (hiservices XPC) or
  `ac6_build_with_network_denied` (sandbox-exec), so its app task came back PARTIAL and the
  validator ran both outside Codex.
- S49: 9 of the 13 outcomes the product owner sketched were already delivered (S23, S45, S46, S48),
  and the dist task fell away entirely. A sketch built from `Left for later` notes cites the slice
  that has since fixed each note, so the foreman can check it against main instead of trusting it.
- S49 (merge): `cargo test -p watane-pkg -- --include-ignored` fails about as often as it passes.
  `support/release.rs` carries an `#[ignore]` test that rewrites the committed fixtures in place, and
  it is compiled into six test binaries, so it races the tests reading those fixtures. Run pkg's
  ignored tests with `--test-threads=1`, or move the regenerator out of the test binaries.
- S61 (2026-11-15): the Mac's disk filled (161 MiB free) and stopped S61's linker. `target/` had
  grown to 75 GB: cargo never removes old artifacts, and one fresh build is 7 GB. Since then dev
  builds keep line tables only (2e17a94), and the merge routine removes the slice worktree's own
  `target/` after each merge and runs `cargo clean` on main's from time to time (agreed with the
  human). A slice worktree starts from an APFS clone of main's `target/` (`cp -c -R`), not a build
  of its own.
- S61: a resumed local foreman that rebased could not force-push, so it pushed
  `slice/s61-rebased`. The product owner merges the local branch and deletes both remote names.
- Main was red from 51e0dfb (step 9 spec) to S65: dev's `ac5_scenario_map_covers_every_scenario_and_names_existing_tests`
  wants `contract::ac_collect_fake_and_answer`, which step 9 builds. Every verification shows it
  until then; it is not a slice's failure.
- S64: no cargo-hakari in the cloud image. The product owner runs `cargo hakari generate`,
  `manage-deps` and `verify` on the Mac when a cloud slice adds a crate or dependency.
- S63: the brief said both "add no dependency" and "do not touch Cargo.lock"; naming gossip's Stream
  needed one line in each. A brief that forbids the lock says what to do when the lock must change.
- S63: net's default test run takes about 113 s of the 120 s limit (pairing settles about 1 s each).
- S67: telling "the server does not have this package" from "its copy failed its check" needs a
  wire answer; both read the same today.
- S65: the cloud has no IPv6 loopback and host's server binds 127.0.0.1 and ::1, so about 90 host and
  dev server tests run only on the Mac. A v4-only bind option for tests would let the cloud run them.
- S65: host fixture builds are not byte-reproducible; rebuild only new fixtures.
- S65, S69 (merge): dev's `ac10_endless_test_stops_with_exact_message_within_deadline` fails in the
  full ignored run on the Mac (7 s against its deadline) and passes alone. S71 gives it a deadline
  that measures the stop, not the cold start.
- S53: Codex filters TOKEN env vars, so every credentialed run falls to the foreman.
- S68 (merge): the product owner gave a verification its own `CARGO_TARGET_DIR` beside main's and
  filled the disk again (two local foremen' cloned targets diverge as they rebuild: 40+ GB each).
  Verify one slice at a time in main's `target/`, never a second target; clean a slice worktree's
  `target/` as soon as its foreman delivers.
- S71: Sol wrote an 11 GB `target/` inside the worktree (it could not write main's `target/`, though
  impl.sh lists `$CARGO_TARGET_DIR`; to check), and S74's build replaced `watane-dev` under S71's
  suite (about 70 false reds). A foreman's verification holds only while no other foreman builds.
- S74: the same swap, a third time: another slice's build replaced `watane-app` during S74's user-sim
  and during the PO's verification (j2 and 8 manager tests red, green on a rerun alone). Codex's
  sandbox cannot start macOS window tests (HIServices XPC): Sol reports window ACs PARTIAL and the
  validator runs them all. Proposed to the human: each local slice its own target again.
- S76: tests leak `.tmp*` folders (about 600 MB each); in the cloud they filled the disk mid-run.
  On the Mac, 928 of them held 186 GB of `$TMPDIR` on 2026-11-16: the real cause of the full disks.
  watane-app does not compile in the cloud (no gdk-3.0), so a shared enum's app fixes go unchecked there.
- S73 to S76: main is red on dev's envelope ac5 again: S73's design names
  `contract::ac_manage_fake_and_records`, which S76 builds. Not a slice's failure until then.
- S75: `--skip front:` skips nothing (front tests are marked only by their ignore reason); a
  validator's run took the screen before it stopped. Briefs and commands skip the four by name.
- S81: briefs must not ask for tests that grep spec text (two broke on S83's respec).
- S84: two worktrees building window tests into one shared target ran each other's app (later
  validators used private targets); two API drops stopped agents mid-run; macOS denies screen
  capture, so no real WebKit pixels; the comparison used other sample mods than the boards in some views.
- S82: Sol's sandbox cannot start window tests (HIServices) or the pairing test (netmon), so app tasks
  come back PARTIAL and validators run them natively; Sol left one task uncommitted; validator scratch
  folders in the shared target cannot be removed under the rm guard; rebuilt fixtures give other bytes
  for menu and item-* logic.wasm than the committed ones (predates S82).

## Digest

- S1: a coding agent runs `watane-dev new hello`, `check` and `serve --json`, opens
  `http://hello.localhost:<port>/` and sees the page; an edit shows after `serve` again;
  `serve --stop` ends the preview.
- S2: the template is a counter; Alice clicks Add on the page and the count goes up, stored by the
  host; a runaway export is stopped and the mod keeps answering.
- S3: the agent edits logic and page while Alice has it open; `serve` reloads her pages with the
  new revision and the same count; every open page updates by itself.
- S7: a mod folder packs into one canonical package with a stable id and unpacks to the same
  folder; hostile packages are refused.
- S4: the agent runs `watane-dev test`, gets unit and contract failures with file, step, expected
  and got, fixes the code and passes; unit tests run sandboxed with a time limit; `serve` refuses
  a mod whose tests fail.
- S5: the agent points an MCP client at `serve --json`'s `mcp` URL with its token, lists and
  calls the mod's functions, and Alice's open page updates by itself.
- S8: a mod folder builds to the same `logic.wasm` from any path and cache, and the preview host
  installs it under the same id `pack` gives; a mod that reads a file outside its folder is refused.
- S9: every page, `/call`, `/events`, `/api` and program scenario of step 1 has a test on the
  real `watane`; hostile pages are refused; `watane --home D` runs a participant home.
- S6: Alice's agent runs `doctor`, builds her reading list with `new`, `test`, a contract fix and
  `serve`; her book survives a page change; the agent reads the list over MCP (J1 passes).
- S10: Alice starts `watane-app --home <dir>` and sees an empty launcher with her two mark words;
  mod windows are sealed (109 hostile attacks, 0 allowed); every request goes through the consent view.
- S14: `cargo test --features driver` in `app/` shows no window, no Dock icon and takes no focus;
  the 4 front tests run with `--include-ignored` once the human is idle 30 s.
- S15: a bad file in a mod gives an error that names the real cause (too many files, a link,
  escaped control characters, the unpack error before the cleanup error); reads never follow a
  swapped-in link.
- S16: a Codex agent makes and previews a mod with no human help; `factory/harness-codex.md`
  lists what made it guess or read source.
- S11: in `recommendations` the agent adds `reading-list`, writes a call and a subscription, tests
  them with a fake and a `deliver` step, and `serve` installs the provider first; a book added in
  one page reaches the other through the call and the event.
- S13: a watane maker runs `cargo test` in dev and gets an answer in about 1.5 min, not 9.
- S12a: Alice installs both mods in the app after consent, uses them together, restarts, withdraws
  and gives back a grant, disables or removes a mod, and gives an agent an MCP token (J2 passes).
- S17: a maker's agent, Claude or a sandboxed Codex, goes from an empty folder to a served mod and
  its MCP tools from watane-dev's output and the template `AGENTS.md` alone.
- S18: every app test passes on arm64 Linux (`app/scripts/linux-test.sh`); the toolset view shows
  the MCP tool name an agent calls.
- S20: pkg turns a mod into a canonical release body that dist can publish and the host can
  verify; any changed byte is refused with its cause.
- S19: the log crate keeps Alice's identity and her signed, hash-chained entries, refuses corrupt
  rows and forks with their cause, and restores from a 108-byte passphrase backup.
- S22: pkg refuses every link in a mod folder, in any part of a path, and reads each file through
  the handle it checked (about 400k swap attempts, none read through a link).
- S12b: in a mod folder, the agent runs `watane-dev install --json`; dev tests the mod and its
  providers, the home host installs them after the person approves, and a decline or a missing app
  comes back with guidance (step 2 done).
- S21a: Alice's dist publishes the reading list into a site folder; Bob's dist follows her link and
  fetches, verifies and unpacks the same bytes; a changed file is refused with its cause and URL.
- S23: a maker's coding agent gets live refresh (`watane.on`) from the guide, sees the first build
  building, gets a `next` from `check`, `serve` and a declined install, and `add . --dir x` works.

- S21b: Alice publishes a mod from her app and shares her link; Bob enters it in his app, sees "by
  Alice (fingerprint) · signature verified", approves, and it runs; a changed package or head entry
  is refused with its cause and URL, and nothing is installed (J3 passes in `app/tests/it/j3.rs`).
- S21c: Bob's coding agent runs `watane-dev publish --json`; Bob approves "Sign a release" in his app
  and the agent gets the release, site and link; Alice installs from that link (step 3 done).
- S25: Carol's refresh updates Alice's mod quietly or through consent when it asks for a new grant;
  a withdrawal shows Alice's reason and offers the newest live version; a fork shows as compromised.
- S26: the step-4 design is in the specs: indexes as `list`, `include`, `unlist` statements, local
  offline search, hostile-index defences; S28 builds dist's part.
- S24: Bob's agent learns that a version is already out before it builds, with its release and link;
  each publish or install refusal says what to do next; dev's text output reads cleanly.
- S27: a second app on a busy port says how to pick another; updates, declines, withdrawals and
  offers read plainly, the offer names the new grant, and the mods list shows each version.
- S28: Carol's dist chooses Ivy's index from its link and searches it; hits are verified from the
  publisher's channel and say which index listed them.
- S30: Bob's agent reads true publish and check texts: the rerun reports the existing release and
  link, `--home` survives in `next`, and the unused-provider message names what is unused.
- S31: Bob's coding agent runs `watane-dev search [words] [--publisher] [--interface]` and gets the
  app's search results with a `next` for no index or no hits; `check` names a mod listed as its own
  provider.
- S29: Carol adds Ivy's index in her app, searches "reading", sees Alice's mod verified and listed by
  Ivy, and installs it; Ivy lists, includes and unlists through consent (step 4 done).
- S32: step 5 is designed: origin, provider bases from listings, fit and pick, derived-mod search.
- S33: Bob's agent reads resolved paths in `next`, app errors that name the watane app, and the
  own-provider message for any copy of its own package.
- S34: Eve's host binds her picked provider in place of a missing one, kept across restarts and
  updates; a preview links a provider by slug, whatever its publisher.
- S35: Eve's dist fetches Carol's release from the bases named by live listings, verified against
  Carol's key; search filters by origin.
- S38: dist's suite passes in parallel on a busy machine.
- S58: a person removes a toolset from the app's agents view (host stage 4 done), and reads a name
  before a key at every consent, a party name on a wrong-channel follow, and a cause on a mod card.
- S56: a maker's agent reads the real cause and a runnable `next` at fifteen dev messages that misled.
- S57: `add_provider` cannot be made to write outside the mod by a swapped link, and pkg, log and
  dist name the real cause at six refusals, `StaleBases` among them.
- S50a: installing watane is designed: one curl | sh from GitHub Releases, rustup offered, the
  Gatekeeper step, upgrade and uninstall, with 15 attacks on the design pinned as scenarios.
- S86: a maker's agent reads a line number in a WIT error, passes `--home` to test and serve, and
  gets a rerun it can paste at every fix that used to say "run this command again".
- S50b: the packed install script installs, upgrades, reports and uninstalls watane in a home, and
  refuses every attack found on it before it changes anything (proved on Linux with fakes).
- S50b-fix: on a Mac, the packed install script installs, upgrades, reports and uninstalls a real
  signed archive on macOS's own sh, and a Ctrl-C at any step exits 130 and leaves a whole version.
- S50b-fix2: a Ctrl-C in the install's first checks on a loaded Mac exits 130 with no false cause;
  install's tests no longer lean on Linux-only accidents.
- S50c: on a clean Mac (a Tart VM), the README's one-liner installs watane from a CI-built archive;
  the app opens and `watane-dev new` and `test` work. The release workflow and Q16 are in place;
  v0.1.0 is out, and its one-liner passes on a clean Mac.
- S87: when Alice's full machine holds a changed package file, her laptop's Machines view always
  names that machine and why, whatever order the events come in. wasmtime 48.0.9 (8 advisories).
- S55: Alice publishes the reading list on this Mac; Bob, in a fresh macOS VM with v0.1.0 from the
  one-liner, follows her link over the network, sees her verified, installs and uses it, and a
  tampered package is refused by name (`sh install/tests/vm_two_machines.sh`).
- S88: the remove confirmation's focus check no longer flakes (a WebKit blur after a consent closes, in hidden test windows only).
- S92: when Alice's install fails on a light machine she is told the real cause and whose machine it is.
- S91: a maker or mod that hits a race or an odd file in pkg, log or host gets watane's own named cause.
- S89: Alice gets a true line from install.sh at every step, interrupted or not.
- S90: Alice's manager says what was done or denied, in her words, and hidden windows still refresh.
- S93: a maker's agent gets a real cause and a runnable next at every dev failure we knew of.
- S94: a maker's `cargo test -p watane-install` ends in about 2 minutes on the Mac, guards intact.
- S98: a merge to main no longer needs a macOS rerun; the app quits cleanly under load.
