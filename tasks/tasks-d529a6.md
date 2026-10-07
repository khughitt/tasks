---
id: tasks-d529a6
title: Preserve corrupt Git metadata errors during linked discovery
status: done
priority: 2
size: s
complexity: mid
process: direct
owner: feat/linked-projects
created: 2026-10-07T11:24:09Z
updated: 2026-10-07T11:30:19Z
started: 2026-10-07T11:25:03Z
completed: 2026-10-07T11:26:56Z
depends: []
parent: tasks-9b0a2e
tags: []
agent: codex
---

Final whole-branch review Important finding: corrupt .git/HEAD is misclassified as no repository and bypasses linked init refusal. Fix shared discovery once, add RED→GREEN where/init regressions with no-write assertions, rerun focused linked tests, fast suite and checks, then scoped re-review.

## Notes

- 2026-10-07T11:25:03Z (feat/linked-projects): started
  provenance: {"harness_session":"codex:01a1161b-88ec-7572-aa17-b01616b634f6","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-07T11:26:37Z (feat/linked-projects): Corrupt HEAD makes Git emit the same not-repository text as absence. Shared GitTree::of now checks ancestor .git markers before treating that message as absence; RED reproduced where/null and plain init/write, GREEN covers main/subdirectory/external worktree without destination or registry changes.
- 2026-10-07T11:26:56Z (feat/linked-projects): done
  provenance: {"harness_session":"codex:01a1161b-88ec-7572-aa17-b01616b634f6","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-07T11:26:56Z (feat/linked-projects): Shared Git discovery preserves io errors for corrupt metadata instead of treating existing .git markers as absence; where and plain init regressions protect main/subdirectory and external linked worktrees, registry, and code-tree status.
  provenance: {"harness_session":"codex:01a1161b-88ec-7572-aa17-b01616b634f6","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-07T11:30:19Z (feat/linked-projects): review: impl round 1 — verdict: accept; findings: Minor 1; reviewer: codex/gpt-6-astra
