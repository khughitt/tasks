---
id: tasks-06afbe
title: list --needs-review and show report a record as an unreviewed idea while a newer copy in another worktree has already promoted it to a todo goal with children; only writes notice the newer copy (stale_copy)
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-20T01:53:17Z
created: 2026-10-18T23:02:56Z
updated: 2026-10-23T09:08:22Z
depends: []
tags: [feedback, gap, "from:material", "from:mind6", friction]
agent: claude-code/claude-opus-5-5
---

Seen during a scope pass: the queue offered an idea whose branch copy (unmerged worktree) was already a todo implementation goal with plan children; show from the main checkout gave no warning. Expected: the queue row or show to flag that a newer copy exists in another checkout (as stale_copy does on write), so a scope pass skips it without manual worktree diffing.

## Notes

- 2026-10-20T01:53:17Z (main): scope: drop; duplicate of tasks-f3910b, whose scoped direct task covers show warnings for newer sibling copies and scope skips on warning or stale_copy; checked against ecb4fb5: show and list still omit stale::compare while writes call it; brief: docs/notes/2026-10-16-scope-queue-other-branches-brief.md; proposal: drop as duplicate of tasks-f3910b
- 2026-10-23T09:08:22Z (feedback): feedback from mind6: show reports an older doing status while the worktree copy is already done
- 2026-10-23T09:08:22Z (feedback): detail from mind6: A maintenance audit found a completed implementation and newer done record in its unmerged worktree while show from the registered checkout silently returned the older doing record. The worktree-only fallback helps when a record is absent, but not when both copies exist. The auditor had to compare record timestamps and contents explicitly to distinguish finished stacked work from incomplete implementation. Surface the newer authoritative checkout or a divergence warning in reads without weakening write protection or silently merging task files.
