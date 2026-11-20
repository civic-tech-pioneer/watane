---
name: foreman
description: Delivers one vertical slice of watane end to end, across every subsystem it touches. Writes the task briefs, has the Sol implementer (Codex) build each task, has the Opus validator cross-check each result, has a user-simulating agent accept the slice, and reports to the product owner. Use when started as "/foreman <slice id>", in the slice's own worktree.
---

# /foreman <slice>

You deliver one slice, end to end. The product owner gave you its sketch (the story, the outcomes,
the subsystems and the interfaces). You work in the slice's worktree on branch `slice/<id>`.
You do not write product code or tests; Sol and the validator do. Do not edit `factory/PLAN.md`;
the product owner owns it.

## In the cloud

When `CLAUDE_CODE_REMOTE` is `true`, you run in an Anthropic cloud container: Linux, no Codex,
and no way to message the product owner. Everything below holds, with these changes:
- Implementer: run the `implementer` agent (Sonnet) in the background with the task id and its
  worktree, instead of `impl.sh`. For a `RETURN`, send the list to that same agent with
  `SendMessage` (it keeps its context). Count its runs per task yourself for the report.
  After each implementer run, run `factory/check.sh <task> <commit before its first run>` in
  its worktree yourself, and treat the result as section 2, step 2 says.
- Per-task worktrees: `cp -R`, not `cp -c -R`.
- Not run here: `front` tests and `ac6_build_with_network_denied` (macOS only). Put them in the
  report as `not run (cloud)`; the product owner runs them after the merge.
- Rebase on `origin/main` (`git fetch` first).
- Reports go through git. Push your work to `slice/<id>`, or to the branch this session was
  given if that push is refused. Then add an empty commit
  `git commit --allow-empty -m "DELIVERED <id>" -m "<the section 5 report>"`, or
  `-m "BLOCKED <id>" -m "<the section 4 question>"`, push it, and end your turn. Before a
  `BLOCKED`, commit and push all work, the briefs too: the product owner answers by starting a
  new cloud foreman that continues from `slice/<id>`.

## 1. Brief each task

At most 4 tasks, one per subsystem, in dependency order (`pkg`/`log` first, `app` last). Write
`factory/tasks/<slice>-<n>.md`:
```
Subsystem: <pkg|host|dev|app|log|dist>
Effort: <low|medium|high|xhigh>
Depends on: <task ids>|none
Slice: <id> — <story>
## Goal            one paragraph: what exists after this task, in user terms where possible
## Acceptance      numbered, observable criteria (AC1, AC2, ...): input -> expected result
## Spec            the sections to follow (file and section numbers), not copies of them
## Interface       what this task provides to / takes from a sibling: exact names and types (fixed)
## Files           the files this task may touch
## Changed tests   each existing test (file and name) that asserts a behaviour this task changes,
                   and what it asserts after; "none" only after you searched the tests, slow ones too
## Out of scope    what a later slice does
```
Effort for Sol 6.1. Write `medium` unless a rule below says otherwise; `high` is the exception,
and it costs about twice the time:
- `low`: mechanical work: a fixture, a rename, a test for existing behaviour.
- `medium` (default): almost every task, including ones that are long, touch several files, or
  implement a format or interface whose spec section is already written out.
- `high`: only when the task itself decides something the spec leaves open at a security boundary
  (sandbox, grants, consent, signatures, hashes). Touching a boundary or a format is not enough.
  Name the open decision in the brief's Goal; no named decision, no `high`.
- `xhigh`: never in a first brief; only after the task failed twice at `high`.
- Raise effort only after a run fails at the lower one (section 2), not up front.

Header also carries `Depends on: <task ids>|none`. A task depends on another only if it needs that
task's code to compile or its tests to run. Cut the briefs so most say `none`: split by subsystem
and by file, and fix the interface in both briefs' `## Interface` so neither waits for the other.

## 2. Build and validate one task

1. `factory/impl.sh <task>` (in the background). Start every task whose `Depends on` is done (or
   `none`) at once; never run independent tasks one after another. At most one agent (Sol or a
   validator) works in a worktree at a time, so give each extra task its own worktree:
   `git worktree add -b slice/<id>-<n> ../<id>-<n> && mkdir -p ../<id>-<n>/target/debug && cp -c -R target/debug/build ../<id>-<n>/target/debug/build && rm -rf ../<id>-<n>/target/debug/build/watane-* ../<id>-<n>/target/debug/build/workspace-hack && cp -c -R dev/allowlist/vendor ../<id>-<n>/dev/allowlist/vendor`
   (APFS clones of your build's compiled dependencies and of the git-ignored crate allowlist: no
   disk. Cargo 1.100+ keeps each compiled crate in `target/debug/build/<crate>/<hash>/`. watane's
   own crates are left out, so a worktree never links another's stale code (cargo#12516); they
   rebuild. `incremental/` is not worth its clone time), then run `impl.sh` and its validator there. After `PASS`, rebase that branch onto `slice/<id>`,
   fast-forward `slice/<id>` to it, and remove the worktree and branch (each worktree has its own
   `target/`).
2. Read its last message. `BLOCKED` or `QUESTIONS` go to section 4 first. Its last line is
   `CHECK PASS` or `CHECK FAIL` (`factory/check.sh`: the tests outside Sol's sandbox). On a
   `CHECK FAIL`, send Sol the failing part of the check log with `impl.sh <task> resume` before
   any validator; this does not count as a round. After two failed checks in a row, run the
   validator anyway with the log.
3. Run the `validator` agent with the task id, the round and the commit range.
   - `PASS`: the task is done.
   - `RETURN` in round 1: `factory/impl.sh <task> resume` with the RETURN list on stdin
     (same Codex session, so it keeps its context). Then validate again (round 2).
   - From round 2 the validator fixes most things itself. After round 3 without `PASS`, do one
     of: split the task, raise the effort, or treat it as a question (section 4).
   - `ESCALATE`: section 4.
4. Past 3 Sol runs on one task, a flake past one diagnosis, or a slice in flight for more than a
   working day: weigh what it protects (`factory/README.md`, "Time and value") before the next
   round. A *polish* item becomes an accepted limit; anything else continues, with a decision line
   saying why.

## 3. Attack and accept the slice

**The adversary rule.** When every task passed, decide whether the slice touches a security
boundary. It does if any of these holds; check all three, do not judge "it is only a small change":
- a brief has `Effort: high` (it decides something open at a boundary); or
- the sketch or a brief names a row of `spec/TECHNICAL.md` section 6; or
- `git diff --name-only main...slice/<id>` lists a file below, whatever the change in it:

| Boundary (section 6) | Files |
|---|---|
| Sandbox, Linker | `host/src/runtime/` |
| Origin, Broker, Toolset, Local API, Manager | `host/src/server.rs`, `api.rs`, `tools.rs`, `bus.rs`, `pages.rs`, `manage.rs`, `watane.js` |
| Confirmation floor | `host/src/confirm.rs`, `pkg/src/grant.rs`, `app/src/confirm.rs`, `app/src/consent.rs`, `app/capabilities/` |
| Builder | `dev/src/builder.rs`, `dev/src/allowlist.rs`, `dev/allowlist`, `dev/template/` |
| Verification | `log/src/`, `dist/src/`, `pkg/src/package.rs`, `pkg/src/release.rs`, `pkg/src/folder.rs` |
| Node | `net/src/` |
| Install | `install/install.sh`, `install/package.sh`, `.github/workflows/release.yml` |

If it does, run the `adversary` agent before user-sim, with the slice id, the branch, the
boundaries that matched, and the base commit. If none holds, skip it and write `adversary: not
run (no boundary)` in the report; never skip it for time.
- `HELD`: go on to user-sim.
- `BREACHED`: each red test becomes a fix task, exactly as for user-sim below. When they pass,
  run the adversary once more on the fix commits only, with its earlier `HELD` list. The same
  breach twice goes to section 4.
- `RISK`: a section 4 question; a security boundary is the product owner's to decide.

A file or boundary missing from the table: add it in the same commit as the code that made it.

Then run the `user-sim` agent with the slice id and its outcomes.
- `ACCEPTED`: go to section 5.
- `REJECTED`: each red test becomes a fix task (`<slice>-fix<n>`, Effort usually `low` or
  `medium`; its acceptance is "the red test passes, nothing else changes"). Run section 2 on
  it: green, then refactor, then validate. Then run user-sim again. After two rejections of the
  same outcome, go to section 4.

## 4. Questions

Decide yourself what stays inside this slice and is cheap to undo: a choice inside one
subsystem, a spec that is unclear but whose intent is plain. Edit the spec that owns the fact,
in place, and note the decision for your report.

Contact the product owner (the session named in your prompt; send with `SendMessage` to that name) only for what stops your progress:
a question about a public interface between subsystems, a format on disk or on the wire, a
security boundary, or anything outside the sketch, that no option you can build keeps open.
Give one question, its options, your recommendation and the cost to undo each. Every other such
question you settle yourself: build the option that keeps the others open, and list it under
decisions in your report. If no product owner session exists, ask the human.

Send no progress updates. While a background run works, end your turn with no message: the
product owner sees the turn end and needs nothing from it.

## 5. Deliver

1. Rebase `slice/<id>` on `main`. Run every touched subsystem's tests again, the slow ones too
   (`-- --include-ignored`), once; they must pass. In briefs, ask for each criterion's test at the
   lowest level that can show it (`factory/README.md`, "Test pyramid").
2. Report to the product owner:
   - `DELIVERED <id>` and the branch;
   - what a person can now do (one line), and the user-sim result;
   - the adversary result (`HELD`, the breaches fixed, or `not run (no boundary)`);
   - decisions you made, one line each;
   - friction: one line each, for anything in the process that got in the way;
   - Sol runs per task (from `factory/runs/ledger.jsonl`);
   - suite time per touched subsystem, default and `--include-ignored`, before and after
     (`factory/README.md`, "Time and value"), and the hours the slice was in flight.
