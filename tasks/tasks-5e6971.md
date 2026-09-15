---
id: tasks-5e6971
title: "Worktree setup only knows 'just setup'; a project whose commit hooks need installed dependencies has no declared setup step, so the first commit in a fresh worktree hangs on an interactive install prompt"
status: idea
priority: 2
created: 2026-09-15T15:42:44Z
updated: 2026-09-15T21:02:02Z
depends: []
parent: tasks-02769c
tags: [feedback, gap, "from:rad"]
agent: claude-code/claude-opus-5
---

Followed the skill's worktree protocol: git worktree add, then 'just setup' if defined. The project defines no justfile, so nothing ran. Its commit-msg hook shells out to a package runner that, finding no installed dependencies in the new worktree, prompted 'Ok to proceed?' on a stdin nobody was reading; the commit sat for 10+ minutes until I killed it. Expected: a per-project setup command declared in tasks/.config.toml (or the skill telling me to look for the project's own install step when no justfile exists), so a fresh worktree is hydrated before the first commit rather than discovered broken by it.

## Notes

- 2026-09-15T21:02:02Z (main): scope: briefed; grouped under tasks-02769c pending the bootstrap-contract research; brief: docs/notes/2026-09-15-fresh-worktree-brief.md
