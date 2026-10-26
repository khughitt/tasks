---
id: tasks-e176a0
title: Design explicit closeout-hub identity from the blocker brief
status: todo
priority: 2
size: s
complexity: high
process: planned
created: 2026-10-26T14:59:53Z
updated: 2026-10-26T14:59:53Z
depends: []
parent: tasks-46d207
tags: [cli]
source: tasks-9d2f66
agent: codex
origin: agent
---

Why: the shipped blockers report intentionally counts childless cross-project closeout hubs as work because structural goal detection cannot distinguish them from ordinary consumers. [ref:brief] records this known inflation risk and the original report boundary. This task designs the explicit distinction; it does not assume a schema field or infer identity from prose.

Done: write a reviewable design choosing between task-level goal identity and edge-purpose metadata (or explain why retaining the current report is preferable). Specify existing-record behavior, mixed closeout/ordinary edges, moves and renames, direct/transitive work and goal totals, status partitions, relief counts, JSON and pretty disclosure, and the impact on hierarchy and readiness callers. Check a childless closeout hub against an ordinary consumer with identical dependencies, plus a diamond and mixed-purpose dependencies. Readiness must keep its existing semantics unless an explicit reviewed change is justified. Bring the design to the user; any implementation needs a separately reviewed plan and scoped steps. No product-code change is part of this design task.

Where to start: [ref:brief], docs/specs/2026-10-25-blocker-ranking-design.md (approved and shipped), src/hierarchy.rs::is_goal, src/blockers.rs, src/model.rs, src/frontmatter.rs, src/commands/blockers.rs, src/blockers_text.rs and tests/blockers.rs. CLI vocabulary, if proposed, lands in ops before adoption; do not change tools/cli.toml directly. Coordinate with tasks-71c74d without gating its history collection.

Ideas it wakes: on completion record the result and recommendation on this task and in [ref:brief], then run tasks note tasks-9d2f66 --no-reviewed "<design result and remaining decision>" before this result's branch merges. A decision to keep current behavior is a valid result and must be explicit.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-25-blocker-ranking-brief.md — current report contracts and follow-on design questions

## Notes

- 2026-10-26T14:59:53Z (add): created
  provenance: {"harness_session":"codex:01a178b3-d016-7562-a8e1-983f62cc0118","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
