---
id: tasks-4b68c8
title: "tasks attach fails outside a checkout although tasks show resolves the same id from anywhere, and its error suggests removing an unrelated empty .git directory"
status: idea
priority: 2
created: 2026-10-23T17:52:50Z
updated: 2026-10-24T11:32:18Z
depends: []
parent: tasks-b34076
tags: [feedback, friction, "from:hq"]
agent: claude-code
---

Action: tasks attach <id> <file> run from a scratch directory off the synced tree (not a git repository), with a valid id. Got an io error from git rev-parse in the current directory, which named the nearest empty .git directory above it and suggested rmdir on it. Expected: attach resolves the task's project from the id, as tasks show and tasks note do from any directory, and reads the file by its path. Workaround: run attach from the project checkout with absolute file paths. Constraint: per-task scratch lives off the synced tree by rule, so attaching from it is the common case at task close.

Scoping evidence at 89e0371: show and attach both route through open_id_ctx_unwarned in src/commands/mod.rs; attach and note both use open_id_write_ctx. Registry fallback occurs only for NoProject. src/link.rs::GitTree::of deliberately returns an IO error for a broken ancestor .git; obstruction supplies conditional rmdir guidance for an empty marker (328790d). Thus the reported show-versus-attach difference is not yet established under the same binary, cwd and environment. [ref:brief] frames the isolated routing comparison and the separate close-time scratch warning. Preserve broken-repository errors and checkout/worktree precedence while choosing a remedy; do not add an attachment-specific fallback before the matrix identifies the cause.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-24-scratch-evidence-brief.md — shared routing evidence and unanswered incident questions
