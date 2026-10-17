---
id: tasks-1a3280
title: Design wait-aware review state from the brief
status: doing
priority: 2
size: m
complexity: high
process: planned
owner: feat/wait-aware-review
created: 2026-10-17T09:19:37Z
updated: 2026-10-17T10:50:45Z
started: 2026-10-17T10:02:52Z
depends: []
parent: tasks-5408fe
tags: []
source: docs/notes/2026-10-17-scope-queue-stranded-ideas-brief.md
agent: claude-code/claude-opus-5-5
spec: docs/specs/2026-10-17-wait-aware-review-design.md
plan: docs/plans/2026-10-17-wait-aware-review.md
---

Question: How should the scope queue tell a reviewed idea that still waits on something recorded from one that nothing will wake, so the stranded ones come back without a waker's note?
Where to start: docs/notes/2026-10-17-scope-queue-stranded-ideas-brief.md (measurements, alternatives, lean 3); docs/specs/2026-10-11-review-state-design.md sections 1, 3.3, 4, 6, 7.2; src/review.rs::needs_review; TaskSummary::of; src/commands/sample.rs::pending_proposal and tasks-22f407; skills/scope/SKILL.md sections 3 and 5; tasks-d4e73d for a done-time wake.
Bound: a design spec for user review covering the rule (what counts as a recorded wait: open edge, pending proposal, user park), its JSON meaning and cost, the scope skill rule, and a backfill for legacy prose waits. Take the user's answers to the brief's four questions (2026-10-17) as inputs; choose the synced wait-on-user marker and the test that tells a newly woken idea from a legacy stranded one. No implementation.
Expected result: a reviewed spec under docs/specs/ attached with --spec, the brief updated with the decisions, and the implementation decomposed under this task's parent.
Ideas it wakes: on completion, run `tasks note tasks-163055 --no-reviewed "<decision>"` before this result's branch merges.

## Notes

- 2026-10-17T09:32:22Z (main): brief revised after the user's review (2026-10-17): ops-1a552c is a missed wake through a recorded edge (ops dc0a36c added it; hq-a558e8 closed 2026-10-09 with no wake), which favours option 3; measurement time and the 09:25Z ops drops recorded; no needs_review reader outside tasks; the user answered all four questions (no park as a wait, proposals count via the shared helper, two tiers, per-project backfill)
- 2026-10-17T09:48:10Z (main): brief revised again (2026-10-17): ops-5bb3e5's wake was written as a plain note by ops-36e093 (a live plain-note case), not missing; the user proposed a synced reviewed_at stamped by --reviewed, with 'freshly woken' = a dependency completed after reviewed_at; open: completed is stamped only on done, so drops need their own close time or count as no new evidence
- 2026-10-17T09:53:56Z (main): user ruling 2026-10-17: a dropped dependency is new evidence (stamp a close time on done or dropped, as completed or a separate closed; done unchanged); a moved dependency is followed to its successor, not treated as closed (Status::is_open counts moved as closed; prefix renames write no tombstones). Recorded in the brief under question 3
- 2026-10-17T09:59:59Z (main): user input 2026-10-17: drops stamp a new closed field (done and dropped both set it; completed stays done-only; the wake rule reads closed). Checked readers: ops obs-index lead time is done-only (safe); obs views read completed_ms as a close time including drops and already miss dropped tasks (obs-1d6eb4), so a separate closed lets obs adopt it deliberately. Recorded in the brief
- 2026-10-17T10:02:52Z (main): started
  provenance: {"harness_session":"claude-code:a8bb9133-4b61-424c-8a52-80795ab6c626","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T10:03:00Z (feat/wait-aware-review): resumed
  provenance: {"harness_session":"claude-code:a8bb9133-4b61-424c-8a52-80795ab6c626","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T10:08:35Z (feat/wait-aware-review): spec drafted (b890fdb): marker = synced awaiting_user bool on reviewed ideas; waits = open dep (moves followed, unreachable counts open) | pending proposal | awaiting_user | future defer (added: else deferred reviewed ideas strand on day one); review_reason unreviewed|woken|stranded|unassessed; stranded drawn before unassessed todos; backfill report+apply, no writer pause
- 2026-10-17T10:08:37Z (feat/wait-aware-review): parked (waiting on user, review): user reviews docs/specs/2026-10-17-wait-aware-review-design.md in .worktrees/wait-aware-review; on approval the agent writes the implementation plan, updates the brief with the decisions, and notes tasks-22f407 with the awaiting_user recommendation
  provenance: {"harness_session":"claude-code:a8bb9133-4b61-424c-8a52-80795ab6c626","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T10:25:05Z (feat/wait-aware-review): resumed
  provenance: {"harness_session":"claude-code:a8bb9133-4b61-424c-8a52-80795ab6c626","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T10:25:06Z (feat/wait-aware-review): review: spec round 1 — verdict: revise; findings: must-fix 3; reviewer: unverified (relayed by user)
- 2026-10-17T10:26:33Z (feat/wait-aware-review): spec revised for round 1: backfill freezes decisions with updated stamps and applies under the review-state 7.2 writer pause (atomic --if-updated rejected as permanent surface); dependency views bound to their scan, memo keyed by checkout and id, parked rows follow their recorded checkout; equal closed/reviewed_at counts as woken, defer test stays strict; acceptance 14-16
- 2026-10-17T10:26:34Z (feat/wait-aware-review): parked (waiting on user, review): user reviews the revised docs/specs/2026-10-17-wait-aware-review-design.md in .worktrees/wait-aware-review (round 2); on approval the agent writes the implementation plan, updates the brief, and notes tasks-22f407
  provenance: {"harness_session":"claude-code:a8bb9133-4b61-424c-8a52-80795ab6c626","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T10:35:24Z (feat/wait-aware-review): resumed
  provenance: {"harness_session":"claude-code:a8bb9133-4b61-424c-8a52-80795ab6c626","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T10:35:25Z (feat/wait-aware-review): review: spec round 2 — verdict: revise; findings: must-fix 1; reviewer: unverified (relayed by user)
- 2026-10-17T10:35:58Z (feat/wait-aware-review): spec round 2 fix landed (86ef756): acceptance 16 split per the review (A in main follows registered copy and waits; B only in W is woken; both in one command); review said no further design blockers, so the spec is taken as accepted and planning starts
