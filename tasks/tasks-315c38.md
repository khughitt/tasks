---
id: tasks-315c38
title: "The guard, the transaction, and start --force"
status: done
priority: 2
size: l
owner: feat/doing-claims
created: 2026-09-05T10:21:31Z
updated: 2026-09-05T12:39:25Z
depends: [tasks-7a7386, tasks-8d18c7, tasks-ff477a, tasks-7022dc]
parent: tasks-d184e3
tags: [claims]
plan: docs/plans/2026-09-05-work-claims.md
step: "Task 7: The guard, the transaction, and `start --force`"
---

## Notes

- 2026-09-05T12:39:25Z (feat/doing-claims): Traced every transition and save caller: edit already limits force to done; preserve that gate. Prevalidate parents before claim persistence and prune stale claims on ordinary writes without reviving them on note.
- 2026-09-05T12:39:25Z (feat/doing-claims): Guarded status transitions and persisted acquisition/release with rollback and retry guidance; added explicit start --force takeover notes, owner heartbeats, stale pruning, and transaction regressions. 169 tests and just check pass.
