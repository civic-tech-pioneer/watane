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
- [ ] **S55 Two machines.** in flight (local; foreman `4a61b2b4`, sessionId `4a61b2b4-c5d7-4988-a8cf-2bf6cbe171b6`) Alice publishes on one machine, Bob installs on another and uses the
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

After step 8, the plan ends and VISION's own step 6 begins: the factory as a set of mods in the
host. It exists today as `factory/*.sh`, markdown briefs and sessions the product owner starts by
hand — outside the substrate, steering coding agents and changing itself, which is what VISION
describes except for where it runs. Moving it inside makes the thing that built watane the first
serious thing built on watane, and is the sharpest test of the premise: if the substrate cannot host
its own factory, the premise is weaker than it looks. Not sketched; it needs its own design slice.

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
- [S15] a link in a hidden folder (`ui/.git/x`) is rule `path` at the link's own path
  (pkg/TECHNICAL.md:288, pinned by `ac1_s46_hidden_link_named`).
- [S12b] `--wait` does not count the test pipeline; it is the time the participant has to answer
  (dev/TECHNICAL.md:131).
- [S39] a killed run's `<id>.<pid>.tmp` folder is never swept; `store` removes only its own pid's
  (dev/src/build_on.rs:136, asserted at build_on.rs:440).
- [S49] dist does not try the next base when a mirror answers 404 for the package: the package is
  read from the one base that served (dist/TECHNICAL.md:990).
- [S49] by DD-33 the cache-path link can point at a dead base (dist/TECHNICAL.md:1270;
  `ac4_cache_hit_no_record_links_first_base` serves a dead host and asserts the link).
- [S37] `{"release"}` in a home with no record answers 200 from dist's cache
  (app/src/api.rs:321, dist/src/fetch.rs:68). Documented as deliberate; no app test covers the
  link-then-release sequence.

### pkg

- [S57] under a live swap race `add_provider` refuses safely, but with plain I/O text ("wit/deps: File
  exists", "No such file or directory").
- [S57] `pack` has the same list-then-stat window `Mod::read` had (`pack_error` gives `Io`), and
  maps every `InvalidInput` to the FIFO/socket text, so a directory swapped in for a file is misnamed.

### log

- [S57] log/src/identity.rs still does `symlink_metadata` and then `File::open` on the key path: a
  check-then-open, inside a 0700 dir.

### host

- [S12a] `ac4_program_defaults_to_environment_home_and_port` probes port 47341 and then binds it
  (host/tests/it/program.rs:567); the probe-to-bind window stands.
- [S45] the 413 guard passes because the test tolerates a reset, not because the server drains the
  unread body (host/tests/it/program/api.rs:401; the server sets `Connection: close`). Worth knowing before
  trusting it under other guards.
- [S12a] an abrupt disconnect mid-`/call` has no test; `Served::shutdown` still documents a 5 s
  bound (host/src/server.rs:195).
- [S44] an unknown `/api` route answers 404 `bad_request` (host/src/server.rs:107, 245).

### dev

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

- [S12a] a disabled provider shows the fixture page's "not granted" and hides the host's message
  (app/tests/fixtures/recommendations/ui/app.js:5, asserted by app/tests/it/j2.rs:212).
- [S12a] there is no grant-change message, so the app reloads the whole window
  (app/src/core.rs:313).
- [S18] the Linux runner prints versions without names (app/scripts/linux-test.sh:14).
- [S18] about 17 AT-SPI warnings bury the output; `NO_AT_BRIDGE` is set nowhere.
- [S27] the busy-port window shows "Install folders" with no working local API; the failure path
  only disables it (app/ui/index.html:2, app.js:151).
- [S27] hidden windows poll only on focus (app/ui/app.js:164).
- [S44] no busy state during a 30 s hanging follow or add-index (app/ui/app.js:262, 273).
- [S44] `Core::mods()` re-reads every related channel view on each 2 s refresh, with no memoization
  (app/src/core.rs:276, 167, 1490; the poll is app/ui/app.js:156).
- [S58] Remove has no confirmation, and a token bound to a removed toolset gives no hint that its
  toolset is gone (app/ui/app.js:122).
- [S58] installing from an index follows the channel silently, with nothing said about the follow.
- [S58] reinstalling after "Keep data" marks the storage `new`.
- [S58] "Deny" reads "statement refused", which says nothing about what was denied.
- [S58] channel rows still show raw transport text.
- [S58] `/api/fetch` with `[]` or `{"link":5}` surfaces "untagged enum FetchQuery"
  (app/src/api.rs:296; the non-JSON and wrong-shape cases now carry the serde cause, this one
  carries serde's internal wording).

### dist

- [S57] `StaleBases` says "1 base(s)", and a slug that does not exist at all behind a stale mirror
  also reads as stale.
- [S26] Q12: log entries of unlisted channels are never removed. `prune_listed_channels` touches
  only follows.json (dist/src/index.rs:463); log has no `forget` API (log/src/lib.rs:81).
  dist/TECHNICAL.md:1231 still carries Q12 open.

### install

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
  v4 run on forced Node 24 (warning).

### net / node

- From S87's adversary (older than S87): a local I/O or holder error during a download reads
  "unreachable <full machine>: package ... not fetched: <local io error, temp path>", blaming the
  remote machine; net 5.9 is silent on it.
- From S87's user-sim: a failed install tells Alice "channel ... is not followed; follow the
  publisher's link" while the real cause, her full machine's bad copy, shows only in Machines (node
  drops the blob error before dist's no-base fallback; app TECHNICAL notes it).

### Unsettled

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

## Deputy decisions
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
