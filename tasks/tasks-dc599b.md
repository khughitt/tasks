---
id: tasks-dc599b
title: Encourage recording the creating coding agent and model on every agent-created task
status: doing
priority: 2
process: planned
owner: feat/tasks-dc599b-provenance
created: 2026-09-13T18:19:44Z
updated: 2026-09-13T19:46:03Z
started: 2026-09-13T19:46:03Z
depends: []
tags: []
spec: docs/specs/2026-09-13-creation-provenance-design.md
---

Knowing which coding harness and model created a task provides useful context when interpreting its assumptions and recommendations. The user reports that tasks-61cc5c was created with Crush and Kimi-K3, but neither appears in its task metadata.

Explore encouraging agents to include their harness and model whenever they create a task, covering ordinary tasks add, feedback reports, and quick-add workflows. Prefer optional creation provenance with clear agent instructions; avoid making unknown identity block task capture or guessing missing values. Decide the smallest suitable representation during scoping. Preserve creation attribution separately from the existing model field, which records the model responsible for the latest completion.

Captured by Codex (GPT-6) from this user request.

## Notes

- 2026-09-13T19:46:03Z (feat/tasks-dc599b-provenance): scope: process planned. Evidence: TASKS_MODEL is exported by no harness config (ops hooks, ai settings, codex config) and no task in any project carries model:, so the tasks-222dab completion stamp is unwired; a creation field on the same mechanism needs the harness side in scope. Design questions: one opaque agent string vs harness+model fields; env var vs add flag; who wires each harness.
- 2026-09-13T19:46:03Z (feat/tasks-dc599b-provenance): parked (waiting on user, review): Review .worktrees/tasks-dc599b-provenance/docs/specs/2026-09-13-creation-provenance-design.md; on approval write the implementation plan (tasks piece here, ops hook piece, ai codex piece) for separate review
