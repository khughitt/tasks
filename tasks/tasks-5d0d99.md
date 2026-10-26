---
id: tasks-5d0d99
title: Compute downstream impact and honest graph coverage
status: done
priority: 2
size: m
complexity: high
process: direct
owner: design/tasks-e637a1
created: 2026-10-26T00:08:24Z
updated: 2026-10-26T01:05:01Z
started: 2026-10-26T00:56:04Z
completed: 2026-10-26T01:05:01Z
closed: 2026-10-26T01:05:01Z
depends: [tasks-ceee0d]
parent: tasks-e637a1
tags: []
source: "tasks-e637a1:plan-task-2"
agent: codex
origin: agent
spec: docs/specs/2026-10-25-blocker-ranking-design.md
plan: docs/plans/2026-10-25-blocker-ranking.md
step: "Task 2: Compute downstream impact and honest graph coverage"
---

Implement canonical graph counts, status breakdowns, rank, cycles and partial coverage. bb-scope boundary targets must include B, L and X, only L unfollowed. Consumes the existing captured Scope::scan result after Task 1; Ruling S1 removes the unused hard wait on tasks-e8d471. No new project walker. Plan 327fe22 approved by Keith, including Ruling S1. The review gate is released; preserve the remaining implementation dependency edges and start execution in a fresh session under the approved plan.

## Notes

- 2026-10-26T00:08:24Z (add): created
  provenance: {"harness_session":"codex:01a17532-7bd2-7062-bf78-835c57d1662f","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
- 2026-10-26T00:08:26Z (design/tasks-e637a1): Draft implementation plan is awaiting separate user review; after approval the controller unblocks this step, preserving implementation dependencies.
  provenance: {"harness_session":"codex:01a17532-7bd2-7062-bf78-835c57d1662f","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-26T00:36:45Z (design/tasks-e637a1): Keith approved plan 327fe22 and Ruling S1 in the controller session; review gate released, remaining dependencies unchanged. Execution is reserved for a fresh session; no Setup or implementation performed here.
- 2026-10-26T00:56:04Z (design/tasks-e637a1): started
  provenance: {"harness_session":"claude-code:bafecb5c-78ad-4c0f-b8fc-6772b931692d","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-26T01:05:01Z (design/tasks-e637a1): done
  provenance: {"harness_session":"claude-code:bafecb5c-78ad-4c0f-b8fc-6772b931692d","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-26T01:05:01Z (design/tasks-e637a1): src/blockers.rs build(): memoized canonical resolution over one captured scan, unique reverse adjacency, visited-set reach, iterative Tarjan cycles, boundary/unfollowed coverage, relief and own counts, deterministic rank; five unit tests green (d298cf3).
  provenance: {"harness_session":"claude-code:bafecb5c-78ad-4c0f-b8fc-6772b931692d","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
