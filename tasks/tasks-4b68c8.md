---
id: tasks-4b68c8
title: "tasks attach fails outside a checkout although tasks show resolves the same id from anywhere, and its error suggests removing an unrelated empty .git directory"
status: idea
priority: 2
created: 2026-10-23T17:52:50Z
updated: 2026-10-23T17:52:50Z
depends: []
tags: [feedback, friction, "from:hq"]
agent: claude-code
---

Action: tasks attach <id> <file> run from a scratch directory off the synced tree (not a git repository), with a valid id. Got an io error from git rev-parse in the current directory, which named the nearest empty .git directory above it and suggested rmdir on it. Expected: attach resolves the task's project from the id, as tasks show and tasks note do from any directory, and reads the file by its path. Workaround: run attach from the project checkout with absolute file paths. Constraint: per-task scratch lives off the synced tree by rule, so attaching from it is the common case at task close.
