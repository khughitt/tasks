---
id: tasks-16a48c
title: A write that cannot find a worktree-only task names the checkout its claim or park records
status: doing
priority: 2
size: s
complexity: mid
process: direct
owner: fix/tasks-16a48c
created: 2026-10-15T12:37:58Z
updated: 2026-10-15T18:05:03Z
started: 2026-10-15T18:04:52Z
depends: []
parent: tasks-984769
tags: [feedback, friction, "from:tasks"]
agent: claude-code/claude-opus-5-5
---

Why: show reads a record this checkout lacks from the checkout its live claim, or else its park, names (record-home spec §6.1, `recorded_elsewhere` in src/commands/show.rs, bd7cb18). Write commands load through `load` in src/commands/mod.rs, which calls `read_task_with_raw` and returns a bare task_not_found. So `note <id>` fails for the same id that show finds, with nothing pointing at the checkout. Two reports: tasks-16a48c (from tasks: show resolved the record via the park's worktree; note <id> from the main checkout of another project failed task_not_found; worked around with -C <park worktree>) and tasks-ae03ed (from mind6: a cross-project note could not find a task known in a linked worktree; rerunning from its worktree succeeded). The skill says note routes by the id's prefix, and it does; the registered root just lacks the record.

Done: when `load` hits TaskNotFound and a live claim or park names a checkout that holds the record (`parked::open_recorded`), the error keeps the task_not_found kind and gains a suffix that names the checkout and gives the remedy under record-home §3.2's rules. Another session's claim or park: name the task and session, with no -C retry (rule 1). Otherwise: `Run it there: tasks -C <worktree> <args>` via `stale::rerun_line` (rule 4). A named checkout that is gone or lacks the record gets show's "which is unavailable" detail. No write is routed automatically (§7). Integration tests in tests/cli.rs cover note and one other write command from main for a worktree-only task under the caller's own park, under another session's claim, and with the named checkout removed. Add one line to the record-home spec (§6 or §8) and the stale_copy paragraph of skills/tasks/SKILL.md.

Where: src/commands/mod.rs (`load`), src/commands/show.rs (`recorded_elsewhere`, to share), src/stale.rs (`refusal`, `rerun_line`), src/commands/parked.rs (`open_recorded`). Coordinate with tasks-a984f1, which asks whether an explicit note into another session's checkout is safe. If it settles that, rule 1's remedy changes for every write, this one included.

## Notes

- 2026-10-15T16:56:12Z (main): scope: scoped; rewrote as the write-path counterpart of show's §6.1 fallback (error remedy, no routing), todo P2/s/mid/direct under tasks-984769. Premise checked against main 20544c4: note -> open_id_write_ctx -> load -> read_task_with_raw has no claim/park lookup. Absorbs tasks-ae03ed.
- 2026-10-15T18:04:52Z (main): started
  provenance: {"harness_session":"codex:01a140a7-9d0a-7fb3-9bbc-121ffeb29423","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-15T18:05:03Z (fix/tasks-16a48c): resumed
  provenance: {"harness_session":"codex:01a140a7-9d0a-7fb3-9bbc-121ffeb29423","harness_session_source":"CODEX_THREAD_ID"}
