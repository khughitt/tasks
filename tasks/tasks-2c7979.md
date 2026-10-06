---
id: tasks-2c7979
title: A park overlay that outlives its task's close stays in prime's parked section with no direct way to clear it; only a recompletion (done on a done task) removes it
status: idea
priority: 2
created: 2026-10-06T18:13:29Z
updated: 2026-10-06T18:13:29Z
depends: []
tags: [feedback, gap, "from:material"]
agent: claude-code/claude-opus-5-5
---

prime lists a done task under parked (list --parked omits it). The only clearing path found is re-running tasks done, which adds a close note and restamps model unless TASKS_MODEL is unset. Expected: done/drop by another session clears it, or an explicit unpark.
