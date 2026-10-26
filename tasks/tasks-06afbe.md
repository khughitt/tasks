---
id: tasks-06afbe
title: list --needs-review and show report a record as an unreviewed idea while a newer copy in another worktree has already promoted it to a todo goal with children; only writes notice the newer copy (stale_copy)
status: dropped
priority: 2
created: 2026-10-18T23:02:56Z
updated: 2026-10-26T15:58:38Z
closed: 2026-10-26T15:58:38Z
depends: []
tags: [feedback, gap, "from:material", "from:mind6", friction]
agent: claude-code/claude-opus-5-5
---

Seen during a scope pass: the queue offered an idea whose branch copy (unmerged worktree) was already a todo implementation goal with plan children; show from the main checkout gave no warning. Expected: the queue row or show to flag that a newer copy exists in another checkout (as stale_copy does on write), so a scope pass skips it without manual worktree diffing.

## Notes

- 2026-10-20T01:53:17Z (main): scope: drop; duplicate of tasks-f3910b, whose scoped direct task covers show warnings for newer sibling copies and scope skips on warning or stale_copy; checked against ecb4fb5: show and list still omit stale::compare while writes call it; brief: docs/notes/2026-10-16-scope-queue-other-branches-brief.md; proposal: drop as duplicate of tasks-f3910b
- 2026-10-23T09:08:22Z (feedback): feedback from mind6: show reports an older doing status while the worktree copy is already done
- 2026-10-23T09:08:22Z (feedback): detail from mind6: A maintenance audit found a completed implementation and newer done record in its unmerged worktree while show from the registered checkout silently returned the older doing record. The worktree-only fallback helps when a record is absent, but not when both copies exist. The auditor had to compare record timestamps and contents explicitly to distinguish finished stacked work from incomplete implementation. Surface the newer authoritative checkout or a divergence warning in reads without weakening write protection or silently merging task files.
- 2026-10-26T15:42:55Z (main): scope: drop; renewed duplicate proposal after the 2026-10-23 recurrence: tasks-f3910b already requires show warnings for every newer sibling copy, including doing here versus done there, and scope skips on warnings or stale_copy; verified at 73893a250530fbc9de3650151c50b860f7451f51: show::read_record and describe read the local copy without stale::compare, while refuse_stale_copy invokes that comparison on writes; record-home spec section 6.1 preserves local reads, so warning rather than silent redirection remains the scoped remedy; origin: creation filing event unavailable (creation occurrence note absent); recurrence filing event unavailable (2026-10-23 feedback occurrence has no harness_session or root_session stamp); no origin_session recorded, no older-request stage identified, origin remains unknown; existing brief unchanged because the recurrence adds a status example already covered: docs/notes/2026-10-16-scope-queue-other-branches-brief.md; proposal: drop as duplicate of tasks-f3910b
  provenance: {"harness_session":"codex:01a178dd-e493-75c0-920b-0160ba39ab58","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-26T15:58:38Z (main): dropped
  provenance: {"harness_session":"codex:01a178dd-e493-75c0-920b-0160ba39ab58","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-26T15:58:38Z (main): User approved the scope proposal: duplicate of tasks-f3910b, which covers warnings for newer sibling task copies and scope skips.
  provenance: {"harness_session":"codex:01a178dd-e493-75c0-920b-0160ba39ab58","harness_session_source":"CODEX_THREAD_ID"}
