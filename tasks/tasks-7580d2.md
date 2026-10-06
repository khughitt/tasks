---
id: tasks-7580d2
title: Record a project's former roots and resolve ids and paths through renames
status: doing
priority: 1
size: m
complexity: high
process: planned
owner: design/tasks-7580d2
created: 2026-10-06T13:23:04Z
updated: 2026-10-06T19:42:36Z
started: 2026-10-06T18:41:11Z
depends: []
parent: tasks-a7460c
tags: []
source: tack-8b7a28
agent: claude-code/claude-fable-5-1
spec: docs/specs/2026-10-06-project-resolution-design.md
plan: docs/plans/2026-10-06-project-resolution.md
---

Why: a prefix rename keeps aliases, but once it completes nothing on the host records the directory the project used to live in (the rename state directory is empty afterwards), and each consumer that maps an id or a path to a project reads [projects] by hand: obs's indexer and join, flows' trial tools, tack's session-episodes. So a rename drops their history (tack docs/specs/2026-10-06-rename-to-hq-design.md §3.5, §3.6; obs-ff4e76, flows-44890e). The user asked on 2026-10-06 that renames be made smoother in general before tack's rename to hq. Done: the registry records each former root when rename, init --prefix --force or rename --adopt changes a root or a prefix, per host; one read-only resolver answers the canonical id for any id form (alias prefixes included) and the project for any path (current root, former roots, and the worktree storage of each); its output is stable JSON a hook or an indexer can call; a host that adopts a rename made elsewhere ends with the same answers; and the existing ai alias gets its former root backfilled or is documented as unknown. This task blocks the cutover (tack-8b7a28) and is the mechanism obs-ff4e76 and flows-44890e are asked to use.

Lane coordination and acceptance: during design, compare the proposed id/path resolver with tasks-9b0a2e's tasks where and locate contracts, distinguishing records root, linked code checkout, and worktree storage. Share registry identity and path classification rather than making consumers read independent registry formats. Agree the boundary before implementation; the linked-checkout feature need not land before former-root support.

Cover canonical and aliased ids, current/former roots and their worktree storage, unregistered or ambiguous paths, and adoption on another host with that host's own storage paths. Record explicit limits where former-root history is unavailable. The rename consumers keep their own historical task/trial identity contract and end-to-end checks; this task supplies the shared resolver rather than taking over their delivery. Preserve the existing written-design and implementation-plan review gates.

## Notes

- 2026-10-06T16:52:01Z (main): organization: first milestone of the P1 project-location lane; coordinate the resolver boundary with tasks-9b0a2e and the existing rename consumers, without making linked-checkout delivery a prerequisite.
- 2026-10-06T18:41:11Z (main): started
  provenance: {"harness_session":"claude-code:00da167b-561d-40c6-aa34-7a035e885cf2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T18:41:20Z (design/tasks-7580d2): resumed
  provenance: {"harness_session":"claude-code:00da167b-561d-40c6-aa34-7a035e885cf2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T18:48:16Z (design/tasks-7580d2): design: separate read-only 'tasks resolve' (registry history, existence-independent) beside 9b0a2e's live 'where', sharing a src/locations.rs index; per-host [locations] table records storage and former roots; rejected inferring storage from the work-link layout
- 2026-10-06T18:48:41Z (design/tasks-7580d2): parked (waiting on user, review): user reviews docs/specs/2026-10-06-project-resolution-design.md; on approval the agent writes the implementation plan in .worktrees/tasks-7580d2
  provenance: {"harness_session":"claude-code:00da167b-561d-40c6-aa34-7a035e885cf2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T18:56:27Z (design/tasks-7580d2): review: spec round 1 — verdict: revise; findings: P1 1, P2 3; reviewer: codex/gpt-6
- 2026-10-06T18:56:27Z (design/tasks-7580d2): spec review findings: P1 external-worktree git fallback loses the nested-project offset (scratch reproduction resolves parent instead of child); P2 move-back removes former storage when the returning root uses different storage; P2 adoption must merge history when the target is already registered rather than overwrite it; P2 six-hex strings overlap the valid-prefix grammar, so classification needs explicit precedence. Revise these rules and add regression cases before implementation planning.
- 2026-10-06T19:05:27Z (design/tasks-7580d2): resumed
  provenance: {"harness_session":"claude-code:00da167b-561d-40c6-aa34-7a035e885cf2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T19:06:34Z (design/tasks-7580d2): spec round 1 addressed: git fallback maps the path into the main checkout keeping its offset, then reapplies the depth rule; former entries keyed by (root, storage) and kept on move-back; adoption merges both [locations] tables; classification order path > id > prefix, so six-hex strings starting with a letter are prefixes
- 2026-10-06T19:06:36Z (design/tasks-7580d2): parked (waiting on user, review): user re-reviews docs/specs/2026-10-06-project-resolution-design.md (round 2); on approval the agent writes the implementation plan in .worktrees/tasks-7580d2
  provenance: {"harness_session":"claude-code:00da167b-561d-40c6-aa34-7a035e885cf2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T19:08:46Z (design/tasks-7580d2): review: spec round 2 — verdict: revise; findings: P2 1; reviewer: codex/gpt-6
- 2026-10-06T19:08:46Z (design/tasks-7580d2): spec round 2: all four round-1 findings are addressed. One P2 remains: section 3.2 says worktree is storage-only and relpath is relative to worktree when present, but section 4.3 and CLI test 5 report via git, worktree T, relpath x for T/sub/nested/x. Joining the fields yields T/x. Recommend allow worktree for git results, keep worktree T, and use relpath sub/nested/x; update the regression expectation to preserve the general field contract.
- 2026-10-06T19:11:15Z (design/tasks-7580d2): resumed
  provenance: {"harness_session":"claude-code:00da167b-561d-40c6-aa34-7a035e885cf2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T19:11:24Z (design/tasks-7580d2): spec round 2 addressed: worktree is allowed on git matches and relpath is always relative to worktree when present (git example: sub/nested/x); user approved the spec with this fix
- 2026-10-06T19:20:14Z (design/tasks-7580d2): parked (waiting on user, review): user reviews docs/plans/2026-10-06-project-resolution.md and picks subagent-driven or native execution; then the agent starts tasks-b1dbc5 in .worktrees/tasks-7580d2
  provenance: {"harness_session":"claude-code:00da167b-561d-40c6-aa34-7a035e885cf2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T19:27:06Z (design/tasks-7580d2): resumed
  provenance: {"harness_session":"claude-code:00da167b-561d-40c6-aa34-7a035e885cf2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T19:27:06Z (design/tasks-7580d2): review: plan round 1 — verdict: revise; findings: Important 2, Minor 2; reviewer: agent (tack session; harness/model unstated)
- 2026-10-06T19:28:50Z (design/tasks-7580d2): plan round 1 addressed: merge leaves the storage record empty and the displaced record travels in Displaced; a dangling link after a root change clears the record and warns (spec §2.1/§2.2 amended); second-host test adopts before its storage move; Task 6 hands the other host its install, pre-move init --force, and ai backfill in that order; clean-tree check after init --force. Execution: subagent-driven (agreed by reviewer)
- 2026-10-06T19:28:52Z (design/tasks-7580d2): parked (waiting on user, review): user re-reviews docs/plans/2026-10-06-project-resolution.md (round 2); on approval the agent runs it subagent-driven from tasks-b1dbc5 in .worktrees/tasks-7580d2
  provenance: {"harness_session":"claude-code:00da167b-561d-40c6-aa34-7a035e885cf2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T19:42:34Z (design/tasks-7580d2): resumed
  provenance: {"harness_session":"claude-code:00da167b-561d-40c6-aa34-7a035e885cf2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T19:42:34Z (design/tasks-7580d2): review: plan round 2 — verdict: accept; findings: none; reviewer: agent (tack session; harness/model unstated)
- 2026-10-06T19:42:34Z (design/tasks-7580d2): parked (waiting on user, approval): plan accepted (round 2); user paused before execution. On the user's go, the agent runs docs/plans/2026-10-06-project-resolution.md subagent-driven, starting tasks-b1dbc5 in .worktrees/tasks-7580d2
  provenance: {"harness_session":"claude-code:00da167b-561d-40c6-aa34-7a035e885cf2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
