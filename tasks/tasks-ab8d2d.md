---
id: tasks-ab8d2d
title: "Design which checkout owns a task record for writes and reads, from the brief"
status: done
priority: 2
size: m
complexity: high
process: planned
owner: main
created: 2026-09-29T20:50:01Z
updated: 2026-09-30T10:28:06Z
started: 2026-09-30T09:54:52Z
completed: 2026-09-30T10:28:06Z
depends: []
parent: tasks-c4ad8e
tags: [worktree]
spec: docs/specs/2026-09-30-record-home-design.md
---

Question: which checkout is a task record's home, and what do writes and reads from other checkouts do? Where to start: docs/notes/2026-09-29-cross-checkout-records-brief.md (alternatives 1-2), warn_on_newer_sibling_copies in src/commands/mod.rs, Claim/Park worktree fields in src/claims.rs, resolve_recorded in src/commands/parked.rs, and work-claims design §Warnings/§Known gaps. Bound: a reviewed design spec that settles refuse vs warn for writes from a non-home checkout (reverses the documented 'signal, not a gate' rule, so the user decides), how home moves to a worktree created after start, and whether show routes / plain list includes worktree-only tasks (JSON shape). No implementation. Expected result: a spec in docs/specs/ linked with --spec, and a note here with the decisions. Ideas it wakes: on completion, tasks note tasks-2c0a1d, tasks-bb53e5, and tasks-fbc32b with the decisions, in the same commit.

## Notes

- 2026-09-29T20:56:10Z (main): Current lean (agent, 2026-09-29; user undecided): a home checkout = the worktree named by the task's live claim or park; start is the handoff (re-start in the new worktree moves the claim) and refuses when a sibling copy is newer, so the handoff cannot drop main's writes; any other write from a non-home checkout refuses with a typed error naming the home and the exact 'tasks -C <home> ...' retry, never auto-routing; with no claim or park there is no home and the newer-sibling rule stays a warning. show falls back to the home copy with a warning; plain list stays local. See brief §Alternatives.
- 2026-09-30T09:54:52Z (main): started
  provenance: {"harness_session":"claude-code:2387b84d-f415-4946-9038-1bb62ec95101","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T10:01:51Z (ab8d2d-record-home): User decided 2026-09-30: a write whose copy is behind another checkout's refuses, for every task, with the checkout named and a tasks -C retry; no override flag. Reverses the work-claims 'signal, not a gate' rule.
- 2026-09-30T10:03:54Z (ab8d2d-record-home): parked (waiting on user, review): User reviews docs/specs/2026-09-30-record-home-design.md in .worktrees/ab8d2d-record-home; on approval the agent records the review note, closes tasks-ab8d2d with notes on tasks-2c0a1d/bb53e5/fbc32b, then runs writing-plans for the implementation
  provenance: {"harness_session":"claude-code:2387b84d-f415-4946-9038-1bb62ec95101","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T10:11:26Z (ab8d2d-record-home): review: spec round 1 — verdict: revise; findings: P2 3; reviewer: codex
- 2026-09-30T10:11:34Z (ab8d2d-record-home): Spec review details: §3.1 needs a preflight before attach reads stdin/clipboard or writes payloads; save runs only after payload creation and rollback can fail. §3.2 cannot promise an exact argv-only retry for edit --body - or attach - after stdin was consumed; define replay/recovery guidance and test it. §4 must reuse Ctx::ownership (ByIdentity and ByProof), not require matching resolved session identity; current note heartbeat already accepts proof-based ownership. Design and implementation unchanged.
- 2026-09-30T10:13:10Z (ab8d2d-record-home): Spec revised for round 1: the stale-copy check runs at load under the mutation lock (before input or files), again after the editor re-locks; stdin retries ask for the same input; holder = Ctx::ownership non-Foreign, ByProof included.
- 2026-09-30T10:13:10Z (ab8d2d-record-home): parked (waiting on user, review): User re-reviews docs/specs/2026-09-30-record-home-design.md (round 2) in .worktrees/ab8d2d-record-home; on approval the agent records the review note, closes tasks-ab8d2d with notes on tasks-2c0a1d/bb53e5/fbc32b, then runs writing-plans
  provenance: {"harness_session":"claude-code:2387b84d-f415-4946-9038-1bb62ec95101","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T10:22:24Z (ab8d2d-record-home): review: spec round 2 — verdict: accept; findings: none; reviewer: codex
- 2026-09-30T10:28:06Z (ab8d2d-record-home): Decisions (spec record-home, accepted round 2): a write from a copy behind a sibling refuses as stale_copy for every task, checked at load under the mutation lock, with a tasks -C retry; the claim's worktree follows every write by its holder (Ctx::ownership non-Foreign); protocol step: tasks start <id> first in a new worktree; show falls back to the claim's or park's checkout; list stays local, no JSON shape change.
- 2026-09-30T10:28:06Z (ab8d2d-record-home): done
  provenance: {"harness_session":"claude-code:2387b84d-f415-4946-9038-1bb62ec95101","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-30T10:28:06Z (ab8d2d-record-home): Design spec docs/specs/2026-09-30-record-home-design.md accepted after two review rounds; implementation filed as tasks-9949f3
  provenance: {"harness_session":"claude-code:2387b84d-f415-4946-9038-1bb62ec95101","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
