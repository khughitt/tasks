---
id: tasks-5e6971
title: Use a root guide’s explicit setup command when Just is unavailable
status: done
priority: 2
size: xs
complexity: low
process: direct
owner: 63742b-bootstrap
created: 2026-09-15T15:42:44Z
updated: 2026-09-15T21:07:23Z
started: 2026-09-15T21:05:57Z
completed: 2026-09-15T21:07:23Z
depends: []
parent: tasks-02769c
tags: [feedback, gap, "from:rad"]
agent: claude-code/claude-opus-5
---

Why: a fresh worktree may lack dependencies required by hooks, and just setup is not universal.\nDone: the task workflow runs just setup when defined, otherwise follows an explicit root setup command and never guesses an installer.\nWhere to look: AGENTS.md and skills/tasks/SKILL.md.

## Notes

- 2026-09-15T21:02:02Z (main): scope: briefed; grouped under tasks-02769c pending the bootstrap-contract research; brief: docs/notes/2026-09-15-fresh-worktree-brief.md
- 2026-09-15T21:05:12Z (63742b-bootstrap): finding: tasks-63742b recommends just setup when available, otherwise the root guide’s explicit setup command; never infer an installer.
- 2026-09-15T21:05:57Z (63742b-bootstrap): scope: scoped; direct documentation change established by tasks-63742b; brief: docs/notes/2026-09-15-fresh-worktree-brief.md
- 2026-09-15T21:07:23Z (63742b-bootstrap): Documented the explicit root-guide setup fallback when Just is unavailable.
