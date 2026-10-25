---
id: tasks-553c8e
title: Establish whether record commits can exclude staging that arrives during hooks
status: todo
priority: 2
size: s
complexity: high
process: direct
created: 2026-10-25T15:49:17Z
updated: 2026-10-25T15:49:17Z
depends: []
parent: tasks-bbfc3c
tags: []
source: "tasks-056222:commit-isolation-investigation"
agent: codex
origin: agent
---

Question: Can record commits exclude unrelated concurrent Git staging while preserving hook-generated files, partial staged versions and cleanup behavior?

Where to start: [ref:brief], src/record_commit.rs::commit_paths_in, its unit tests for derived-file hooks and refusal cleanup, tests/record_commit.rs, and the approved docs/specs/2026-10-10-record-commit-design.md §§3.3/3.7. That design explicitly retains the during-hook staging exception; the report is not proof the initial staged-work check failed.

Bound: Reproduce one foreign file staged during a controlled hook, with task-only and hook-derived-file controls in disposable repositories. Clear inherited GIT_* selectors, isolate global/system configuration, assert fixture top-level and git-dir before writes, and construct subprocess environments key by key. Compare retaining serialization guidance with a narrow isolation approach; account for HEAD/index locking, staged partial content, failed hooks and derived files. Do not change the host's Git configuration, use real repositories as fixtures, or implement a new transaction framework.

Expected result: Record observed commits, staged blobs and post-failure state here and in [ref:brief]; recommend either an explicit retained limitation or one feasible contract with regression checks. Explain why a pathspec-only commit does or does not repeat the approved design's stale-index problem. A behavior change needs a reviewed design and plan; the research itself is bounded direct work.

Ideas it wakes: On completion, run `tasks note tasks-056222 --no-reviewed "<finding>"` before this result's branch merges.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-25-evidence-handoffs-brief.md — evidence, questions and follow-up boundaries

## Notes

- 2026-10-25T15:49:17Z (add): created
  provenance: {"harness_session":"codex:01a173bc-eec8-72f3-afcd-4de019909d0d","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
