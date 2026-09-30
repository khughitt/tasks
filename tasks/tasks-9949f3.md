---
id: tasks-9949f3
title: "Implement the record-home design: stale-copy refusal, claim follows holder, show fallback"
status: doing
priority: 2
size: l
complexity: high
process: planned
owner: ab8d2d-record-home
created: 2026-09-30T10:28:06Z
updated: 2026-09-30T10:40:26Z
started: 2026-09-30T10:28:52Z
depends: []
parent: tasks-c4ad8e
tags: [worktree]
agent: claude-code/claude-opus-5-5
spec: docs/specs/2026-09-30-record-home-design.md
---

Implements docs/specs/2026-09-30-record-home-design.md (accepted 2026-09-30, tasks-ab8d2d). The spec is approved; the implementation plan still needs review before code. Closes tasks-2c0a1d, tasks-bb53e5 and the show half of tasks-fbc32b.

## Notes

- 2026-09-30T10:28:41Z (ab8d2d-record-home): Evidence 2026-09-30: closing tasks-ab8d2d, a note on tasks-bb53e5 from this worktree forked the record against a newer committed note in main (the warning fired after writing); resolved by merging main and keeping both notes. Under the spec it would have refused as stale_copy.
- 2026-09-30T10:28:52Z (ab8d2d-record-home): started
  provenance: {"harness_session":"claude-code:2387b84d-f415-4946-9038-1bb62ec95101","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T10:39:06Z (ab8d2d-record-home): Correction: the body's 'accepted 2026-09-30' overstates the spec's state; round 2 was codex's accept, and the user returned round 3 (revise). The spec is revised here before this plan proceeds; the plan draft is on hold until the user re-reviews.
- 2026-09-30T10:40:26Z (ab8d2d-record-home): Spec revised for round 3: remedy ladder in §3.2 (another session's claim or park names the newer worktree → no -C retry, name it; same stamp different bytes → refuse, merge only; linked worktree behind main → merge remedy first, -C as the alternative; otherwise -C), decision 2 wording, same-second content rule, cross-project show fallback, §8 widened, §10 tests.
