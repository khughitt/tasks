---
id: tasks-9949f3
title: "Implement the record-home design: stale-copy refusal, claim follows holder, show fallback"
status: todo
priority: 2
size: l
complexity: high
process: planned
created: 2026-09-30T10:28:06Z
updated: 2026-09-30T10:28:41Z
depends: []
parent: tasks-c4ad8e
tags: [worktree]
agent: claude-code/claude-opus-5-5
spec: docs/specs/2026-09-30-record-home-design.md
---

Implements docs/specs/2026-09-30-record-home-design.md (accepted 2026-09-30, tasks-ab8d2d). The spec is approved; the implementation plan still needs review before code. Closes tasks-2c0a1d, tasks-bb53e5 and the show half of tasks-fbc32b.

## Notes

- 2026-09-30T10:28:41Z (ab8d2d-record-home): Evidence 2026-09-30: closing tasks-ab8d2d, a note on tasks-bb53e5 from this worktree forked the record against a newer committed note in main (the warning fired after writing); resolved by merging main and keeping both notes. Under the spec it would have refused as stale_copy.
