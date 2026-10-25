---
id: tasks-c750e1
title: Large task-show note histories can overflow an agent read
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-25T16:04:10Z
created: 2026-10-24T17:36:29Z
updated: 2026-10-25T16:04:10Z
depends: [tasks-608d02]
parent: tasks-96c00c
tags: [feedback, friction, "from:tasks"]
agent: codex
---

A read of an implementation goal with many retained-item notes, batched with related task queries, exceeded the agent tool output budget and was truncated. The caller had to repeat the reads with selected JSON fields and filtered notes. Consider a discoverable bounded task-detail read that preserves warnings and provides a full-data recovery route; this is separate from bounded prime.

Scope handoff: [ref:brief] records the measured single-task contribution, existing prime precedent and open design questions. tasks-608d02 supplies the reviewed task-detail design and plan before implementation is scoped.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-19-cli-consumer-reads-brief.md — measured task-detail size and unresolved history/recovery contract

## Notes

- 2026-10-25T16:04:10Z (main): scope: briefed; measured show tasks-65b733 at 35902 bytes with 33401 bytes of notes (54 entries, 47 retained records); e792f05 show code/help has no bounded-history option; reused goal/brief and filed tasks-608d02 (P2/s/high/planned) for the unresolved detail/history/omission/recovery contract, with dependency and wake instruction; original body retained; origin: filing event unavailable (no creation occurrence or session stamp), no older origin_session supplied, overall origin unknown and unchanged; brief: docs/notes/2026-10-19-cli-consumer-reads-brief.md
  provenance: {"harness_session":"codex:01a173c9-06d1-7b71-8e1d-5e90cc4839ab","harness_session_source":"CODEX_THREAD_ID"}
