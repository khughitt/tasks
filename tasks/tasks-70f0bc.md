---
id: tasks-70f0bc
title: Explain registered-record publication for cross-project dependencies
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-04T19:25:06Z
updated: 2026-10-06T11:25:35Z
depends: []
parent: tasks-984769
tags: [feedback, friction, "from:lore"]
agent: codex
---

Original report: Cross-project dependency writes reject a task that exists only in an active worktree; the unresolvable_id error does not explain how to publish a planning dependency.
Related report tasks-562c53: Cross-project dependencies read an older registered task record after a preparation task closes in its worktree, blocking the next native step until task records are integrated separately from runtime activation.

Why: tasks-d0d520 established that explicit record publication to registered authority suffices, but existing diagnostics and workflow guidance do not explain why direct show can locate an unpublished task while dependency writes cannot, or when a worktree completion becomes visible to another project.

Done: Clarify foreign-dependency diagnostics and tasks-owned workflow guidance: publish the upstream task record into its registered checkout before linking and publish its completion before expecting downstream readiness; direct show claim/park fallback does not publish records. Explain record-only Git integration without runtime activation, preserving newer/divergent registered histories and coordinating upstream owners. Publish necessary linked artifacts when used. Distinguish genuinely missing/unregistered targets from an unpublished worktree task; do not imply every unreachable ID exists in a worktree. Preserve current resolver authority, cycle checks, ownership safeguards and JSON shape. No runtime/host-pointer change or new resolver. Tasks owns CLI/skill guidance; each upstream project owns publication and its rollout gates.

Where: src/commands/dep.rs::add_dependencies, src/resolve.rs::resolve_task/resolve_registered, src/commands/show.rs::recorded_elsewhere; README.md and skills/tasks/SKILL.md cross-project/worktree guidance; existing record-home and multi-project specs. Evidence: tasks-d0d520 attachments and docs/notes/2026-10-06-record-continuity-brief.md.

Verification: Retain or adapt the attached isolated two-project reproduction: before/after publication, missing-record show fallback versus dependency rejection, worktree-only completion versus registered readiness, and record-only integration retaining the runtime sentinel. Keep failed writes, cycles, foreign closure and missing/config-invalid authority guarded. Use just test-one filters and just test-fast before committing code.

## Notes

- 2026-10-06T09:45:47Z (main): scope: briefed; grouped record visibility and recovery evidence under tasks-984769; bounded reproductions precede a routing or recovery change; brief: docs/notes/2026-10-06-record-continuity-brief.md
- 2026-10-06T11:25:33Z (docs/tasks-d0d520): Finding from tasks-d0d520: a visible claim lets direct show locate a missing worktree-only record, but dependency writes/readiness use registered records. Publishing the open record allows linking; publishing its worktree completion enables readiness without activating the runtime sentinel. Published cycles, foreign closure and unreachable/config failures remain guarded.
- 2026-10-06T11:25:33Z (docs/tasks-d0d520): scope: scoped; tasks-d0d520 established explicit record publication suffices; diagnostic and workflow guidance owns creation/completion visibility without a resolver change; brief: docs/notes/2026-10-06-record-continuity-brief.md
