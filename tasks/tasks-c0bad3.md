---
id: tasks-c0bad3
title: Establish Codex spawned-agent identity and hook semantics
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-06T10:41:53Z
updated: 2026-10-06T10:41:54Z
depends: []
parent: tasks-2fa8c6
tags: []
source: tasks-1ece46
agent: codex
---

Question: What are Codex's intended semantics for CODEX_SESSION_ID and CODEX_THREAD_ID in a spawned agent, and what session_id do that worker's hooks receive?

Where to start: tasks-1ece46's capture and evidence tables; official Hooks common-input documentation at https://learn.chatgpt.com/docs/hooks (checked 2026-10-06: subagent hooks use parent session_id, while subagent events include agent_id); exact Codex 0.160.1 source for command environment construction and hook dispatch. The current environment-variable documentation does not describe these two native variables. Compare work-claims spec Identity and relay's inherited-environment constraints.

Bound: trace those two environment assignments and the worker hook payload at the captured release, using official docs and source; cite immutable source locations and distinguish intended contract from observed inheritance. Compare with current docs only to identify version differences. No live harness sweep, native-ID overrides, product-code or host-config changes. If exact-version semantics are unavailable, state the gap explicitly.

Expected result: a concise identifier/parent/hook mapping and evidence-backed recommendation: tasks contract adaptation if the difference is deliberate, upstream repair only if Codex's own contract is violated. Assess thread as worker identity and session as parent association without approving a new schema. Update the brief, tasks-5e2472 and tasks-56b450; keep controller aggregation and authorization separate.

Ideas it wakes: on completion add the finding to tasks-5745bb, tasks-616cd7, tasks-cf8687, tasks-290fa3, tasks-6e2d15 and tasks-f537a8 with tasks note in the same commit. These remain one remediation cluster rather than six independent scoping jobs.

## Notes

- 2026-10-06T10:41:53Z (main): concerns: tasks-1ece46 extension — Verify Codex intended variable and hook semantics before treating captured disagreement as an upstream delivery defect.
