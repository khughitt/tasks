---
id: tasks-5d0d99
title: Compute downstream impact and honest graph coverage
status: blocked
priority: 2
size: m
complexity: high
process: direct
created: 2026-10-26T00:08:24Z
updated: 2026-10-26T00:08:26Z
depends: [tasks-e8d471, tasks-ceee0d]
parent: tasks-e637a1
tags: []
source: "tasks-e637a1:plan-task-2"
agent: codex
origin: agent
spec: docs/specs/2026-10-25-blocker-ranking-design.md
plan: docs/plans/2026-10-25-blocker-ranking.md
step: "Task 2: Compute downstream impact and honest graph coverage"
---

Implement canonical graph counts, status breakdowns, rank, cycles and partial coverage. bb-scope boundary targets must include B, L and X, only L unfollowed. Requires integrated incoming-scan work, not merely a done record on another branch. Draft implementation plan is awaiting separate user review; after approval the controller unblocks this step, preserving implementation dependencies.

## Notes

- 2026-10-26T00:08:24Z (add): created
  provenance: {"harness_session":"codex:01a17532-7bd2-7062-bf78-835c57d1662f","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
- 2026-10-26T00:08:26Z (design/tasks-e637a1): Draft implementation plan is awaiting separate user review; after approval the controller unblocks this step, preserving implementation dependencies.
  provenance: {"harness_session":"codex:01a17532-7bd2-7062-bf78-835c57d1662f","harness_session_source":"CODEX_THREAD_ID"}
