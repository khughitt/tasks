---
id: tasks-91c622
title: Design cross-project scoping from the review brief
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-02T14:29:35Z
updated: 2026-10-14T13:29:49Z
depends: []
parent: tasks-ffdbaa
tags: []
agent: codex
---

Why: tasks-c591da requests registry-wide discovery; tasks-4e8cef exposes the missing mixed-project write and handoff contract. These are one workflow, not two independent skill rewrites.
Where to start: docs/notes/2026-10-02-cross-project-review-brief.md; skills/scope/SKILL.md, docs/specs/2026-09-12-scope-pass-design.md section 4, docs/notes/2026-09-13-scope-skill-validation.md, docs/specs/2026-09-04-multi-project-design.md, src/scope.rs. The captured source body is available; the mindful thought lookup returned no match.
Bound: Define --all-projects discovery and explicit mixed IDs, bounded eligibility/clustering and tie-breaks, one fixed root per project, deterministic handoff placement and portable references, supported cross-project goal associations, per-member revalidation, partial-write reporting, and reuse on rerun. Preserve existing local/worktree and --project behavior. Propose acceptance packets covering two projects, a local worktree whose registered main differs, an already-parented idea, a stale/claimed member, and a rerun without duplicate artifacts. Compare a simple explicit workflow with a separate discovery service; prefer the workflow. Project groups, semantic indexes, and new background services are outside this task.
Done: Obtain user review of the written spec, then user review of the implementation plan before changing the shipped skill. Plain CLI listing can support the design, so neither tasks-af97da nor project-group design is a prerequisite.
Ideas it wakes: Record the approved finding on tasks-c591da and tasks-4e8cef and update the brief before the result's branch merges, then re-scope those ideas against the accepted contract.

## Notes

- 2026-10-11T17:16:47Z (design/tasks-a9b8d3-review-state): Review-state implementation tasks-b8f3c9 is reviewed; when scoping this design, use list --status idea --needs-review eligibility and atomic note --reviewed/--no-reviewed audit/wake commands. Live activation is still awaiting coordinated rollout. Reuse this existing design task rather than file a duplicate.
- 2026-10-12T19:18:37Z (main): Review-state rollout tasks-b8f3c9 is now complete on both reading hosts; this supersedes the earlier activation-pending note. Use list --status idea --needs-review for eligibility, note --reviewed for scope decisions, and note --no-reviewed for findings that readmit an idea. Evidence: tasks/files/tasks-b8f3c9/review-state-rollout.json. No separate cross-project review-state task is needed.
- 2026-10-13T09:55:52Z (main): Design input from scope pass 2026-10-13: the user ruled that a scope pass may move a misfiled record into its owning project without approval (tasks-19d4fe adds a refile verdict via tasks move); the cross-project contract should allow that one write outside the fixed root.
- 2026-10-14T13:29:49Z (main): Eligibility update 2026-10-14: scope's default selection now reads tasks list --needs-review (unreviewed ideas first, then process-less todo/blocked rows as re-scopes), superseding the --status idea --needs-review rule in the 2026-10-11/12 notes; see skills/scope/SKILL.md §1 and the review-state spec's 2026-10-14 amendment (tasks-3333bc).
