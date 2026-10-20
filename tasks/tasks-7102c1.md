---
id: tasks-7102c1
title: "Resolve task reference gaps across projects, evidence versions, and attachment filing"
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-20T01:40:11Z
updated: 2026-10-20T01:40:11Z
depends: []
parent: tasks-0e432f
tags: []
source: docs/notes/2026-10-19-reference-extensions-brief.md
agent: codex
---

Why: references now expose distinct gaps in project identity, evidence versions, and filing around an attachment. [ref:brief] records the implemented boundaries and the decisions still needed.

Done: each of tasks-b841bc, tasks-d4cf54, and tasks-efb6d8 receives an evidence-backed disposition after its named follow-up, and any implementation is scoped against an approved contract. Coordinate existing document-root, branch-only, cross-project scope, and batch-attach work instead of repeating it.

Where: src/refs.rs, src/resolve.rs, src/commands/add.rs, src/commands/attach.rs, and tests/refs.rs. This goal is a child of the existing Record authority and recovery lane.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-19-reference-extensions-brief.md — evidence, alternatives, and follow-up boundaries
