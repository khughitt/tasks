---
id: tasks-7ad96f
title: "start --force --reason run from a worktree writes the halt-override note into the registered checkout's working copy of the halt; when the halt itself is being worked in that worktree, the next write there refuses as stale_copy (and a same-second tie cannot be resolved by merging), so the override blocks the halt's own session"
status: idea
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-02T10:35:27Z
updated: 2026-10-02T14:44:09Z
depends: []
parent: tasks-0e7216
tags: [feedback, friction, "from:sci"]
agent: claude-code/claude-opus-5-5
---

Why: start writes halt-override audit notes through snapshot.authority().write_task rather than the shared load/refuse_stale_copy path. The authority is the registered checkout even when the halt is actively worked in another checkout. Code confirms this deliberate bypass; the reported active-worktree failure and same-stamp fork were not reproduced in this scope pass.

Decision to settle: Reconcile registered-checkout halt decisions and durable attempted-override notes with the rule that writes never fork or overwrite a newer task copy. Keep incident visibility and audit-before-start ordering. Choose the audit write location/refusal policy explicitly, including a halt held by the caller or another live session, stale/equal-stamp copies, an authority-only halt, multiple blockers, and failure after an attempted note. Merely bumping timestamps, weakening stale_copy, or advising the halt owner to reset its copy does not settle ownership.

Where to look: src/commands/status.rs::start, src/halt.rs::snapshot, src/commands/mod.rs::load/refuse_stale_copy/save, src/stale.rs, src/repo.rs::write_task; docs/specs/2026-09-30-halt-enforcement-design.md and docs/specs/2026-09-30-record-home-design.md; existing halt override and record-home CLI tests; docs/notes/2026-10-02-worktree-audits-bootstrap-brief.md.

Original report: start --force --reason run from a worktree writes the halt-override note into the registered checkout's working copy of the halt; when the halt itself is being worked in that worktree, the next write there refuses as stale_copy (and a same-second tie cannot be resolved by merging), so the override blocks the halt's own session

## Notes

- 2026-10-02T14:44:07Z (main): scope: briefed; authority audit writer bypass is confirmed but ownership/refusal contract needs design; waits on tasks-10c968; brief: docs/notes/2026-10-02-worktree-audits-bootstrap-brief.md
