Deputy: the agentic-harness contract S100 builds against, for the build team (it could not reach that repository). Quoted from civic-tech-pioneer/agentic-harness design/TECHNICAL.md at ba43796. This is the contract for F1.1, F1.2, F1.3, F1.4, F1.9; where host TECHNICAL disagrees, agentic-harness wins for the H2 wire items and the stage B budgets (TECHNICAL 2.1 and 15 there; watane accepted them as N3).

### 4.5 The control protocol (lines 409-474)
```
### 4.5 The control protocol

1. The host writes `initialize`. With `pre-tool-check` it holds
   `hooks.PreToolUse = [{matcher: "*", hookCallbackIds: [<id>], timeout:
   <timeout-s>, onFailure: "block"}]`.
2. With a callback, the answer must hold `hook_on_failure_supported: true`
   and not `hooks_applied: false`; otherwise the host kills the group and
   ends the stream `mismatch`. Once this step has passed, the host pushes the
   item `{"type":"initialized"}` on the `claude-code.start` (and
   `claude-code-shell.start`) stream. The host starts the process only after
   the workspace consent is held (8), so `initialized` never comes before it;
   the checked provider relies on that (10).
3. The `system/init` line must match the launch in tools, MCP servers,
   agents, permission mode (`manual` reads `default`) and plugins (exactly
   `watane-run` and Claude Code's `cc-plugin-*`). Skills are not compared.
   A difference: kill, `mismatch {field}`.
4. Then each stdout line becomes an item:

| Line | Item |
|---|---|
| `control_request` `can_use_tool` | `{"type":"ask","id":<host id>,"tool_use_id":<the request's tool_use_id>,"tool":…,"input":…,"expires-at":<ms>}` |
| `control_request` `hook_callback` (PreToolUse) | `{"type":"check","id":<host id>,"tool_use_id":<the request's tool_use_id>,"tool":…,"input":…,"expires-at":<ms>}` |
| `control_cancel_request` | `{"type":"ask-resolved","id":…,"by":"cancelled"}` |
| an `answer` the host accepted, whoever called it | `{"type":"ask-resolved","id":…,"by":"answered","outcome":"allow"\|"deny"\|"none"}`: `outcome` is the answer the host sent to `claude`, `none` only for a check (step 5) |
| an ask the host denied at its `expires-at`, or a check it has not answered at its `expires-at` | `{"type":"ask-resolved","id":…,"by":"expired"}` (step 5) |
| a line whose top-level `type` is any watane item type that can share the stream: `ask`, `check`, `ask-resolved`, `initialized`, `wrapped`, `consent-pending`, and the checked provider's `verdict` and `checking` (10); a slice that adds an item type to any of these streams adds it here | never passed through: the host pushes `{"type":"wrapped","line":"<the line, as a JSON string>"}` instead |
| a line that is not valid UTF-8, not exactly one JSON object, or that has a duplicate key at the top level | the same `wrapped` item (a byte that is not UTF-8 becomes U+FFFD in `line`; the kernel keeps this `line` as it is; the `\xNN` form of 9.8 is only for bytes that reach the kernel without the host, as in its fixtures): the host decides on the parsed value, so the kernel and the host never read one line two ways |
| any other line, known or not | the line unchanged |

   `expires-at` is on the host's clock: for an ask, the time the host pushed
   the item plus 10 minutes, the same instant at which it denies the ask
   (step 5); for a check, the push time plus the hook's `timeout-s`. A
   `control_request` whose `tool_use_id` is missing, not a string, or empty is
   not passed as an ask or check: the host answers it deny (ask) or `deny` (check) at once and pushes
   nothing for it. So no `claude` stdout line can pass as a host item. The
   host's `id` of an `ask` or `check` item is unique on its stream: it is never
   reused, also after its `ask-resolved`. The host builds each `ask`, `check`
   and `ask-resolved` item from its own parsed value and serializes it
   itself; it never copies bytes of a `claude` line into an item. An
   `expired` or `cancelled` item has no `outcome`.

5. Writes to stdin are only these:

| Operation | Written | Rule |
|---|---|---|
| `say` | a `user` message with the content | refused `slash-command` when the first text block starts with `/` after white space |
| `compact` | a `user` message `/compact` | – |
| `answer` to an ask | `control_response` `{behavior: allow, updatedInput}` or `{behavior: deny, message}` | `none` refused; `allow(none)` sends the original input; a changed input only for `AskUserQuestion` and `ExitPlanMode` |
| `answer` to a check | `{}` for `none`; `{hookSpecificOutput: {hookEventName: PreToolUse, permissionDecision: deny, permissionDecisionReason}}` for `deny` | `allow` refused |
| `interrupt` | `control_request` `interrupt` | – |

   After any answer, and when an ask expires (10 minutes from the push of
   the ask item, at its `expires-at`; deny), the host
   pushes `ask-resolved {id, by}`: `by` is `answered` (with the `outcome` it sent) for an answer,
   `expired` for the expiry and `cancelled` for a `control_cancel_request`
   (step 4). The host pushes no other `by`. A second answer is `unknown-ask`.
   The host closes checks too: at a check's `expires-at` (the push time plus
   `timeout-s`, step 4) it pushes `ask-resolved {by: expired}` for a check it
   has not answered and writes nothing to stdin for it (`claude`'s own hook
   timeout, `onFailure: block`, already blocks the tool); an `answer` to it
   after that is `unknown-ask`. Nothing
   else is ever written: no `apply_flag_settings`, `mcp_set_servers`,
   `set_permission_mode`, `updatedPermissions` or second `initialize`.
6. A turn is over when `claude` reports itself idle, not at the first
   `result`.

```

### 17, stage B budgets (lines 2242-2252)
```
Budgets (the "Load" kind of 16). All p95; measured by a slow load test on the
stage's fakes, recorded with the date and the machine in 16, "Budget record";
a stage is done only within them. Writer busy share: the share of wall time the host's state writer (host 7, one per host) spends in commits, fsync included, over the load run. B and D run with 2 owners (two client mods), so the per-owner caps hold; stage A's 8 host streams come from a test source in the host's own tests (no H2–H4).

| Stage | Load | Item delay, source to page | Stop, to idle at the page | Writer busy share | State bytes read and written |
|---|---|---|---|---|---|
| A | 8 host streams × 100 items/s of 1 KiB, through a mod consumer to a page | ≤ 50 ms | `cancel` to `ended`: ≤ 250 ms | ≤ 10% | none per item |
| B | 8 fake `claude` sessions, each 50 deltas/s and 5 finished entries/s of 4 KiB; transcripts at 10 and at 10 000 entries | ≤ 100 ms | `interrupt` to `idle {interrupted}`: ≤ 1 s | ≤ 50% | per finished entry ≤ 2 MiB, within 10% between 10 and 10 000 entries |
| C | as B on the real `claude` | ≤ 100 ms (kernel and host part) | ≤ 2 s | ≤ 50% | as B |
| D | B, plus 8 OpenAI sessions on the scripted Responses server, each one step/s with 4 tool calls; wire histories at 10 and at 1 000 steps | ≤ 100 ms | ≤ 1 s | ≤ 60% | per step ≤ 4 MiB, within 10% between 10 and 1 000 steps |
| E | D, with the Claude sessions through the checked provider and the scripted judge answering in 200 ms | ≤ 100 ms for items no verdict holds; a check adds ≤ judge time + 100 ms | ≤ 1 s | ≤ 60% | as D |
```

### 15, the host rows that name asks, checks or symlinks
```
contracts are accepted, F-AGENT-2 and W6 included). The checklist is applied
- [ ] `host/TECHNICAL.md` H2: the `initialized` item after `initialize` passes, never before the workspace consent is held (4.5, 10; P04); `claude-session.say`, `answer`, `interrupt` and `compact` are requests, applied after the call commits, so a deferred or failed call sends nothing to `claude` (2.1, 7); the host builds each `ask`, `check` and `ask-resolved` item itself and never copies bytes of a `claude` line into one, and `ask-resolved {by: answered}` carries the `outcome` (4.5; accepted, the human, 2026-11-21); the host pushes `ask-resolved {by: expired}` for a check it has not answered at its `expires-at` and writes nothing to stdin for it (4.5; S22; accepted, the human, 2026-11-21).
- [ ] `host/TECHNICAL.md` H2: a file tool's ask gives the kernel the path that will run with its symlinks resolved, before the kernel computes a scope key (9.6; stage B's first slice names how, in 4.5); until then no session has a `workspace` (PLAN S33 RISK 2, S34).
- [ ] `host/TECHNICAL.md` H2: `--strict-mcp-config` admits only the `watane` server, so an `mcp__watane__<tool>` name is always watane's (4.4); a scope key for such a tool (9.6, 14) relies on it (PLAN S33 RISK 3, S34).
- [ ] `host/TECHNICAL.md` H2: the host's `--settings=` for the shell grant always writes `sandbox.allowUnsandboxedCommands: false`, never leaves it out (Claude's default is `true`); a test reads the settings the host writes (4.1, 4.2, 14; PLAN S33).
- [ ] `app/TECHNICAL.md` new section: entering, showing (never the value) and removing the judge key and origin on the checked provider's manager page, with the origin shown next to the key and never prefilled, stored in `<home>/app/accounts/judge/<provider mod id>.json` and deleted when the provider is uninstalled (5, 10; N10).
```

### Deputy rulings for this round (the reviewer keeps or drops its findings in r2)
- F1.8: agentic-harness 17's stage B row sets item delay source to page at <= 100 ms p95, so 100 ms for the host's small-turn and ask-delivery checks is within the contract; keep 100 ms and say so in host 15 with this source. The three isolated passes are still wanted as the record.
- F1.9: the stage B row is a kernel-plus-host budget, measured end to end in agentic-harness S36 with the kernel. S100's part: dev's fake `claude` load mode can produce exactly the B load (8 sessions, 50 deltas/s and 5 finished entries/s of 4 KiB each, 2 owners), and the host test records the host's share (item delay host to consumer, `interrupt` to the host's `ask-resolved`/`result`, writer busy share) at that load. Source-to-page, interrupt-to-idle at the page, and state bytes per entry at 10 and 10 000 entries are S36's.
- F1.1 (`id`, not `ask`), F1.2 (resolved path in the ask), F1.3, F1.4: follow 4.5 and 15 above.

