---
id: tasks-74a629
title: Record and expose the agent stamp
status: done
priority: 2
size: m
complexity: low
process: direct
owner: feat/tasks-dc599b-provenance
created: 2026-09-13T20:50:23Z
updated: 2026-09-13T20:52:40Z
started: 2026-09-13T20:50:33Z
completed: 2026-09-13T20:52:40Z
depends: []
parent: tasks-dc599b
tags: []
spec: docs/specs/2026-09-13-creation-provenance-design.md
plan: docs/plans/2026-09-13-creation-provenance.md
step: "Task 1: Record and expose the agent stamp"
---

Task 1 of docs/plans/2026-09-13-creation-provenance.md: agent field on the record and codec, --agent/--no-agent flags, creation_agent resolver (flag first, then TASKS_AGENT) shared by add and feedback via blank(), JSON projections, test-builder scrub, five integration tests and one format unit test. Gate and reinstall.

## Notes

- 2026-09-13T20:52:40Z (feat/tasks-dc599b-provenance): Optional agent field: record, codec (after model), --agent on add/edit and --no-agent, creation_agent resolver (flag validated first, then TASKS_AGENT via the provenance_var reader shared with TASKS_MODEL) passed into blank() so add and feedback share it; agent in Task, TaskSummary, ParkedRow JSON; builders scrub TASKS_AGENT. Five integration tests and one format unit test red then green; just gate 172 unit + 294 CLI; installed.
