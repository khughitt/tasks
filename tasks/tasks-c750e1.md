---
id: tasks-c750e1
title: Large task-show note histories can overflow an agent read
status: idea
priority: 2
created: 2026-10-24T17:36:29Z
updated: 2026-10-25T16:03:35Z
depends: [tasks-608d02]
parent: tasks-96c00c
tags: [feedback, friction, "from:tasks"]
agent: codex
---

A read of an implementation goal with many retained-item notes, batched with related task queries, exceeded the agent tool output budget and was truncated. The caller had to repeat the reads with selected JSON fields and filtered notes. Consider a discoverable bounded task-detail read that preserves warnings and provides a full-data recovery route; this is separate from bounded prime.

Scope handoff: [ref:brief] records the measured single-task contribution, existing prime precedent and open design questions. tasks-608d02 supplies the reviewed task-detail design and plan before implementation is scoped.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-19-cli-consumer-reads-brief.md — measured task-detail size and unresolved history/recovery contract
