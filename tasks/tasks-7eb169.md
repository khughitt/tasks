---
id: tasks-7eb169
title: "list has no --project flag; reading another registered project needs -C <dir> while add, note, and dep route by prefix"
status: done
priority: 2
size: s
created: 2026-09-07T08:29:07Z
updated: 2026-09-07T09:36:46Z
depends: []
tags: [feedback, gap, "from:tasks", cross-project]
---

Wanted to list tasks in another registered project from a hub checkout. add --project <prefix> exists, and every id-taking command routes by prefix, but list (and ready) only offer -C <dir> or --all-projects. Expected list --project <prefix> for symmetry.

## Notes

- 2026-09-07T09:30:55Z (feat/read-scope-project): Read scope flag --project <prefix> on list, ready, next, prime, tree, tags via a shared ScopeArgs; reads the registered root, conflicts with --all-projects, needs no local project; complete::scoped follows it.
- 2026-09-07T09:36:46Z (main): Scoped before implementing: --project <prefix> as the third read scope on list, ready, next, prime, tree, tags (shared ScopeArgs), plus complete::scoped. Separate from tasks-88d356, which is bare-id routing for tree.
