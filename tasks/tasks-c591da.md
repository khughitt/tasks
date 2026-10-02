---
id: tasks-c591da
title: "scope: an all-projects pass that clusters ideas across projects"
status: idea
priority: 2
size: m
complexity: high
process: planned
created: 2026-09-30T14:12:35Z
updated: 2026-10-02T14:29:37Z
depends: []
parent: tasks-ffdbaa
tags: [quick-add]
source: "mindful:thought:c21c822e191c2e6c5939f9caa1f05a39"
agent: claude-code/claude-opus-5-5
---

Why: Related ideas in different registered projects cannot currently be reviewed as one default scope batch. The CLI can already list ideas across the registry; the scope skill deliberately fixes one root before member reads.

Decision to settle: Define explicit registry-wide discovery and mixed-project scoping with a fixed checkout per member, a bounded cluster, one deterministic handoff destination, preserved existing parents/sources, and per-member revalidation. Prefer extending the existing scope workflow over creating a new clustering service. Decide how topical connections count when tags or sources differ, and how reruns find the existing handoff.

Where to look: skills/scope/SKILL.md, docs/specs/2026-09-12-scope-pass-design.md section 4, docs/notes/2026-09-13-scope-skill-validation.md, src/scope.rs, and docs/notes/2026-10-02-cross-project-review-brief.md. Related tasks-77dbc6/tasks-ece1e2 design project groups; use current --all-projects without making groups a prerequisite. tasks-6d33e6 already delivered registry-wide ready/prime.

Source availability: mindful --json show c21c822e191c2e6c5939f9caa1f05a39 returned no matching thought during this pass. The captured report below and its source reference are preserved; unavailable source content is unknown.

Original report:
Today scoping means opening a terminal and agent per project. Give /scope an all-projects mode that samples ideas across the registry and clusters ones that connect, directly (shared signals or messages) or by concept or technology (e.g. three.js work, reducing friction), so a mixed group can be scoped together. Related: tasks-77dbc6 (project groups), tasks-6d33e6 (ready/prime --all-projects).

## Notes

- 2026-10-02T14:29:35Z (main): scope: briefed; registry-wide discovery needs an explicit multi-root scope contract; captured mindful source retained but lookup returned no match; waits on tasks-91c622; brief: docs/notes/2026-10-02-cross-project-review-brief.md
