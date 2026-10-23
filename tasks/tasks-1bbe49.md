---
id: tasks-1bbe49
title: Expose dependency satisfaction separately from overall readiness
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-22T23:05:11Z
created: 2026-10-22T21:04:57Z
updated: 2026-10-22T23:05:11Z
depends: [tasks-e90b4f]
parent: tasks-606dd3
tags: [feedback, gap, "from:crew"]
agent: codex
---

A read-only allocation study needs a canonical dependency-met answer that also covers dropped and recurring done records. The installed CLI exposes dependency ids, statuses and resolved flags through show, but resolved means found; ready combines dependency checks with claims, parks and other admission filters, so absence cannot answer the dependency question. Add a read-only query or an explicit per-dependency verdict so callers do not copy the tracker rule. Current workaround records the answer as unknown and uses positive ready membership only as evidence that all dependencies passed.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-22-dependency-state-brief.md — observed dependency rules and remaining contract decisions

## Notes

- 2026-10-22T23:05:11Z (main): scope: briefed; d0564d1 confirms show.resolved means found and ready separately follows moves then checks closed status; recurring done remains satisfied until restarted; JSON shape, unavailable cases and batching need reviewed design; waits on tasks-e90b4f, which updates the brief and wakes this idea before merge; original body retained; brief: docs/notes/2026-10-22-dependency-state-brief.md
