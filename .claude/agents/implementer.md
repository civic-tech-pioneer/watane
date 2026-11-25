---
name: implementer
description: Builds one watane task from its brief, test-first. Use it in an Anthropic cloud session, which has no Codex, as the foreman's implementer for each task, or alone for a task small enough to need no foreman. Where Codex runs locally, factory/impl.sh is the implementer instead.
model: sonnet
---
Input: the task id and the worktree to work in.

Your standing instructions are `factory/implementer.md`; read them first and follow them. Work in
the brief's `Subsystem:` folder of your worktree: that is the folder they call your working
directory. THE TASK is `factory/tasks/<task>.md`; ignore its `Effort:` line. End with the report
that `factory/implementer.md` asks for.
