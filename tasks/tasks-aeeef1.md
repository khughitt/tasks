---
id: tasks-aeeef1
title: Implement wait-aware review state
status: doing
priority: 2
size: l
complexity: high
process: planned
owner: main
created: 2026-10-17T11:25:02Z
updated: 2026-10-17T17:37:47Z
started: 2026-10-17T12:58:26Z
depends: []
parent: tasks-5408fe
tags: []
source: tasks-1a3280
agent: claude-code/claude-opus-5-5
spec: docs/specs/2026-10-17-wait-aware-review-design.md
plan: docs/plans/2026-10-17-wait-aware-review.md
---

Why: reviewed ideas that wait on nothing recorded never return to the scope queue (tasks-163055; brief docs/notes/2026-10-17-scope-queue-stranded-ideas-brief.md).
Done: every step below lands; the plan's Rollout has run on every reading host and registered project.
Where: spec docs/specs/2026-10-17-wait-aware-review-design.md (accepted after spec review round 2); plan docs/plans/2026-10-17-wait-aware-review.md (accepted after plan review round 2, codex/gpt-6.1-sol). Execute subagent-driven, steps in order, read-only reviewers.

## Notes

- 2026-10-17T12:31:12Z (feat/wait-aware-review): review: impl round 1 — verdict: revise; findings: critical 1, minor 4; reviewer: claude-code/claude-opus-5-5
- 2026-10-17T12:37:54Z (feat/wait-aware-review): review: impl round 2 — verdict: accept; findings: none; reviewer: claude-code/claude-sonnet-5-5
- 2026-10-17T12:37:55Z (feat/wait-aware-review): parked (waiting on user, approval): User picks integration for feat/wait-aware-review (.worktrees/wait-aware-review, all 7 steps done, final review clean, suite green). Merging is Rollout step 1: pause writers on every host, merge, cargo install --path . on each host, confirm tasks note --help lists --awaiting-user, then resume writers; the agent then runs Rollout steps 2-6 with the user ruling on judge entries
  provenance: {"harness_session":"claude-code:f4b45253-a492-455b-872d-fec9d743d492","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T12:46:15Z (main): attached: wait-aware-review-ledger.md (11201 bytes): SDD ledger for the wait-aware review plan: pre-flight scan, rulings, per-task and final review outcomes, deferred minors (filed as tasks-d2cdfe)
- 2026-10-17T12:52:17Z (main): parked (waiting on user, review): User reviews ~/.local/state/tasks/wait-backfill/judge-review.md (67 judge entries J1-J67 with proposals) and replies with only the entries to change; the agent then writes the decisions (judge-proposals.json plus overrides) into the per-project snapshots and, once the user confirms writers are paused and other hosts run the new binary, runs Rollout step 4 apply per project (no live claims), then steps 5-6. Done so far: merged 19974fe, binary installed on titan, freeze run for all 33 projects (139 candidates: 72 auto, 67 judge); auto mind6-573a5b overridden to leave (dep on own ancestor, tasks-9c18e7).
  provenance: {"harness_session":"claude-code:f4b45253-a492-455b-872d-fec9d743d492","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T12:58:26Z (main): started
  provenance: {"harness_session":"claude-code:f4b45253-a492-455b-872d-fec9d743d492","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T12:58:44Z (main): rollout step 3: user ruled on judge-review.md (11 overrides: J6,J7,J10,J32,J42,J45,J63 narrowed dep; J12,J14,J34,J61 awaiting_user); decisions written to all snapshots: 67 judge entries decided
- 2026-10-17T13:04:18Z (main): rollout step 4 (partial, user-approved idle projects): applied/left/changed — beliefs 2/1/1 (beliefs-c800ef), fam 1/1/0, forge 2/0/0, hq 3/0/2 (hq-48c5c6, hq-5b8413), ns 3/0/0, ops 22/0/0, tui 0/1/0, wali 4/0/0; step 5: all eight committed, git status clean under tasks/
- 2026-10-17T13:04:20Z (main): parked (waiting on agent): Agent applies the remaining six snapshots once each project shows no live claim in tasks claims: material, mind6, obs, prism, relay, tasks (this goal's own claim is released by this park). From each project's root: python3 -I <tasks root>/tools/wait-backfill --state ~/.local/state/tasks/wait-backfill --root <root> --writers-paused; then git status --porcelain -- tasks/ there (tasks commit if needed), note counts here, append 'rollout complete <date>' to the spec status line, done the goal. Other hosts reading synced records must run the new binary (user).
  provenance: {"harness_session":"claude-code:f4b45253-a492-455b-872d-fec9d743d492","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T17:34:33Z (main): resumed
  provenance: {"harness_session":"claude-code:f4b45253-a492-455b-872d-fec9d743d492","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T17:37:27Z (main): parked (waiting on agent): Agent is applying the tasks project snapshot now (claim released for the tool's live-claim check); resume with start and finish the rollout
  provenance: {"harness_session":"claude-code:f4b45253-a492-455b-872d-fec9d743d492","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T17:37:46Z (main): resumed
  provenance: {"harness_session":"claude-code:f4b45253-a492-455b-872d-fec9d743d492","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
