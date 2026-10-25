---
id: tasks-a7a8ce
title: "A read of a full task id fails with an io error when the cwd sits under an empty .git directory, instead of routing by prefix as it does from a plain non-git directory"
status: idea
priority: 2
created: 2026-10-25T11:26:06Z
updated: 2026-10-25T11:26:06Z
depends: []
tags: [feedback, friction, "from:hq"]
agent: claude-code
---

Running 'tasks show <prefix>-<hex> --json' from a non-repository directory works, but from a directory whose ancestor holds an empty .git directory it exits 1 with kind io (git rev-parse failed, not a git repository). Expected: the id's prefix routes to its registered project regardless of cwd, as it does when no .git ancestor exists; the empty directory is not a repository. The error does name the empty directory and suggests removing it, which helped diagnosis.
