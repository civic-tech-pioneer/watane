Subsystem: host
Effort: medium
Depends on: none
Slice: S99 — Alice has a first conversation with Claude on dev's fake `claude` (stage B, watane half)
## Goal
Close the two breaches S99's adversary found, and two small fail-open spots it named as risks.
## Acceptance
AC1. `claude_launch::adv_s99_file_allow_with_an_empty_or_star_argument_is_a_bare_name` passes:
     an `allow` on `Read`, `Glob`, `Grep`, `Edit`, `Write` or `NotebookEdit` whose argument is empty
     or exactly `*` is `bad-launch {field: "allow"}` (Claude Code reads `Tool()` and `Tool(*)` as the
     bare name). Write it into host 19.3, Protected reads, second rule.
AC2. `claude::adv_s99_a_start_in_its_version_probe_makes_a_scratch_folder_after_the_owners_remove`
     passes: the start checks `stopped()` again after the `--version` probe and before it makes a
     scratch folder or row (and before any later step that makes a file); a stopped start makes
     nothing.
AC3. (RISK 5) The first line after the `initialize` answer that is not a control line must be a
     valid `system/init` (UTF-8, one JSON object, no duplicate key); any other line there, wrapped or
     not, kills the group and ends the stream `mismatch("init")`. A test with the host's fixture fake
     playing a non-UTF-8 init line and one with a duplicate key. Write the rule into 19.3 step 3.
AC4. (RISK 4) The stop/started handshake of `Session` cannot miss both writes: use one `Mutex` (or
     `SeqCst` on both sides, with a comment saying why). No new test needed if a race test cannot be
     made reliable; say so in the report.
AC5. Nothing else changes: every other test that passed still passes.
## Spec
host TECHNICAL 19.3 (Protected reads, control protocol step 3, Folders: Lifetime).
## Interface
none
## Files
host/src/agent/claude/*.rs, host/tests/it/claude.rs, host/tests/it/claude_launch.rs, host/tests/fixtures/fake-claude/*, host/TECHNICAL.md.
## Changed tests
none (the two red tests are the adversary's; keep them as written).
## Out of scope
The adversary's RISK 1 to 3 (protected paths per workspace root, session ids per owner, spellings of
the protected roots): questions for stage A3, in host 18.
