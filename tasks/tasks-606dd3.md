---
id: tasks-606dd3
title: Explain dependency satisfaction and intentional waits
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-22T23:04:06Z
updated: 2026-10-22T23:04:06Z
depends: []
parent: tasks-46d207
tags: []
source: docs/notes/2026-10-22-dependency-state-brief.md
agent: codex
---

Make dependency state understandable to consumers and maintainers while preserving the current blocking rules. First milestone: agree a canonical dependency-verdict JSON contract and correct guidance for an intentional shelved wait.

Why: [ref:brief] records both reports, verified source paths and the boundary between dependency satisfaction and admission to work.

Done: the dependency-reporting idea has an explicit disposition after its reviewed design, and the shelved-dependency guidance accepts retaining a recorded intentional wait. Preserve each child's process and review gates; a brief is not design approval.

Where to look: [ref:brief], src/commands/show.rs, src/commands/list.rs and src/commands/check.rs.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-22-dependency-state-brief.md — observed dependency rules and remaining contract decisions
