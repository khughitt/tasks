---
id: tasks-c8ad5b
title: Establish the smallest reliable way to retain reviewed evidence versions
status: todo
priority: 3
size: s
complexity: mid
process: direct
created: 2026-10-20T01:40:47Z
updated: 2026-10-20T01:40:47Z
depends: []
parent: tasks-7102c1
tags: []
source: tasks-d4cf54
agent: codex
---

Question: Which concrete evidence needs revision pinning beyond the current attach-a-copy workflow, and what retention/retrieval promise can tasks actually keep?

Where to start: [ref:brief] and docs/specs/2026-10-19-task-references-design.md sections 9–11; src/refs.rs, src/resolve.rs, src/attachments.rs, and the existing review-report attachments on tasks-6eeecd. The current parser has no revision attribute and reads current bytes; do not re-establish that as the research result.

Bound: inspect one tracked local report, one untracked artifact, and one external-document case; use disposable fixtures where no real example is available and label them synthetic. Compare attached snapshots with a Git revision and an external version identifier. Check changed/deleted source bytes, absent history/provider versions, detached or reused attachment names, and existing size/safety limits. No product-code change, general artifact store, or external service setup. Record any missing real-world requirement explicitly.

Expected result: record on this task and in the brief an evidence matrix and recommendation: existing snapshots suffice, or a minimal pinning boundary with retention guarantees, failure semantics, and acceptance cases for later design. A content hash verifies identity but does not itself retain evidence.

Ideas it wakes: On completion, run `tasks note tasks-d4cf54 --no-reviewed "<finding>"` before this result's branch merges.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-19-reference-extensions-brief.md — question, evidence, and scope boundaries
