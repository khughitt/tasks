---
id: tasks-cee8d9
title: Establish a compliant bootstrap path when task-record commits fail
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-02T14:44:07Z
updated: 2026-10-02T14:44:08Z
depends: []
parent: tasks-0e7216
tags: []
agent: codex
---

Question: When a pre-existing content gate blocks a task-record-only commit, is there an existing compliant route to an isolated repair, or which exact workflow exception and owner change are required?

Where to start: docs/notes/2026-10-02-worktree-audits-bootstrap-brief.md; skills/tasks/SKILL.md Process and workspace, AGENTS.md, .githooks/pre-commit, justfile docs_paths/docs_check_cmd, tools/ops-check, docs/notes/2026-09-15-fresh-worktree-brief.md. The original gate output is unavailable, so distinguish a synthetic controlled failure from the reported incident.

Bound: Trace the local task-only classifier and retained checks; demonstrate one content-gate refusal and one candidate recovery in a disposable Git checkout with isolated tasks registry/state. Use a single CLI-created task record and retain exact bytes and notes, relevant staged patch, and unrelated working changes. Do not disable hooks, modify product code or host launchers, write live task files by hand, or execute a real-work bootstrap exception before it is authorized. If no compliant recovery exists, stop after the controlled finding and identify the smallest proposed instruction change and its owner; do not invent permission.

Expected result: A short command transcript and recommendation distinguishing hook selection from content failure, giving exact gates and record/claim handoff guarantees. Recommend existing instructions when sufficient; otherwise propose the smallest reviewed exception spanning the shipped workflow and its global/hook owners. Update the brief; downstream owner work is filed only after the finding establishes what is needed.

Ideas it wakes: On completion, add the finding to tasks-850488 with tasks note and update the brief in the same commit, then re-scope it.

## Notes

- 2026-10-02T14:44:07Z (main): concerns: tasks-5a46b9 extension — add an evidence-backed isolated-repair bootstrap path for a task-record commit blocked by pre-existing content checks
