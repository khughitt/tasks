---
id: tasks-c543ae
title: "Enforce the halt tag: refuse new starts in a halted project, with a reasoned override"
status: todo
priority: 1
size: m
complexity: mid
process: planned
created: 2026-09-30T10:09:44Z
updated: 2026-09-30T10:09:44Z
depends: []
tags: [cross-project]
source: ops-5beefd
agent: codex
---

Contract in ops docs/specs/2026-09-29-test-latency-escalation-design.md section 6: a project is halted while its registered checkout holds a task tagged halt whose status is not done or dropped (idea, todo, doing, blocked and shelved all halt; a deferred one too). start refuses every new start (todo, blocked, the next occurrence of a completed recurring task) with error kind halted naming the halt task; allowed: resuming a doing task, the halt task, its descendants and dependencies, and any task whose priority is at least the halt task's. Override: tasks start <id> --force --reason "<why>" writes one note on each task naming the other, the session and the reason; --force without --reason is refused under a halt. prime prints a halt: line first; ready and next list the halt task and what it allows and count what they hid. An unreadable registered checkout fails the start. Tests listed in the spec's section 11 under The halt contract. Spelling of --reason on start is this piece's to confirm against cli.toml.
