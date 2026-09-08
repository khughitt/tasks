---
id: tasks-88d356
title: "tree <id> does not resolve an id from another registered project even though dep, note, and show do"
status: done
priority: 2
size: xs
owner: fix/tree-routes-by-prefix
created: 2026-09-07T00:07:50Z
updated: 2026-09-08T10:46:41Z
depends: []
tags: [feedback, friction, "from:tasks", cli]
---

tasks tree <foreign-id> returns task_not_found while show and every write command route the same id by its prefix. tree builds its context with open_read_ctx, which never consults the id; show re-opens the registered project at src/commands/show.rs:12 and the write commands do it through open_id_write_ctx at src/commands/mod.rs:174. tree is the only id-taking command missing that step.

Not a behaviour change: docs/specs/2026-09-04-multi-project-design.md already says the subtree of an id is read from that id's own project. The implementation drifted from it, and skills/tasks/SKILL.md propagated the drift outward by documenting 'a bare id in tree is not routed by its prefix'. The spec needs no change; the skill line goes.

Fix: a read counterpart to open_id_write_ctx, used only by tree. With no explicit scope and an id whose prefix names another registered project, re-scope through scope::open_registered with Origin::Id, the same call show makes, so an unregistered prefix gives show's error rather than a misleading task_not_found. An explicit --project wins over the id's prefix, since the caller named the scope; --all-projects with an id stays a clap conflict. tree.rs's comment asserting the scope is always local stops being true.

## Notes

- 2026-09-07T09:30:50Z (feat/read-scope-project): Still open after tasks-7eb169: --project <prefix> gives tree an explicit read scope (tasks tree --project fam <id>), but a bare foreign id in tree is still not routed by its prefix.
- 2026-09-08T10:46:41Z (fix/tree-routes-by-prefix): tree now routes a bare id by its prefix through open_id_read_ctx, the read counterpart to open_id_write_ctx, using the same scope::open_registered call show makes -- so an unregistered prefix gives unresolvable_id rather than a misleading task_not_found. An explicit --project still wins. The multi-project design already specified this; the fix was drift, and skills/tasks/SKILL.md carried the drift outward. A test at tests/cli.rs was pinning the old behaviour and now asserts the routing. Verified: tasks tree ops-500adb from the tasks repo reads the ops subtree.
