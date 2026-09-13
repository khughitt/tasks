---
id: tasks-0d50ff
title: "scope skill: cluster, gather context, verdict per idea, brief and research tasks"
status: done
priority: 2
size: m
complexity: mid
owner: scope
created: 2026-09-13T01:42:27Z
updated: 2026-09-13T11:13:15Z
started: 2026-09-13T09:58:48Z
completed: 2026-09-13T11:13:15Z
depends: [tasks-470e8c]
parent: tasks-019c60
tags: [skill]
spec: docs/specs/2026-09-12-scope-pass-design.md
plan: docs/plans/2026-09-13-scope-skill.md
---

skills/scope/SKILL.md plus the tasks skill and AGENTS.md pointers. Picks one cluster of 3-5 related ideas by default (explicit ids override), gathers evidence per idea, and writes exactly one verdict each: scoped, briefed, shelved, question, or a drop proposal. A cluster whose decisions remain gets a brief under docs/notes and only the research or design tasks needed to settle them; ideas stay ideas until design review. Reruns update the existing goal and brief.

## Notes

- 2026-09-13T10:03:52Z (scope): Implementation plan drafted: two steps for tested skill/discovery and isolated real Prism acceptance with rerun evidence; plan review precedes implementation.
- 2026-09-13T10:03:52Z (scope): parked (waiting on user, review): Review docs/plans/2026-09-13-scope-skill.md, then execute its two steps with subagent-driven development.
- 2026-09-13T10:44:32Z (scope): Plan approved with three changes: three baseline/green trials, real scratch CLI verdict packet, five-field spec correction in Task 1; execution started.
- 2026-09-13T11:12:44Z (scope): Real acceptance: Prism .worktrees/scope-acceptance at 6887eae9d02078914067003b5e23d4326d3f2bbc; b8b589,920f31,ad2b12 briefed; 8a8eac,49a068 shelved pending bf3ae9 reset and 46035b autosave (2026-11-10 review retained). One brief docs/notes/2026-09-13-profile-editing-brief.md, goal prism-3415ef, design prism-e37618; no research or spec needed. Exact user summary and main-relative paths: docs/notes/2026-09-13-scope-skill-validation.md. Default exclusion and zero-write explicit rerun verified; 161 main files unchanged; Prism gate 327 Node plus Lua, zero check warnings. Missing Mindful source recorded; no skill correction demonstrated.
- 2026-09-13T11:13:15Z (scope): Shipped tested scope skill and discovery docs with real Prism acceptance at 6887eae; validation records exact summary, isolated roots, preserved relationships, and idempotent rerun. No acceptance wording correction needed.
