---
id: tasks-980a39
title: check reports step_missing on dropped tasks and edit cannot clear a plan or step link
status: idea
priority: 2
created: 2026-09-13T15:32:44Z
updated: 2026-09-13T15:32:44Z
depends: []
tags: [feedback, gap, "from:tasks"]
---

Merged two plan headings into their neighbours and dropped the two step tasks with tasks drop; tasks check then failed with step_missing for the dropped tasks because their old headings were gone. tasks edit has --plan and --step but no --no-plan/--no-step, so the only way out was to repoint the dropped tasks at the surviving headings. Expected: either check ignores step links on closed tasks, or edit can clear them.
