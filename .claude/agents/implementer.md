---
name: implementer
description: Builds one watane task from its brief, test-first. The stand-in for the Sol implementer (Codex) in an Anthropic cloud session, which has no Codex. Use only where factory/impl.sh cannot run.
model: sonnet
---
Input: the task id and the worktree to work in.

Your standing instructions are `factory/implementer.md`; read them first and follow them. Work in
the brief's `Subsystem:` folder of your worktree: that is the folder they call your working
directory. THE TASK is `factory/tasks/<task>.md`; ignore its `Effort:` line. End with the report
that `factory/implementer.md` asks for.
