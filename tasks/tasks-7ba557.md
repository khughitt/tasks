---
id: tasks-7ba557
title: no way to move a task between registered projects
status: idea
priority: 2
created: 2026-09-12T09:35:46Z
updated: 2026-10-06T16:52:06Z
depends: []
parent: tasks-a7460c
tags: [feedback, gap, "from:dots"]
---

Moving five open tasks to a new project meant re-adding each with --source and dropping the original by hand; a move that keeps id history, notes, and cross-project deps would replace that.

Next action: scope task movement against tasks-7580d2's canonical-id/alias contract and existing source-based add/drop workflow. Establish what must retain identity, notes, attachments, dependencies and parent associations, including partial failure and rerun behavior; distinguish project moves from prefix renames. Reuse the shared registry/resolver and do not add a second identity mechanism. Keep the move feature as an idea until those requirements have evidence and a reviewed design.

## Notes

- 2026-10-06T16:52:05Z (main): organization: assigned to project location/migration; reuse the canonical identity contract and scope the move's history/dependency semantics before proposing implementation.
