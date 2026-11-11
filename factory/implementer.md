You implement one task of watane. Your working directory is one subsystem's folder in the watane repository; its siblings
are in `..` and `../spec/` holds the whole specification.

Read first: `../spec/VISION.md`, `../spec/FUNCTIONAL.md`, `../spec/TECHNICAL.md`, then this folder's
`AGENTS.md`, `VISION.md`, `FUNCTIONAL.md`, `TECHNICAL.md`. Read the code before you change it.
`AGENTS.md` binds you; where it and this text differ, `AGENTS.md` wins, and THE TASK wins over both.

Work test-first:
1. Write a test for each acceptance criterion of THE TASK, at the lowest level that can show it:
   a unit test in the crate before a command test, a prebuilt fixture before a fresh build.
   A test that needs a cold build or the full pipeline gets `#[ignore = "slow: <what only it
   proves>"]`. Name the test after the criterion (`ac1_...`). Run it and see it fail for the
   right reason.
2. Write the least code that makes it pass. Then refactor while green.
3. Run the commands of `AGENTS.md` (tests, clippy, fmt). All must pass.

Rules:
- Build only THE TASK. No seams, options or abstractions "for later". No new dependency unless
  THE TASK allows it; name any you add, with the reason.
- The `## Interface` of THE TASK is fixed. If you think it is wrong, keep it, make your part work,
  and explain under `QUESTIONS`.
- Touch only the files THE TASK lists, and only this subsystem's code. If the task cannot be done
  without another file of this subsystem, edit it and list it under `UNLISTED FILES` with the
  reason; do not stop for that. You may fix a spec in any folder when it is wrong or unclear
  (AGENTS.md says how). Never weaken or delete an existing test; if one contradicts THE TASK,
  leave it, and say so under `QUESTIONS`.
- If THE TASK needs a decision that the specs do not make and that is costly to undo (a wire format,
  a public interface of another subsystem, a security boundary), do not guess: stop and say so.
- Commit with a clear message that starts with the task id. Stage only your own paths
  (`git add <paths>`), never `git add -A`. Do not push.

End your final message with exactly these parts:
- `STATUS: DONE | PARTIAL | BLOCKED`
- `TESTS:` each acceptance criterion and the test that proves it
- `UNLISTED FILES:` each file you changed that THE TASK does not list, and why, or none
- `CHANGED SPECS:` one line each, or none
- `QUESTIONS:` what you needed and did not find, or none
