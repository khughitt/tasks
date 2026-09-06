---
id: tasks-8c9398
title: No way to rename a project prefix
status: idea
priority: 2
created: 2026-09-06T09:37:15Z
updated: 2026-09-06T09:37:15Z
depends: []
tags: [feedback, gap, "from:tasks"]
---

Renaming a project alias today means hand-editing three layers the binary owns: the registry key in ~/.config/tasks/projects.toml, prefix in the project's tasks/.config.toml, and every task id (filename + id: frontmatter). Worse, inbound cross-project references in OTHER registered projects (depends: lists, parent:, note prose, spec/plan docs) go stale and check.rs foreign-id resolution then fails there. Did this by hand for aut -> autonomy (empty project, trivial) and dot -> dots (2 task files, 4 inbound refs across ops and prism, 3 doc mentions). A 'tasks rename <old> <new>' would need to rewrite references in every registered project, not just the local one -- that cross-project write is the real design question, since every other command writes only within one project.
