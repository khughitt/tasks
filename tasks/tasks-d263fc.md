---
id: tasks-d263fc
title: Fit the unregistered-worker refusal's causes to registries that already hold worker rows
status: todo
priority: 3
size: xs
complexity: low
process: direct
created: 2026-10-15T12:41:34Z
updated: 2026-10-15T12:41:34Z
depends: []
tags: [relay]
source: tasks-af3aac
agent: claude-code/claude-opus-5-5
---

hint_mismatch in src/relay/resolve.rs tells a worker with no relay row that the relay writing the registry predates worker rows or its Codex hooks are not installed. Once relay publishes worker rows (relay-635fdc), the usual cause when sibling worker rows exist on the same process is that relay has not yet seen this worker's first hook, or pruned its row. Choose the cause list by whether the candidates include a scope-worker row (or add that clause), keep the message value-free with TASKS_SESSION recovery, and pin both forms in relay_worker_rows_refuse_an_unregistered_thread_without_values plus a root-only control. Deferred minor from tasks-af3aac's final review.
