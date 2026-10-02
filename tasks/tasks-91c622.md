---
id: tasks-91c622
title: Design cross-project scoping from the review brief
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-02T14:29:35Z
updated: 2026-10-02T14:29:35Z
depends: []
parent: tasks-ffdbaa
tags: []
agent: codex
---

Why: tasks-c591da requests registry-wide discovery; tasks-4e8cef exposes the missing mixed-project write and handoff contract. These are one workflow, not two independent skill rewrites.
Where to start: docs/notes/2026-10-02-cross-project-review-brief.md; skills/scope/SKILL.md, docs/specs/2026-09-12-scope-pass-design.md section 4, docs/notes/2026-09-13-scope-skill-validation.md, docs/specs/2026-09-04-multi-project-design.md, src/scope.rs. The captured source body is available; the mindful thought lookup returned no match.
Bound: Define --all-projects discovery and explicit mixed IDs, bounded eligibility/clustering and tie-breaks, one fixed root per project, deterministic handoff placement and portable references, supported cross-project goal associations, per-member revalidation, partial-write reporting, and reuse on rerun. Preserve existing local/worktree and --project behavior. Propose acceptance packets covering two projects, a local worktree whose registered main differs, an already-parented idea, a stale/claimed member, and a rerun without duplicate artifacts. Compare a simple explicit workflow with a separate discovery service; prefer the workflow. Project groups, semantic indexes, and new background services are outside this task.
Done: Obtain user review of the written spec, then user review of the implementation plan before changing the shipped skill. Plain CLI listing can support the design, so neither tasks-af97da nor project-group design is a prerequisite.
Ideas it wakes: Record the approved finding on tasks-c591da and tasks-4e8cef and update the brief in the same commit, then re-scope those ideas against the accepted contract.
