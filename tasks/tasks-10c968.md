---
id: tasks-10c968
title: Design halt audit writes that preserve active task ownership
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-02T14:44:07Z
updated: 2026-10-02T14:44:08Z
depends: []
parent: tasks-0e7216
tags: []
agent: codex
---

Why: tasks-7ad96f reports an override that blocks the halt owner because audit notes are written into a sibling copy. The writer bypass is confirmed in src/commands/status.rs::start.
Where to start: docs/notes/2026-10-02-worktree-audits-bootstrap-brief.md; src/halt.rs::snapshot and authority, src/commands/status.rs::start, src/commands/mod.rs::load/refuse_stale_copy/save, src/stale.rs, src/repo.rs::write_task; the halt-enforcement and record-home specs and existing CLI fixtures.
Bound: Compare keeping audit writes at registered authority with explicit preflight/refusal, versus separating authority reads from an explicitly selected current record write destination. Settle active claims, local versus foreign holder, newer/equal-stamp siblings, authority-only halts, multiple blockers, attempted notes on failure, and no unaudited claimed start. Keep the registered checkout authoritative for halt decisions and preserve the no-silent-write-routing rule unless a reviewed contract deliberately changes it. Avoid a general event store or weakening stale-copy checks. Include a scratch-worktree acceptance case where a halt has newer notes/status in its active tree and the override must not fork or overwrite those notes.
Done: User-reviewed spec then user-reviewed implementation plan, with concrete before/after examples and failure checks. Name any task/hook owner change and update superseded doc status when the final work lands.
Ideas it wakes: On design completion, record the approved finding on tasks-7ad96f and update the brief in the same commit, then re-scope that idea.

## Notes

- 2026-10-02T14:44:07Z (main): concerns: tasks-c543ae defect — halt override audit writes bypass sibling freshness checks and can fork the actively worked halt record
