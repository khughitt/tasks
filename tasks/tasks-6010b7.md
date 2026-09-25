---
id: tasks-6010b7
title: Close the date recency color review minors
status: done
priority: 3
size: s
complexity: low
process: direct
owner: main
created: 2026-09-25T12:27:49Z
updated: 2026-09-25T12:51:19Z
started: 2026-09-25T12:45:58Z
completed: 2026-09-25T12:51:19Z
depends: []
tags: [cli]
source: tasks-142d2f final review
agent: claude-code/claude-opus-5-5
---

From the tasks-142d2f final branch review, deferred: (1) Ctrl-C during the query window exits without RawModeGuard::drop (ISIG stays on), leaving echo/canonical off in shells that do not reset modes; restore on SIGINT or clear ISIG for the exchange. (2) Palette::query's has_connected_stdio_stream check cannot fail while stdout is a terminal; remove it. (3) terminal() errors all map to NoTerminal; map NotFound/ENXIO there and the rest to QueryError::Io. (4) tests/common/mod.rs shim_command does not set TASKS_PALETTE (spec §3.3's isolation argument applies). (5) exchange's doc comment calls a timeout the only exit that can leave bytes; an I/O error is another. (6) quiet's painted park date has no end-to-end test.

## Notes

- 2026-09-25T12:45:58Z (main): started
- 2026-09-25T12:51:15Z (date-minors): host pointer: ~/.cargo/bin/tasks repointed to .worktrees/date-minors build by cargo install --path . during this task; restore by reinstalling from the main checkout before the worktree goes away
- 2026-09-25T12:51:19Z (date-minors): done
- 2026-09-25T12:51:19Z (date-minors): Six review minors closed: ISIG cleared for the exchange behind its own guard so Ctrl-C cannot leave raw mode on; the redundant has_connected_stdio_stream check removed from Palette::query; terminal() errors mapped NotFound/ENXIO to NoTerminal and the rest to Io; shim_command sets TASKS_PALETTE like the other helpers; exchange's doc comment names an I/O error as another byte-leaving exit; and an end-to-end test pins quiet's painted park date. Spec §3.1/§3.3/§5 updated.
