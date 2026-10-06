---
id: tasks-325e79
title: Establish a safe plan-linking workflow before review
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-06T16:52:01Z
updated: 2026-10-06T16:52:01Z
depends: []
parent: tasks-46d207
tags: []
source: tasks-b2e718
agent: codex
---

Question: Can a draft implementation plan be linked for review without offering its unreviewed steps as executable work, using the current warning and picker contracts?

Where to start: tasks-b2e718; the linked-plan heading loop in src/commands/check.rs, readiness in src/commands/list.rs and next, skills/tasks/SKILL.md and the writing-plans workflow, and the hierarchy/plan-link contracts. Code inspection confirms unlinked_step is emitted per uncovered plan heading; establish severity and exit behavior with the actual CLI rather than treating a warning as a failed gate.

Bound: one disposable git project with isolated XDG_CONFIG_HOME and XDG_STATE_HOME; compare an unlinked draft plan, a linked draft with uncovered headings, and linked todo step records while the parent is parked for human review. Capture check's exit code and warnings plus ready, next and prime eligibility. Compare the existing safe sequence of reviewing the written plan before filing executable children; test other existing holds only when needed to establish a supported alternative. No product-code change, new approval field, or automated review decision.

Expected result: an evidence-backed before-review/after-review workflow that preserves document review and task-plan drift checks. Prefer clarifying the existing sequence or diagnostic over adding review state. If current contracts cannot express a safe workflow, state the precise gap and file a planned follow-up with acceptance cases. Update tasks-b2e718 with the finding and identify the workflow owner; the guidance lane can incorporate a settled instruction without blocking its independent documentation tasks.
