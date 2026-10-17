---
id: tasks-52bd39
title: "tasks dep --on a shelved task, and tasks shelve on a task that open tasks depend on, both succeed without a warning; only a later tasks check reports shelved_dep"
status: idea
priority: 2
created: 2026-10-17T22:09:56Z
updated: 2026-10-17T22:09:56Z
depends: []
tags: [feedback, gap, "from:mind6"]
agent: claude-code
---

Reproduced in a scratch project: shelve B then dep A --on B, and dep C --on D then shelve D, each printed warnings: []; check then reported both. Shelve already refuses a goal with unshelved open descendants but ignores dependents. Meanwhile a reviewed idea waiting on the shelved task stays out of the needs-review queue (a shelf counts as an open wait), so nothing surfaces it until someone runs check. In the case that prompted this, a batch dependency backfill added an edge onto a task shelved a week earlier, and the stranded idea was found only through a pre-commit check warning. A warning at write time naming the dependents (on shelve) or the shelved target (on dep), with check's three remedies (unshelve, drop the edge, shelve the dependent), would catch it when it happens.
