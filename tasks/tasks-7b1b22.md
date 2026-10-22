---
id: tasks-7b1b22
title: Establish a safe error contract for missing and half-removed checkouts
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-22T23:10:52Z
updated: 2026-10-22T23:10:52Z
depends: []
parent: tasks-a7460c
tags: []
source: tasks-d6bd82
agent: codex
---

Question: Which checkout failures can tasks classify as absence without hiding corruption, permission failures or other real read errors?
Where to start: [ref:brief] preserves both reported cases and the existing discovery safeguards. Trace src/repo.rs::Project::locate, src/link.rs::GitTree::of, src/error.rs and show/list dispatch; recheck tasks-211e28 integration before choosing the boundary.
Bound: Trace the shared call paths and exercise a disposable matrix: absent directory, surviving worktree with missing Git metadata, corrupt marker, ordinary non-repository, healthy unborn repository and permission/read failure. Isolate Git config, registry and state. No production-code changes or consumer rollout.
Expected result: Record observed JSON kinds, which states can be distinguished reliably, negative controls and the smallest proposed error contract on this task and in the brief. Address both reported asks; do not equate Git exit 128 with absence. Record any untestable case explicitly.
Ideas it wakes: Before this result's branch merges, run tasks note tasks-d6bd82 --no-reviewed "<finding>" with the classification recommendation and limits.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-22-missing-checkout-errors-brief.md — evidence, constraints and open questions
