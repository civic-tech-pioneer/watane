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

Step 1 done 2026-11-11: J1 passes (`dev/tests/journey.rs`); every step-1 stage is done or its rest
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

Step 2 done 2026-11-12: J2 passes (`app/tests/j2.rs`); every step-2 stage is done.

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

Step 3 done 2026-11-12: J3 passes (`app/tests/j3.rs`); every step-3 stage is done.

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
- [~] **S29 Carol finds Alice's mod in her app (J4).** (branch `slice/s29`; foreman: see `Foremen`) app
  stage D's index part as designed (app TECHNICAL 8.2, 9.6, the index and search commands of 7, the
  `list`, `include`, `unlist` details of 6.2; APP-F-38, 43 to 46): Carol adds Ivy's link as an index,
  searches "reading", sees Alice's mod verified and listed by Ivy, installs it after consent; Ivy
  lists a mod in her own index through a signature. Done when `tests/find.rs` and the whole
  `tests/j4.rs` (with index) pass and `static_config` lists the new commands; this closes J4's app
  part. Replace S28's `_ => Refuse` arm in `confirm.rs` with the index statement details.
  - Also (S28 user-sim): choosing the own channel as an index says "cannot follow the own channel";
    `IndexView.skipped` is only a count (say which channel and why); a listing of the own key shows
    nowhere; when only a stale mirror answers, `hit.link` carries the mirror's base; `Via::Index`
    has only the key (use `channel(index)` for "listed by Ivy"); error texts show raw 52-character
    keys; "invalid note: invalid character count" gives no limit.
  - Touches: app; dist only where the notes above need it. Not dev (S30, then S31).
- [~] **S31 The agent searches (dev stage 8).** (branch `slice/s31`; foreman: see `Foremen`)
  `watane-dev search` (dev TECHNICAL 10, stage 8): app status, `POST /api/search`, the result, a
  `next`; `fake-watane` answers `/api/search`; DEV-S-33, 34. Also (S30): pkg gives the own-provider
  case its own message (about one line), and dev drops its own `wit/world.wit` reader for it (about
  45 lines). Touches: dev, pkg. Not app (S29).
- [x] **S27 The app says what happened.** Delivered 2026-11-12 (main `292cd7f`). Sol runs: S27-1 2 (new,
  resume). Left for later (user-sim): a fast retry can leave a stale error (`action()` and
  `button()` both call showError); the busy-port window still shows "Install folders" with no
  working local API; hidden windows poll only on focus; consent requests from clients that stopped
  waiting pile up (S24).

## Stage coverage

One row per stage of each subsystem's build plan (its TECHNICAL "Build plan" table), with the
step that needs it. Status: `open`, `part` (what is missing), `done` (slice). A step is done only
when its journey passes and every stage of that step is `done`, or a deputy decision moves the
rest to a later step. Leftover `ponytail:` comments count as missing parts (`git grep ponytail:`).

| Subsystem | Stage | Step | Status |
|---|---|---|---|
| pkg | 1 names, 2 meta/deps, 3 world/folder/grant | 1 | done (S1) |
| pkg | 4 component | 1 | done (S2) |
| pkg | 5 package (pack, unpack) | 1 | done (S7, S8: the host installs through it) |
| pkg | 6 add_provider | 2 | done (S11) |
| pkg | 7 release | 3 | done (S20) |
| host | 1 runtime core | 1 | done (S2) |
| host | 2 links and events | 1, 2 | done (S3, S11: links, subscribers, hops on the preview `install_folders` path) |
| host | 3 server and pages | 1 | done (S9); the subscription and revoke hostile rows in step 2 |
| host | 4 tools (toolsets, tokens, MCP) | 1 | part (S5): `remove_toolset`, `toolsets`, `tokens` with the app (step 2) |
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
| app | A shell | 2 | done (S10) |
| app | B host | 2 | done (S12a) |
| app | F Linux | 2 | done (S18: arm64 Linux in Docker, `app/scripts/linux-test.sh`; front tests are macOS only) |
| app | C dist | 3 | done (S21b: publish, follow, `DistConfirm`) |
| app | D find | 4 | part (S25: APP-F-37, 39, 40); APP-F-38 waits on the index design (S26) |
| app | E build on | 5 | open |
| log | L1 to L5 | 3 | done (S19: L1 to L4; S21a, S21b: L5) |
| dist | S1 to S5 | 3 | done (S21a) |
| dist | S6 find | 4 | done (S28) |

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
- S29: `4825c26d` (sessionId `4825c26d-3ec3-4617-8564-af8f2286a844`), worktree `.claude/worktrees/s29`.
- S31: `a3423763` (sessionId `a3423763-65b9-4e29-b8f9-9662be460fb7`), worktree `.claude/worktrees/s31`.
- S7: `871c5605` (sessionId `871c5605-a537-44d1-bd37-c2dc3068d11c`), worktree `.claude/worktrees/s7` (stopped, removed).

## Waiting on the human

- S16/S17: should the host mark some MCP tools read-only (`readOnlyHint`), so harnesses like Codex
  call them without an approval setting? Options: (a) no hint; the template `AGENTS.md` documents
  the approval line per harness; (b) a mod declares read-only functions (a new `mod.toml` or WIT
  convention) and the host passes the hint on. Recommendation: (a). The hint is the mod's own
  claim, so a mod could mark a write read-only to skip approval, and (b) adds a format. Cost to
  undo: (a) nothing; (b) a format change in pkg, host and dev. S17 built (a).

## Deputy decisions
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

## Friction

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
  is refused with its cause and URL, and nothing is installed (J3 passes in `app/tests/j3.rs`).
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
