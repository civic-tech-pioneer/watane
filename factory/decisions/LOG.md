# Deputy decisions before 2026-11-27

Moved out of `PLAN.md` on 2026-11-27. Each one is past the human's veto window. Each still has to
be written into the spec that owns its fact (an accepted risk under that spec's "Not protected"
or known limits; a rule in its section) and then deleted from here: ISSUE_PLACEHOLDER. Review
leads treat these as precedent, as they did under `Deputy decisions` in `PLAN.md`.

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
