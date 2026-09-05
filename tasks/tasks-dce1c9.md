---
id: tasks-dce1c9
title: Lock feedback recurrence
status: done
priority: 2
size: xs
owner: feat/doing-claims
created: 2026-09-05T10:21:31Z
updated: 2026-09-05T12:51:25Z
depends: [tasks-8dcf64]
parent: tasks-d184e3
tags: [claims]
plan: docs/plans/2026-09-05-work-claims.md
step: "Task 9: Lock `feedback` recurrence"
---

## Notes

- 2026-09-05T12:51:25Z (feat/doing-claims): Recurrence also preloads and prunes the target claim store under the target-only lock per next-write policy.
- 2026-09-05T12:51:25Z (feat/doing-claims): Serialized feedback recurrence under the target project lock and pruned stale claims after successful updates.
