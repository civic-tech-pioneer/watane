# J1 under Codex and Claude after S17

Bob asks his coding agent for a reading list in an empty folder. The agent has `watane-dev` and
`watane` on its PATH (symlinks to `target/debug` of `slice/s17`) and nothing else from watane.
Same prompts as S16 (`factory/runs/s16-bob/p1..p3.txt`; for Claude, p2 says "Claude Code config"
instead of "Codex config"). Logs in the main checkout: `factory/runs/s17-bob-codex/` and
`factory/runs/s17-bob-claude/` (`s<n>.jsonl`, `s<n>.time`, prompts), untracked.

## Setup

- Fresh install: `dev/allowlist/vendor` of the worktree moved aside for the runs (restored
  afterwards); each harness has its own empty `WATANE_DEV_CACHE`.
- Codex: `codex-cli 0.163.0`, `gpt-6.1-sol`, effort `medium`; `codex exec --skip-git-repo-check
  -s workspace-write` (resumes with `sandbox_mode="workspace-write"`), network on, writable: the
  work folder, the cache, `~/.cargo` and its own fresh `CODEX_HOME` (auth.json and the model only).
- Claude: Claude Code 2.1.322, `claude -p --output-format stream-json`, its own empty
  `CLAUDE_CONFIG_DIR`, `--allowedTools "Bash Edit Write Read Glob Grep"`; session 3 adds
  `mcp__reading-list`, as the guide says. No bypass flag.
- Sessions: 1 build and serve, 2 Bob's change (a count at the top) and MCP setup, 3 add a book
  and list it through MCP.

## Comparison

Commands are shell commands (Codex `command_execution`, Claude `Bash`); file edits are not
counted, as in S16. Failed: a non-zero exit, or an error reply the agent had to work around.
Source reads: reads of the watane checkout or watane's internal files.

| | S16 Codex | S17 Codex | S17 Claude |
|---|---|---|---|
| Commands (all sessions) | 38 | 27 (20 + 7 + 0) | 14 (12 + 2 + 0) |
| Failed commands | 6 | 3 | 2 |
| Source reads of watane | about 9 | 5 (+1 `ready.json`) | 1 (+1 served `watane.js`) |
| Guesses | 3 | 1 | 1 |
| Human lines | 1 (approval), plus a refused session | 1 (approval, asked for by Codex) | 0 |
| MCP calls | blocked until the human line | add and get passed | add passed |
| Wall time, session 1 | 379 s | 259 s | 286 s |
| Wall time, sessions 2 and 3 | 76 s + 3 sessions | 78 s + 20 s | 63 s + 9 s |

S17 Codex, in detail:
- Failed: `rg --files` in the empty folder (exit 1); `ps` (sandbox); `rg` on a guessed
  `dev/src/test.rs`.
- Source reads: `cat dev/scripts/vendor.sh` before running the fix (caution, reasonable);
  `ls dev` and `rg` for `watane.js`; `cat host/src/watane.js` (to learn `watane.on` and the
  `changed` message for its page); `rg` for `dev/src/test.rs`; `rg --files dev/src`; session 2
  read the token from `<cache>/previews/*/ready.json` instead of the `serve --json` it had just run.
- Guess: page checks against `127.0.0.1` with a `Host:` header, not the page URL the guide gives.
- The vendor fix ran from the sandbox into `<cache>/vendor` as given; doctor then passed, no export.
- Session 2 still searched the web for Codex's MCP keys (its own `openai-docs` skill asks it to),
  wrote the config the guide gives, and told Bob to add `default_tools_approval_mode = "approve"`,
  quoting the guide. Bob added that one line; session 3 called `reading-list_api_add` and `_get`.

S17 Claude, in detail:
- Failed: `test --json` with `mod.invalid` (`list` is a WIT keyword; Claude's own mistake,
  fixed with `%list` in one step); a `curl` `/call` without `args` answered
  `bad_request: missing field 'args'`.
- Source reads: `cat dev/scripts/vendor.sh` before running the fix; then `curl .../watane.js`
  from the preview to see how the page sends `args`.
- Guess: omitted `args` in curl, from the guide's page section ("omit args or pass null"); the
  curl section does not say `/call` needs it.
- Session 2: `claude mcp add --scope user --transport http ...` as the guide says, checked with
  `claude mcp list` (connected). Session 3: one MCP call, no prompt, no human line.

## Outcome 1 checks (by hand, same binaries)

- `--help`: every command and option has a line; `serve --help`: `--stop  Stops the preview and
  deletes its data`.
- Missing vendor: `env.allowlist` "the crate allowlist at `<cache>/vendor` is missing", fix
  `'<dev>/scripts/vendor.sh' '<cache>/vendor'`; both agents ran it from their sandbox.
- Doctor text: dotted keys, `allowlist: missing`, no JSON; `next` on a gap: "Run each error's
  fix, then run: watane-dev doctor --json". `test` and `serve` on the gap: `next` doctor.
- `serve` ok: `next` "Give the person this page: <url>". Stop: "The preview and its data are gone".
- Refused serve: `preview.previous` "the preview at http://demo-list.localhost:<port>/ still
  serves the previous build"; the contract failure has `line: 6` (the step's line).
- WIT syntax and keyword errors: `line: null` (pkg gives no location; accepted).
- Data after the preview process is killed and `serve` runs again: kept (port and token change).

## Red tests

- dev `src/template.rs` `regress_s17_guide_curl_call_needs_args`: the guide's "Try the page
  without a browser" section must show `"args":null` for a call without arguments; Claude
  omitted `args` and got `bad_request`.

## Remaining friction

- The guide does not describe `watane.on` and the `changed` message; Codex read
  `host/src/watane.js` to wire live refresh.
- The first `test` builds for about 25 s without a word; Codex ran `ps` and searched `dev/src`
  for timeouts to learn whether a build was still running.
- Codex checked the page with `127.0.0.1` and a `Host:` header (as in S16), not the
  `*.localhost` URL in the guide; it works, but it is a guess.
- Codex took the token from `ready.json` in the cache, not from `serve --json`.
- Both agents read `vendor.sh` before running it: the fix is a script in the watane checkout.
- After a restart the port changes, so Bob's link and the MCP url change too; `serve`'s `next`
  (with `reused: false`) does not say to update the agent's MCP config.
- Text mode: empty lists print `calls: ` with nothing; a list of objects reads
  `pages: local: demo-list, slug: demo-list, url: ...`; an unknown command prints `: failed`.
- `doctor` run inside a mod folder suggests `watane-dev new <slug>`; `check` on `mod.invalid`
  has `next: null` while `test` gives one; `usage.not_a_mod` suggests the same command again.
