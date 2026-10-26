---
id: tasks-40b49b
title: Cross-project dependencies cannot resolve records in an unmerged task worktree
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-26T15:20:32Z
created: 2026-10-26T13:21:06Z
updated: 2026-10-26T15:20:32Z
depends: []
tags: [feedback, gap, "from:obs"]
agent: codex
origin: agent
---

Creating cross-project plan steps with a dependency on a newly filed step returned unresolvable_id: resolution used the registered root while the authoritative record existed in the plan worktree. A way to validate against the authoritative checkout or defer the edge would avoid textual ordering until approved plan records are integrated. The workaround preserves the ordering in the plan and adds the edge after those records land.

## Notes

- 2026-10-26T13:21:06Z (feedback): created
  provenance: {"harness_session":"codex:01a1780f-e78c-7582-93a0-9507ebf89699","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
- 2026-10-26T15:20:32Z (main): scope: drop; original report preserved; tasks-d0d520 already demonstrates registered-record publication before foreign dependency creation and after worktree completion, and tasks-70f0bc owns diagnostics, publication guidance and the later branch-merge check; current src/resolve.rs::resolve_task and src/commands/dep.rs::add_dependencies retain this authority at f1d45f2; no second resolver or research task; origin: filing matched codex:01a1780f-e78c-7582-93a0-9507ebf89699 call line 592 and full created result line 595, id and at match 2026-10-26T13:21:06Z; line 584 captures unresolvable_id, line 591 is the agent workaround; human selected an existing task at line 14 and later reviewed its spec, not this incidental tracker remedy, so origin agent stands; no origin_session on occurrence, no older-request stage; brief: docs/notes/2026-10-06-record-continuity-brief.md; proposal: drop as covered by tasks-70f0bc, with behavior established by tasks-d0d520
  provenance: {"harness_session":"codex:01a178c5-fc4f-79b0-847a-9aa4826e274e","harness_session_source":"CODEX_THREAD_ID"}
