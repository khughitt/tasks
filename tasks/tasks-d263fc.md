---
id: tasks-d263fc
title: Fit the unregistered-worker refusal's causes to registries that already hold worker rows
status: doing
priority: 3
size: xs
complexity: low
process: direct
owner: crew
created: 2026-10-15T12:41:34Z
updated: 2026-10-16T18:10:39Z
started: 2026-10-16T18:09:11Z
depends: []
tags: [relay]
source: tasks-af3aac
agent: claude-code/claude-opus-5-5
---

hint_mismatch in src/relay/resolve.rs tells a worker with no relay row that the relay writing the registry predates worker rows or its Codex hooks are not installed. Once relay publishes worker rows (relay-635fdc), the usual cause when sibling worker rows exist on the same process is that relay has not yet seen this worker's first hook, or pruned its row. Choose the cause list by whether the candidates include a scope-worker row (or add that clause), keep the message value-free with TASKS_SESSION recovery, and pin both forms in relay_worker_rows_refuse_an_unregistered_thread_without_values plus a root-only control. Deferred minor from tasks-af3aac's final review.

## Notes

- 2026-10-16T18:09:11Z (crew): started
- 2026-10-16T18:10:39Z (crew): crew: attempt 01M52YEMXK3BNP2ZSVV3JQXZ89 started — kind code-xs, worker openrouter/z-ai/glm-5.3-flash, worktree .worktrees/crew-v3jqxz89
