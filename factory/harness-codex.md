# J1 with Codex (S16)

Bob asks Codex for a reading list in an empty folder. Codex has `watane-dev` on its PATH and
nothing else: no repo, no `AGENTS.md` of its own, a fresh `CODEX_HOME` (only `auth.json` and the
model). Logs: `factory/runs/s16-bob/` in the main checkout (`s<n>.jsonl`, prompts `p<n>.txt`).

## Setup

- `codex-cli 0.163.0`, `gpt-6.1-sol`, effort `medium`, 2026-11-11.
- `codex exec --skip-git-repo-check -s workspace-write`, network on, writable: the work folder,
  `WATANE_DEV_CACHE` and `~/.cargo` (as `impl.sh` gives Sol). Session 2 also had `CODEX_HOME`
  writable. Later sessions are `codex exec resume` of the same thread.
- Not `--dangerously-bypass-approvals-and-sandbox`: Claude Code's permission check refused it.
  So this is Codex in its normal sandbox, the setup a careful Bob would use.
- `watane-dev` and `watane` from `target/debug` of `slice/s16`; the worktree had no
  `dev/allowlist/vendor` (a fresh checkout, as a real install without the packager would be).

## Outcomes

1. **Holds.** Session 1 (379 s, 31 commands, 6 failed): `doctor`, `new`, the mod, `test` ok
   (11 contract steps, 0 unit tests), `serve` ok, link given. No human help, but only by reading
   watane-dev's source to get past the vendor gap (below).
2. **Holds only with one human line.** Session 2 (76 s, 7 commands, 0 failed): Bob's change (a
   count at the top) was served, then Codex wrote `[mcp_servers.reading_list]` with `url` and a
   bearer `http_headers` into its own `config.toml` and checked it with a raw JSON-RPC call.
   Session 3: Codex saw the tools and called `reading-list_api_add` and `_get`; both failed:
   `MCP tool call requires approval, but approval policy is never`. Session 4: asked to fix its
   own config, Codex refused ("I can't change my own approval controls"). Session 5, after Bob
   added `default_tools_approval_mode = "approve"`: add and get passed through MCP.
   Probe (session 6): with a proxy that only adds `annotations: {readOnlyHint: true}` to the
   `_get` tool, and no approval line, Codex called `_get` without approval.
3. This file.

## Friction log (Codex)

Commands, in order (session 1 unless marked):

| # | Command | Result | Doc read / guess |
|---|---|---|---|
| 1 | `rg --files` for AGENTS.md, README | exit 1 (empty folder) | |
| 2 | `command -v watane-dev; watane-dev --help` | ok | help lists commands with no descriptions |
| 3 | `new --help; serve --help; doctor --json; test --help` | doctor `env.allowlist` | options have no descriptions |
| 4 | `new reading-list --json` | ok | |
| 5 | `cat AGENTS.md mod.toml wit/... logic/... ui/... tests/...` | ok | template `AGENTS.md`, `host.wit` |
| 6 | `cat dev/scripts/vendor.sh; rg WATANE\|allowlist\|vendor dev/src` | ok | **source**: the fix script writes into the watane checkout, outside its sandbox |
| 7 | `cargo vendor ... .watane-tools/vendor` (copied allowlist) | ok | **guess**: vendor into its own folder; `WATANE_DEV_VENDOR` found in source, not in the error |
| 8 | write wit, logic, page, tests | ok | |
| 9 | `sed dev/src/preview.rs`, `builder.rs`, `cli.rs`, `unit.rs` | one exit 2 (guessed file names) | **source**: how serve and test run, how long a build takes |
| 10 | `npm install playwright` | EPERM (`~/.npm` outside sandbox) | wants a browser to try the page |
| 11 | `npm install --cache .watane-tools/npm-cache playwright` | ok | |
| 12 | `ps` | not permitted (sandbox) | wanted to see if a build was running |
| 13 | Playwright with system Chrome | browser closed at launch (sandbox) | |
| 14 | `npm install linkedom`, then a pinned older one | ESM error, then ok | builds its own DOM page check |
| 15 | `doctor`, `test --json` with `WATANE_DEV_VENDOR`, `WATANE_DEV_CACHE` set | ok (build 71 s) | **guess**: also moved the cache into the work folder |
| 16 | `cat host/src/watane.js`; `rg ... host/src` | ok | **source**: to fake `watane.call` in its DOM check |
| 17 | `serve --json` | ok, `next: null` | |
| 18 | `curl` page; node `fetch` with `Host` header | 404 `unknown API route` | **guess**: Host header on 127.0.0.1 |
| 19 | its page check, after one fix | `PASS` | |
| s2 | `cat ready.json`; `sed host/src/tools.rs`; web search Codex MCP docs | ok | token from `<cache>/previews/*/ready.json` (dev Q6), tool names from source |
| s2 | append `[mcp_servers.reading_list]`; `codex mcp list` | ok | Codex docs (web) for `http_headers` |
| s3, s4 | MCP `tools/call` | approval required | no watane doc says how a harness approves tools |

Sum: 38 commands, 6 failed; about 9 reads of watane source (`dev/src`, `dev/scripts`, `host/src`);
3 guesses (vendor folder, cache folder, Host header). Tokens: 1.33 M input (1.26 M cached),
9.3 k output for session 1; 2.64 M input total over all six.

## Side by side with Claude (J1, S6)

S6 has no command log for Claude, only the scripted J1 test (`dev/tests/journey.rs`) and the
user-sim's notes (PLAN `Friction`, "S6 user-sim"). So this compares findings, not counts; S17 can
count both the same way from these logs.

| Step | Claude (S6 user-sim) | Codex (S16) |
|---|---|---|
| vendor gap | message reads as "differs from this watane-dev"; no sandbox, so the fix script can run | same message; could not run the script (sandbox), read source, found `WATANE_DEV_VENDOR` |
| docs | template `AGENTS.md`, `--json` results | the same, plus 9 source reads; `--help` gave no descriptions |
| try the page | headless Chrome hung on `/events` | no browser in the sandbox; built a DOM check with npm packages |
| serve result | `preview.previous` names the API URL, not the page | `next: null` after a good serve (S11 adds it) |
| MCP | the scripted test calls `tools/call` with the token | adds the endpoint to `config.toml` itself; calls blocked until a human approves |
| errors seen | `mod.invalid` with `line: null`; doctor text prints raw JSON; `/call` without `args` is `bad_request` | none from watane-dev; its own errors were sandbox and npm |

## Red tests

- dev `tests/doctor.rs` `regress_s16_missing_vendor_says_missing`: a missing vendor folder says
  "differs from this watane-dev: vendor folder is missing"; TECHNICAL 2.2 says
  "the crate allowlist at <path> is missing".

## Lines for S17

- A sandboxed agent cannot run the vendor fix: it writes into the watane checkout. The fix
  now works from the agent's writable cache: `vendor.sh '<cache>/vendor'`; dev finds it
  without an export. An explicit override remains authoritative. S17-1 also describes all
  help inputs, flattens doctor text, points environment gaps to fixes, preserves available
  WIT and contract locations, and names the page in previous-preview warnings.
- `watane-dev --help` and each command's help have no descriptions; the agent read source instead.
- The preview's MCP tools carry no `annotations`; Codex in `exec` blocks every call that is not
  `readOnlyHint`. A host decision: mark functions that write no state, or document the
  harness's approval line (`default_tools_approval_mode = "approve"`) in the template `AGENTS.md`.
- The template `AGENTS.md` does not say how to add the preview's MCP to an agent's config
  (url, bearer header, the token changes with each preview); Codex searched its own docs.
- The template does not say how to try the page without a browser; Codex installed npm
  packages and read `host/src/watane.js` to fake the page runtime.
- `serve` succeeds with `next: null` (S11 covers it).
