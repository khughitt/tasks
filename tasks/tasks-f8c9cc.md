---
id: tasks-f8c9cc
title: Decide closure responsibility for controller-owned tasks
status: todo
priority: 1
size: s
complexity: mid
process: direct
created: 2026-10-06T10:41:53Z
updated: 2026-10-06T16:50:05Z
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
