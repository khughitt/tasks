---
id: tasks-71c74d
title: Design blocker impact and persistence reporting from the brief
status: todo
priority: 2
size: s
complexity: high
process: planned
created: 2026-10-25T21:37:44Z
updated: 2026-10-25T21:37:44Z
depends: []
parent: tasks-6166d6
tags: []
source: tasks-e637a1
agent: codex
origin: agent
---

Why: tasks-e637a1 asks for downstream impact and persistent blocking, but graph-counting, historical-evidence and presentation decisions remain. [ref:brief] establishes the current code, originating request and recommended advisory-report boundary; do not repeat that discovery as a separate research task.

Done: produce a written design spec for user review, then an implementation plan for user review before any implementation. Settle unique direct/transitive counts versus immediately released work, goals/closeout edges, cross-project and unavailable coverage, deterministic ranking, and evidence-backed age/scope-pass meanings. Cover all dependency mutation paths and removal/re-addition, reopening/recurrence, aliases and moves. Unknown history stays explicit. Choose one initial JSON/pretty read surface and concrete verification cases; do not silently change stored priorities, ready/next ordering or admission gates. Reconcile tasks-e8d471 and canonical dependency-satisfaction work; any CLI vocabulary lands in ops before parser implementation. No implementation or paid runtime probe is authorized by this scope pass.

Where to look: [ref:brief]; src/commands/dep.rs (add_dependencies, shared by dep/add/edit), src/commands/show.rs, src/commands/list.rs (resolve_dependency, ready, prime), src/moving/mod.rs (inbound), src/scope.rs, src/query.rs, src/model.rs, src/review.rs; tasks-e8d471, tasks-1bbe49/tasks-606dd3, tasks-298107 and crew-a0952b. Design can proceed before reverse dependents land.

Ideas it wakes: On completion, record the design outcome in the brief and run tasks note tasks-e637a1 --no-reviewed "<finding>" before the result branch merges. The idea must also depend on this task.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-25-blocker-ranking-brief.md — evidence, alternatives, metric questions and verification cases

## Notes

- 2026-10-25T21:37:44Z (add): created
  provenance: {"harness_session":"codex:01a174fb-0fa0-7571-93ce-b3218da174d1","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
