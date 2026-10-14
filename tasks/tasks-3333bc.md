---
id: tasks-3333bc
title: "The pretty-table '*' and --needs-review merge two states with different remedies: an unreviewed idea, which the scope skill's default selection picks up, and a leaf todo/blocked task with no process, which that selection never returns (it filters --status idea), so starred todos can only be cleared by naming their ids as re-scopes. Proposal: make the scope skill's default selection read the whole --needs-review set (unreviewed ideas first, then process-less todos as re-scopes), so '*' means 'in the scope queue'; and label the two kinds apart in pretty output (e.g. the mark for ideas, the existing '-' process column for todos, or a reason field in JSON)."
status: todo
priority: 2
created: 2026-10-14T12:53:25Z
updated: 2026-10-14T13:22:46Z
depends: []
tags: [feedback, friction, "from:ops"]
---
