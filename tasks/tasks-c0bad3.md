---
id: tasks-c0bad3
title: Establish Codex spawned-agent identity and hook semantics
status: done
priority: 1
size: s
complexity: mid
process: direct
owner: investigate/tasks-c0bad3
created: 2026-10-06T10:41:53Z
updated: 2026-10-06T17:08:39Z
started: 2026-10-06T16:59:40Z
completed: 2026-10-06T17:08:37Z
depends: []
parent: tasks-2fa8c6
tags: []
source: tasks-1ece46
agent: codex
---

Question: Does SubagentStart/SubagentStop agent_id equal the spawned worker's CODEX_THREAD_ID, and does Codex intend CODEX_SESSION_ID to identify the parent conversation while CODEX_THREAD_ID identifies the worker? Current docs already answer hook session_id: it is the parent's.

Where to start: tasks-1ece46's capture and evidence tables; official Hooks common-input documentation at https://learn.chatgpt.com/docs/hooks (checked 2026-10-06: subagent hooks use parent session_id; SubagentStart/SubagentStop include agent_id and agent_type; SessionEnd and Interrupt exclude subagents); exact Codex 0.160.1 source for command environment construction and hook dispatch. The current environment-variable documentation does not describe these two native variables. Compare work-claims spec Identity and relay's inherited-environment constraints.

Bound: trace both environment assignments, the SubagentStart/SubagentStop agent_id construction and the lifecycle event dispatch at Codex 0.160.1, using official docs and source. Establish whether agent_id equals CODEX_THREAD_ID so a subagent hook can select its worker claim despite the parent session_id. Verify the review finding that workers emit SubagentStop rather than Stop, and do not receive SessionEnd or Interrupt; do not reuse an injected child Stop with a child session_id as a lifecycle test. Cite immutable source locations and distinguish intended contract from observed inheritance. Compare with current docs only to identify version differences. No live harness sweep, native-ID overrides, product-code or host-config changes. If exact-version semantics are unavailable, state the gap explicitly.

Expected result: a concise identifier/parent/agent_id mapping, emitted-event table and evidence-backed recommendation: tasks contract adaptation if the difference is deliberate, upstream repair only if Codex's own contract is violated. Document whether hook agent_id can address a thread-keyed worker claim. The documented parent hook ID and captured parent CODEX_SESSION_ID favor thread-as-worker/session-as-parent semantics, subject to the source check; do not approve a new schema here. Update the brief, tasks-5e2472 and tasks-56b450; keep controller aggregation and authorization separate.

Ideas it wakes: on completion add the finding to tasks-5745bb, tasks-616cd7, tasks-cf8687, tasks-290fa3, tasks-6e2d15 and tasks-f537a8 with tasks note in the same commit. These remain one remediation cluster rather than six independent scoping jobs.

## Notes

- 2026-10-06T10:41:53Z (main): concerns: tasks-1ece46 extension — Verify Codex intended variable and hook semantics before treating captured disagreement as an upstream delivery defect.
- 2026-10-06T10:59:44Z (main): Review follow-up: parent hook session_id is already documented. Narrow the open hook identity question to whether SubagentStart/SubagentStop agent_id equals CODEX_THREAD_ID, and trace the captured release event dispatch. The prior child Stop input was synthetic, not a real subagent event. Reviewer identity was not supplied; probe/review attribution is to the reviewer, not assumed human.
- 2026-10-06T16:50:03Z (main): priority: raised to P1 under the adopted agent-identity lane; identity research precedes dependent design, and controller-owned closure stays a separate decision.
- 2026-10-06T16:59:40Z (main): started
  provenance: {"harness_session":"codex:01a11226-e23f-7c60-a6c3-9e51bdcfdd97","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-06T16:59:49Z (main): Direct exact-release source/documentation research in .worktrees/tasks-c0bad3; no harness sweep, native-ID override, product code, or host configuration changes. Trial arm: not enrolled.
- 2026-10-06T17:00:09Z (investigate/tasks-c0bad3): resumed
  provenance: {"harness_session":"codex:01a11226-e23f-7c60-a6c3-9e51bdcfdd97","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-06T17:05:31Z (investigate/tasks-c0bad3): Exact Codex 0.160.1 source at d27764b82f7118f674371e6d6e76271d9d606edb deliberately writes shared root CODEX_SESSION_ID and concrete CODEX_THREAD_ID; SubagentStart/SubagentStop agent_id equals worker thread ID. Worker completion dispatches SubagentStop, not Stop; worker SessionEnd/Interrupt are excluded. Recommend tasks contract adaptation, not upstream export repair. Root grouping is not an immediate-parent link for nested workers. Findings and immutable source links: docs/notes/2026-09-30-delegated-identity-brief.md; live hook delivery remains unverified.
- 2026-10-06T17:06:35Z (investigate/tasks-c0bad3): Verification: 25 exact-release source files match their Git tree blob IDs; all 15 cited source ranges and Markdown reference links validated. just test-one codex (6 passed), just test-one provenance (11 passed), just test-fast and just check passed. No runtime code changed. Worktree resume warning filed as tasks-ef69c0.
- 2026-10-06T17:08:37Z (investigate/tasks-c0bad3): review: impl round 1 — verdict: accept; findings: none; reviewer: codex (model unspecified)
- 2026-10-06T17:08:37Z (investigate/tasks-c0bad3): Review exclusions accepted: actual hook/relay delivery belongs to follow-up verification; schema, aggregation and closure policy remain separate reviewed tasks; other releases, ordinary forks and configuration overrides are outside the captured worker scope; historical capture is preserved, not reproduced.
- 2026-10-06T17:08:37Z (investigate/tasks-c0bad3): done
  provenance: {"harness_session":"codex:01a11226-e23f-7c60-a6c3-9e51bdcfdd97","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-06T17:08:37Z (investigate/tasks-c0bad3): Verified Codex 0.160.1 source: commands deliberately use shared root-session and concrete worker-thread IDs; SubagentStart/SubagentStop agent_id equals worker thread ID, worker completion uses SubagentStop, and workers exclude SessionEnd/Interrupt. Recommend tasks adaptation; brief and eight related records updated. Source blob/range checks, focused tests, fast suite, repository checks and review pass; live delivery remains follow-up scope.
  provenance: {"harness_session":"codex:01a11226-e23f-7c60-a6c3-9e51bdcfdd97","harness_session_source":"CODEX_SESSION_ID"}
