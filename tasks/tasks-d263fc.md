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
updated: 2026-10-16T18:38:54Z
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
- 2026-10-16T18:14:39Z (crew): crew: attempt 01M52YEMXK3BNP2ZSVV3JQXZ89 — verdict passed (worker openrouter/z-ai/glm-5.3-flash, reviewer openrouter/openai/gpt-6.1-sol); report /home/keith/.local/state/crew/attempts/01M52YEMXK3BNP2ZSVV3JQXZ89/attempt-report.md
- 2026-10-16T18:14:40Z (crew): parked (waiting on user, review): review crew attempt 01M52YEMXK3BNP2ZSVV3JQXZ89 (verdict passed): branch crew/tasks-d263fc-v3jqxz89 in .worktrees/crew-v3jqxz89; merge and close, or discard
- 2026-10-16T18:15:15Z (crew): resumed
- 2026-10-16T18:16:47Z (crew): crew: attempt 01M52YSRFP77SMFC54WWND8QF6 started — kind code-xs, worker openrouter/qwen/qwen3-coder-next, worktree .worktrees/crew-wwnd8qf6
- 2026-10-16T18:27:31Z (crew): crew: attempt 01M52YSRFP77SMFC54WWND8QF6 — verdict not-passed (worker openrouter/qwen/qwen3-coder-next, reviewer openrouter/openai/gpt-6.1-sol); report /home/keith/.local/state/crew/attempts/01M52YSRFP77SMFC54WWND8QF6/attempt-report.md
- 2026-10-16T18:27:32Z (crew): parked (waiting on user, review): review crew attempt 01M52YSRFP77SMFC54WWND8QF6 (verdict not-passed): branch crew/tasks-d263fc-wwnd8qf6 in .worktrees/crew-wwnd8qf6; merge and close, or discard
- 2026-10-16T18:28:06Z (crew): resumed
- 2026-10-16T18:29:40Z (crew): crew: attempt 01M52ZH983B7SPJQ6X0MCY5F1M started — kind code-xs, worker openrouter/anthropic/claude-sonnet-5.5, worktree .worktrees/crew-0mcy5f1m
- 2026-10-16T18:31:56Z (crew): crew: attempt 01M52ZH983B7SPJQ6X0MCY5F1M — verdict passed (worker openrouter/anthropic/claude-sonnet-5.5, reviewer openrouter/openai/gpt-6.1-sol); report /home/keith/.local/state/crew/attempts/01M52ZH983B7SPJQ6X0MCY5F1M/attempt-report.md
- 2026-10-16T18:31:57Z (crew): parked (waiting on user, review): review crew attempt 01M52ZH983B7SPJQ6X0MCY5F1M (verdict passed): branch crew/tasks-d263fc-0mcy5f1m in .worktrees/crew-0mcy5f1m; merge and close, or discard
- 2026-10-16T18:32:40Z (crew): resumed
- 2026-10-16T18:34:11Z (crew): crew: attempt 01M52ZSMSS59VH45XP36CT2EJ8 started — kind code-xs, worker openrouter/deepseek/deepseek-v4-flash-0731, worktree .worktrees/crew-36ct2ej8
- 2026-10-16T18:38:53Z (crew): crew: attempt 01M52ZSMSS59VH45XP36CT2EJ8 — verdict passed (worker openrouter/deepseek/deepseek-v4-flash-0731, reviewer openrouter/openai/gpt-6.1-sol); report /home/keith/.local/state/crew/attempts/01M52ZSMSS59VH45XP36CT2EJ8/attempt-report.md
- 2026-10-16T18:38:54Z (crew): parked (waiting on user, review): review crew attempt 01M52ZSMSS59VH45XP36CT2EJ8 (verdict passed): branch crew/tasks-d263fc-36ct2ej8 in .worktrees/crew-36ct2ej8; merge and close, or discard
