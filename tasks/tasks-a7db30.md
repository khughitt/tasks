---
id: tasks-a7db30
title: Ship the scoped CLI report and user guidance
status: doing
priority: 2
size: m
complexity: high
process: direct
owner: design/tasks-e637a1
created: 2026-10-26T00:08:28Z
updated: 2026-10-26T01:09:43Z
started: 2026-10-26T01:09:43Z
depends: [ops-b36208, tasks-5d0d99]
parent: tasks-e637a1
tags: []
source: "tasks-e637a1:plan-task-3"
agent: codex
origin: agent
spec: docs/specs/2026-10-25-blocker-ranking-design.md
plan: docs/plans/2026-10-25-blocker-ranking.md
step: "Task 3: Ship the scoped CLI report and user guidance"
---

Adopt landed ops vocabulary with parser changes; implement blockers JSON, compact pretty output, host-local attention/claims, tests and README/skill guidance. No change to ready/next order or other JSON contracts. Plan 327fe22 approved by Keith, including Ruling S1. The review gate is released; preserve the remaining implementation dependency edges and start execution in a fresh session under the approved plan.

## Notes

- 2026-10-26T00:08:28Z (add): created
  provenance: {"harness_session":"codex:01a17532-7bd2-7062-bf78-835c57d1662f","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
- 2026-10-26T00:08:30Z (design/tasks-e637a1): Draft implementation plan is awaiting separate user review; after approval the controller unblocks this step, preserving implementation dependencies.
  provenance: {"harness_session":"codex:01a17532-7bd2-7062-bf78-835c57d1662f","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-26T00:36:51Z (design/tasks-e637a1): Keith approved plan 327fe22 and Ruling S1 in the controller session; review gate released, remaining dependencies unchanged. Execution is reserved for a fresh session; no Setup or implementation performed here.
- 2026-10-26T01:09:43Z (design/tasks-e637a1): started
  provenance: {"harness_session":"claude-code:bafecb5c-78ad-4c0f-b8fc-6772b931692d","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
