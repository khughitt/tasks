---
id: tasks-a14f0d
title: Validate TASKS_FORMAT even when --pretty overrides it
status: todo
priority: 3
size: xs
created: 2026-09-04T02:03:12Z
updated: 2026-09-04T02:03:12Z
depends: []
tags: [cli]
---

main.rs matches (true, _) first, so --pretty with TASKS_FORMAT=xml succeeds and never validates the variable; without --pretty the same value is a config error. Fail-early says validate whenever set. Deferred from the colour design because it turns a currently-succeeding command into an error.
