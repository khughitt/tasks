---
id: tasks-5ab7e8
title: "Migrate a project's open tasks to its successor: a triage view and a bulk move"
status: idea
priority: 3
created: 2026-10-23T00:10:51Z
updated: 2026-10-23T00:10:51Z
depends: []
tags: []
source: "forge-0e720e, 2026-10-22"
agent: claude-code
---

Gap met while superseding a project with a fresh successor (forge v1 to v2, 2026-10-22). tasks retire --successor needs every open task closed or moved, but tasks move takes one id at a time, and nothing helps decide per task whether it carries over, is dropped as obsolete, or is closed as done elsewhere. Wanted: a view of a project's open tasks for that decision (carry, drop with a shared reason, close), applied in one transaction that moves the carried ones with their notes, parents and dependency edges intact and leaves tombstones, so the old project can then retire. Related: ops idea on project generations filed the same day.
