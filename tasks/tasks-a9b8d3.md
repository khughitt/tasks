---
id: tasks-a9b8d3
title: Design explicit idea review state from the work-selection brief
status: doing
priority: 2
size: m
complexity: high
process: planned
owner: design/tasks-a9b8d3-review-state
created: 2026-10-08T10:53:28Z
updated: 2026-10-11T13:40:50Z
started: 2026-10-11T12:11:58Z
depends: []
parent: tasks-46d207
tags: []
source: tasks-e34b99
agent: codex
---

Why: tasks-e34b99 requests visible idea-review state, and its 2026-10-08 user input chooses a top-level boolean and a left-most * marker for unread ideas. Scope currently rereads the latest note to determine eligibility.
Where to start: docs/notes/2026-09-30-work-selection-brief.md; tasks-e34b99 original report and user-input note; src/model.rs, src/frontmatter.rs, src/cli.rs, src/filter.rs, src/output.rs, src/commands/edit.rs, src/commands/note.rs, src/commands/sample.rs, skills/scope/SKILL.md and skills/curate/SKILL.md.
Bound: Design the smallest explicit reviewed flag, list filtering and pretty marker contract. Preserve the requested boolean direction; distinguish the user's choice from the agent's proposed reviewed name, explicit wake reset, and one-time latest-note backfill. Settle unset/false/true serialization, non-idea transitions and return to idea, atomic note-plus-reset behavior, safe rerunnable migration, and pending drop proposals versus parks. Do not equate idea review with design/plan approval or executable status. Identify affected JSON consumers, fixtures, CLI completion, skills and docs; coordinate eligibility semantics with tasks-91c622 without expanding into cross-project discovery.
Done: A written spec reviewed by the user, followed by a reviewed implementation plan. Acceptance examples cover new, briefed, explicitly readmitted and repeatedly reviewed ideas; incidental later notes; pending proposals; non-idea transitions; migration reruns; JSON/filter/pretty agreement; and stale-copy/live-claim refusal. No implementation or live backfill during this design task.
Ideas it wakes: On completion, record the approved finding with tasks note on tasks-e34b99 and update the brief in the same commit. Use the current note-based wake rule until an explicit review-state contract has actually shipped.

## Notes

- 2026-10-11T12:11:58Z (main): started
  provenance: {"harness_session":"claude-code:9761865f-f4ce-4ed0-a4e3-b8a7a1ee1115","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-11T12:12:11Z (design/tasks-a9b8d3-review-state): resumed
  provenance: {"harness_session":"claude-code:9761865f-f4ce-4ed0-a4e3-b8a7a1ee1115","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-11T12:22:00Z (design/tasks-a9b8d3-review-state): spec drafted: docs/specs/2026-10-11-review-state-design.md. User 2026-10-11 chose the marker scope: ideas (stored reviewed flag, set by note/edit --reviewed, cleared by --no-reviewed and every status transition) plus leaf todo/blocked with no process (derived needs_review); goals and doing never marked. Rejected note-derived marks and a review-state enum. Backfill via CLI per project; proposals stay note-based.
- 2026-10-11T12:22:00Z (design/tasks-a9b8d3-review-state): parked (waiting on user, review): User reviews docs/specs/2026-10-11-review-state-design.md (worktree .worktrees/tasks-a9b8d3-review-state, branch design/tasks-a9b8d3-review-state); then the agent revises it or writes the implementation plan
  provenance: {"harness_session":"claude-code:9761865f-f4ce-4ed0-a4e3-b8a7a1ee1115","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-11T12:44:27Z (design/tasks-a9b8d3-review-state): review: spec round 1 — verdict: revise; findings: P1 2, P2 2; reviewer: codex
- 2026-10-11T12:47:13Z (design/tasks-a9b8d3-review-state): resumed
  provenance: {"harness_session":"claude-code:9761865f-f4ce-4ed0-a4e3-b8a7a1ee1115","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-11T12:47:13Z (design/tasks-a9b8d3-review-state): spec revised for round 1: backfill keeps a per-project ledger of set ids and retries only unreviewed, unledgered candidates (no record rewritten twice; edit --no-reviewed readmission survives reruns); every host reading synced records runs the new binary before the first reviewed write, with a README line beside move's; same-status editor saves refuse reviewed on a non-idea; parked views get reviewed/needs_review on ParkedRow (None when unresolved), the marker column, and --parked --needs-review via Fields::of_row; acceptance 8, 14, 15 added or extended
- 2026-10-11T12:47:13Z (design/tasks-a9b8d3-review-state): parked (waiting on user, review): User (or reviewer) re-reviews the revised docs/specs/2026-10-11-review-state-design.md in .worktrees/tasks-a9b8d3-review-state; on acceptance the agent writes the implementation plan
  provenance: {"harness_session":"claude-code:9761865f-f4ce-4ed0-a4e3-b8a7a1ee1115","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-11T12:54:29Z (design/tasks-a9b8d3-review-state): review: spec round 2 — verdict: revise; findings: P1 1; reviewer: codex
- 2026-10-11T12:55:55Z (design/tasks-a9b8d3-review-state): resumed
  provenance: {"harness_session":"claude-code:9761865f-f4ce-4ed0-a4e3-b8a7a1ee1115","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-11T12:56:01Z (design/tasks-a9b8d3-review-state): reviewer correction (user, 2026-10-11): the spec round 2 reviewer is codex/gpt-6.1-sol; the round 2 review line above records the harness alone. Round 1's model is not confirmed. Notes cannot be amended (tasks-d56b72).
- 2026-10-11T12:56:26Z (design/tasks-a9b8d3-review-state): spec revised for round 2: the backfill freezes its candidates and their updated stamps once, applies the flag only to entries still unreviewed ideas with an unchanged stamp, reports changed entries instead of re-deriving eligibility from notes; acceptance 14 covers the interrupted-run and post-freeze-scope sequences
- 2026-10-11T12:56:26Z (design/tasks-a9b8d3-review-state): parked (waiting on user, review): Reviewer (codex/gpt-6.1-sol) or the user re-reviews §7.2 and acceptance 14 of docs/specs/2026-10-11-review-state-design.md in .worktrees/tasks-a9b8d3-review-state; on acceptance the agent writes the implementation plan
  provenance: {"harness_session":"claude-code:9761865f-f4ce-4ed0-a4e3-b8a7a1ee1115","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-11T13:05:38Z (design/tasks-a9b8d3-review-state): review: spec round 3 — verdict: revise; findings: P2 1; reviewer: codex
- 2026-10-11T13:07:23Z (design/tasks-a9b8d3-review-state): resumed
  provenance: {"harness_session":"claude-code:9761865f-f4ce-4ed0-a4e3-b8a7a1ee1115","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-11T13:07:23Z (design/tasks-a9b8d3-review-state): spec revised for round 3: §7.2 requires a writer pause on the project, across reading hosts, for every backfill apply pass (first run and reruns), checked before each pass; a stamp-checked edit in the CLI rejected as surface a one-time migration does not justify
- 2026-10-11T13:07:23Z (design/tasks-a9b8d3-review-state): parked (waiting on user, review): Reviewer or user re-reviews the §7.2 writer-pause bullet of docs/specs/2026-10-11-review-state-design.md in .worktrees/tasks-a9b8d3-review-state; on acceptance the agent writes the implementation plan
  provenance: {"harness_session":"claude-code:9761865f-f4ce-4ed0-a4e3-b8a7a1ee1115","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-11T13:09:03Z (design/tasks-a9b8d3-review-state): review: spec round 4 — verdict: revise; findings: P2 1; reviewer: codex
- 2026-10-11T13:25:37Z (design/tasks-a9b8d3-review-state): resumed
  provenance: {"harness_session":"claude-code:9761865f-f4ce-4ed0-a4e3-b8a7a1ee1115","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-11T13:25:49Z (design/tasks-a9b8d3-review-state): spec revised for round 4: the pause check is tasks claims on each reading host (no live claim in the project) plus the operator's explicit confirmation that every other record writer is paused, since scope, curate and plain notes write without claims; host-load and tasks list --status doing removed as evidence; parking is no longer named as a way to pause a session
- 2026-10-11T13:25:49Z (design/tasks-a9b8d3-review-state): parked (waiting on user, review): Reviewer or user re-reviews the §7.2 pause checklist of docs/specs/2026-10-11-review-state-design.md in .worktrees/tasks-a9b8d3-review-state; on acceptance the agent writes the implementation plan
  provenance: {"harness_session":"claude-code:9761865f-f4ce-4ed0-a4e3-b8a7a1ee1115","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-11T13:26:39Z (design/tasks-a9b8d3-review-state): review: spec round 5 — verdict: accept; findings: none; reviewer: codex
- 2026-10-11T13:27:08Z (design/tasks-a9b8d3-review-state): resumed
  provenance: {"harness_session":"claude-code:9761865f-f4ce-4ed0-a4e3-b8a7a1ee1115","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-11T13:27:08Z (design/tasks-a9b8d3-review-state): User 2026-10-11: spec accepted for planning (after reviewer round 5 accept).
- 2026-10-11T13:40:49Z (design/tasks-a9b8d3-review-state): plan drafted: docs/plans/2026-10-11-review-state.md, six tasks (record field, note/edit writers, derived needs_review + filter, pretty marker + show, skills/README, backfill tool) plus an operator rollout. Plan-level deviations folded into the spec by its own tasks: the mark is an id-cell prefix (prime shares id width through column 0), the backfill refuses a whole pass under a live local claim, and auto-commit projects commit per edit.
- 2026-10-11T13:40:49Z (design/tasks-a9b8d3-review-state): parked (waiting on user, review): User (or reviewer) reviews docs/plans/2026-10-11-review-state.md in .worktrees/tasks-a9b8d3-review-state and picks an execution method; on acceptance file the implementation task named in the plan's Tracking section, note the finding on tasks-e34b99, and close tasks-a9b8d3
  provenance: {"harness_session":"claude-code:9761865f-f4ce-4ed0-a4e3-b8a7a1ee1115","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
