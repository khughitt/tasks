---
id: tasks-6010b7
title: Close the date recency color review minors
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-09-25T12:27:49Z
updated: 2026-09-25T12:27:49Z
depends: []
tags: [cli]
source: tasks-142d2f final review
agent: claude-code/claude-opus-5-5
---

From the tasks-142d2f final branch review, deferred: (1) Ctrl-C during the query window exits without RawModeGuard::drop (ISIG stays on), leaving echo/canonical off in shells that do not reset modes; restore on SIGINT or clear ISIG for the exchange. (2) Palette::query's has_connected_stdio_stream check cannot fail while stdout is a terminal; remove it. (3) terminal() errors all map to NoTerminal; map NotFound/ENXIO there and the rest to QueryError::Io. (4) tests/common/mod.rs shim_command does not set TASKS_PALETTE (spec §3.3's isolation argument applies). (5) exchange's doc comment calls a timeout the only exit that can leave bytes; an I/O error is another. (6) quiet's painted park date has no end-to-end test.
