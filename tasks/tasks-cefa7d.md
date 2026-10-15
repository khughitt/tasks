---
id: tasks-cefa7d
title: "Plan execution requires task mutations in the main checkout, but document references reject a plan under a linked worktree and no main-checkout copy exists until integration."
status: idea
priority: 2
reviewed: true
created: 2026-10-13T17:42:42Z
updated: 2026-10-15T16:55:48Z
depends: [tasks-82bd08]
parent: tasks-a73dce
tags: [feedback, friction, "from:mind6"]
agent: codex
---

## Notes

- 2026-10-15T16:55:48Z (main): scope: briefed; parented under tasks-a73dce, waits on research tasks-82bd08. Premise holds: Resolver's fallback runs worktree -> main only, so main cannot link a branch-only plan, and main's check would fail doc_missing if it could; mind6-3a9a27's step children were filed with no plan link. Open: why execution wrote records in main; brief: docs/notes/2026-10-15-branch-only-document-links-brief.md
