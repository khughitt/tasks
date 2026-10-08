---
id: tasks-f8c9cc
title: Decide closure responsibility for controller-owned tasks
status: done
priority: 1
size: s
complexity: mid
process: direct
owner: docs/controller-closure
created: 2026-10-06T10:41:53Z
updated: 2026-10-08T14:47:13Z
started: 2026-10-08T14:43:11Z
completed: 2026-10-08T14:47:11Z
depends: []
parent: tasks-2fa8c6
tags: []
source: tasks-1ece46
agent: codex
---

Question: Should workers close a task claimed by their controller, or should the controller retain lifecycle closure?

Where to start: docs/notes/2026-09-30-delegated-identity-brief.md; tasks-1ece46's captured claimed refusal; tasks-f27fe3 and tasks-673b21; src/commands/mod.rs::ownership/claim_guard and the controller/worker workflow instructions.

Bound: one small policy decision using the existing evidence. Recommend keeping controller-closes: workers report completion and the controller closes its own claim. Record the alternative of explicit delegated authority and reject it unless an evidenced need justifies a separately planned authorization feature. Do not add identity precedence, relay representation, provenance, Stop aggregation, a takeover workaround or a product-code change to this task.

Expected result: record the closure policy and its rationale on this task and in the brief, identify any necessary workflow-owner follow-up, and give both reports an explicit disposition. Existing foreign-live-claim refusal remains required even after identity repair.

Ideas it wakes: on completion add the decision to tasks-f27fe3 and tasks-673b21 with tasks note in the same commit, update the brief, and close or propose the reports together according to the recorded policy.

## Notes

- 2026-10-06T16:50:04Z (main): priority: raised to P1 under the adopted agent-identity lane; identity research precedes dependent design, and controller-owned closure stays a separate decision.
- 2026-10-08T14:43:11Z (main): started
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T14:43:33Z (main): Scope confirmed: record controller-closes policy in the delegated-identity brief and both already-dropped reports; existing worker-handoff guidance and foreign-live-claim refusal implement the rule, so no authorization code or workflow-owner follow-up is needed.
- 2026-10-08T14:44:03Z (docs/controller-closure): resumed
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T14:45:21Z (docs/controller-closure): Decision: controller closes its own live claim after inspecting worker results and verification; worker-owned tasks use the existing done-or-park handoff. Reject explicit delegated authority absent a separately scoped need. Foreign-live refusal remains required; existing Worker handoff guidance is sufficient, so no workflow-owner follow-up or product-code change is needed.
- 2026-10-08T14:47:11Z (docs/controller-closure): review: impl round 1 — verdict: accept; findings: none; reviewer: codex
- 2026-10-08T14:47:11Z (docs/controller-closure): Ruling: accept final review; namespace support, worker registration, Stop aggregation and new authorization features remain separately scoped; fresh hook delivery is not claimed. No minor (deferred) findings.
- 2026-10-08T14:47:11Z (docs/controller-closure): done
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-08T14:47:11Z (docs/controller-closure): Recorded controller-closes policy and rejected delegated authority absent separate need; updated the brief and both dropped-report dispositions. Existing workflow guidance needs no follow-up. Focused foreign-claim and delegated-handoff checks passed; just test-fast passed (1159 tests, one ignored); final review accepted with no findings.
  provenance: {"harness_session":"codex:01a11bed-fe69-7060-9c93-813a4949349b","harness_session_source":"CODEX_THREAD_ID"}
