---
id: tasks-dc599b
title: Encourage recording the creating coding agent and model on every agent-created task
status: doing
priority: 2
process: planned
owner: feat/tasks-dc599b-provenance
created: 2026-09-13T18:19:44Z
updated: 2026-09-13T20:50:32Z
started: 2026-09-13T19:46:03Z
depends: [ops-a405cc, ai-d5a56c]
tags: []
spec: docs/specs/2026-09-13-creation-provenance-design.md
plan: docs/plans/2026-09-13-creation-provenance.md
---

Knowing which coding harness and model created a task provides useful context when interpreting its assumptions and recommendations. The user reports that tasks-61cc5c was created with Crush and Kimi-K3, but neither appears in its task metadata.

Explore encouraging agents to include their harness and model whenever they create a task, covering ordinary tasks add, feedback reports, and quick-add workflows. Prefer optional creation provenance with clear agent instructions; avoid making unknown identity block task capture or guessing missing values. Decide the smallest suitable representation during scoping. Preserve creation attribution separately from the existing model field, which records the model responsible for the latest completion.

Captured by Codex (GPT-6) from this user request.

## Notes

- 2026-09-13T19:46:03Z (feat/tasks-dc599b-provenance): scope: process planned. Evidence: TASKS_MODEL is exported by no harness config (ops hooks, ai settings, codex config) and no task in any project carries model:, so the tasks-222dab completion stamp is unwired; a creation field on the same mechanism needs the harness side in scope. Design questions: one opaque agent string vs harness+model fields; env var vs add flag; who wires each harness.
- 2026-09-13T19:46:03Z (feat/tasks-dc599b-provenance): parked (waiting on user, review): Review .worktrees/tasks-dc599b-provenance/docs/specs/2026-09-13-creation-provenance-design.md; on approval write the implementation plan (tasks piece here, ops hook piece, ai codex piece) for separate review
- 2026-09-13T20:06:33Z (feat/tasks-dc599b-provenance): Spec review applied: Claude stamp resolves a scratchpad state file at command time (SessionStart exports, PostModelSwitch rewrites; live /model transition is the acceptance check); Codex harness-only since the effective model is unknowable from config (Codex 0.154.0 has hooks but no model field); --agent resolves before the env is read; feedback fallback is TASKS_AGENT on that invocation.
- 2026-09-13T20:06:33Z (feat/tasks-dc599b-provenance): parked (waiting on user, review): Re-review .worktrees/tasks-dc599b-provenance/docs/specs/2026-09-13-creation-provenance-design.md (revision after review); on approval write the implementation plan for separate review
- 2026-09-13T20:15:26Z (feat/tasks-dc599b-provenance): Second review applied: unknown-model paths export TASKS_MODEL= and remove the state file; live check is add-under-A, switch, add-under-B, done-under-B; Claude <model> is documented as the enclosing session's model (subagent-specific attribution deferred). Spec approved for planning.
- 2026-09-13T20:19:12Z (feat/tasks-dc599b-provenance): Plan written: Task 1 CLI field (flag-first resolver shared by add and feedback), Task 2 skill/README/base-design docs; Pieces A (ops claude-provenance hook, both events, one script) and B (ai hook registration, codex harness-only, live /model acceptance) to be filed in ops/ai on plan approval and wired with tasks dep. Spec hook file names aligned to the single script.
- 2026-09-13T20:19:12Z (feat/tasks-dc599b-provenance): parked (waiting on user, review): Review .worktrees/tasks-dc599b-provenance/docs/plans/2026-09-13-creation-provenance.md; on approval add the two step children with --process direct, file Pieces A/B in ops and ai, dep the goal on them, then execute Task 1
- 2026-09-13T20:50:32Z (feat/tasks-dc599b-provenance): Plan approved with fixes (9b4b726). Steps: tasks-74a629 (Task 1), tasks-ca6718 (Task 2). Pieces: ops-a405cc (hook), ai-d5a56c (registration, codex, live acceptance); goal depends on both.
- 2026-09-13T20:50:32Z (feat/tasks-dc599b-provenance): parked (waiting on agent): Goal waits on tasks-74a629, tasks-ca6718, ops-a405cc, ai-d5a56c; closeout after the live acceptance on ai-d5a56c
