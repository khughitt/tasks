---
id: tasks-a58a90
title: Design task creation and feedback occurrence provenance from the brief
status: doing
priority: 2
size: m
complexity: high
process: planned
owner: design/tasks-a58a90
created: 2026-10-20T14:40:20Z
updated: 2026-10-25T11:31:44Z
started: 2026-10-25T11:15:47Z
depends: []
parent: tasks-9b1c2b
tags: []
source: "tasks-516ab4:provenance-design"
agent: claude-code/claude-opus-5-5
spec: docs/specs/2026-10-25-creation-occurrence-provenance-design.md
---

Why: [ref:brief] establishes that feedback creation and recurrence are unstamped and that a scope pass reconstructs a report's origin by hand; occurrence records, origin distinctions and retrieval need one design. Split from tasks-441019 on 2026-10-20 so the drop-step fix, which the audit shows carries most of the losses, does not wait on retrieval work.

Done: produce a user-reviewed design and implementation plan that (1) stamps each creation and recurrence separately while distinguishing reporting session, older origin session and inferred call anchors, preserving a short tool-only summary; (2) visibly marks, at filing, a report that relays the user's own words, coordinating lore-c01bc8's capture guidance; (3) defines scope retrieval of request/action/result/later correction and checks live, archived, subagent-parent and other-host retrieval or explicit unavailable limitations, coordinating hq-c411df rather than duplicating its reader work; (4) retains the blind at-filing pilot: a fresh agent restricted to the reporting session, against records as they stood at filing, recovers each distinct ask and names the task that covers it. obs-c5c599 owns cross-tool tracing. tasks-441019 owns the drop step and the post-change audit and does not wait on this design; it judges user relay from report text until the marker in (2) exists. Do not assume session stamps alone solve drop-time losses.

Where to look: [ref:brief]; tasks-516ab4 notes (retrieval workarounds from the audit: 9 origins found only through the CLI's created output, 6 Codex subagents through their parent thread, 1 unrecoverable); src/commands/feedback.rs, src/commands/mod.rs::append_stamped_note, src/provenance.rs; skills/scope/SKILL.md's evidence step; the owner records named above. No implementation before written design and plan reviews.

Additional acceptance from tasks-2e7d03 (2026-10-25): include ordinary tasks add in the same creation/session and requester/origin contract, retaining all feedback requirements above. At 79b8253, add::blank is the shared constructor with empty notes, add::run and commands::create add no native provenance, and a sourced duplicate returns reused without writing. Reuse the native resolver; compare a creation note with dedicated metadata rather than assuming storage. Specify an optional origin indication that distinguishes an explicit user request from agent-originated follow-ups, feedback, decomposition and scope filings, with mixed/unknown evidence preserved. A session stamp identifies the filer, not the requester; never derive user intent from agent, parent, source or commit time alone. Settle who supplies/corrects the indication and how an older request is linked. Preserve source and existing attribution; no speculative backfill; reused adds must not stamp the later caller as creator. Acceptance must include ordinary creation with known/unknown/conflicting native identity, idempotent reuse, and a later agent recovering the originating ask or an explicit unavailable result without relying on commit time. Relevant surfaces: src/commands/add.rs, the shared constructor used by feedback, src/commands/mod.rs, src/provenance.rs, record serialization and JSON/pretty readers, tests/cli.rs, README.md and skills/tasks/SKILL.md; any new flags require ops CLI rows first. [ref:brief] records these additions and open storage/vocabulary choices. The task remains P2/m/high/planned; design and plan review precede implementation.

Ideas it wakes: On completion, run `tasks note tasks-516ab4 --no-reviewed "<finding>"` and `tasks note tasks-2e7d03 --no-reviewed "<finding>"` before this result branch merges.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-20-feedback-ask-preservation-brief.md — evidence, distinct asks, alternatives and ownership boundaries

## Notes

- 2026-10-25T11:15:47Z (main): started
  provenance: {"harness_session":"claude-code:900c1161-c4b1-4ba6-8bca-efadba4b94c9","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-25T11:16:18Z (design/tasks-a58a90): resumed
  provenance: {"harness_session":"claude-code:900c1161-c4b1-4ba6-8bca-efadba4b94c9","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-25T11:30:34Z (design/tasks-a58a90): review: spec round 1 — verdict: revise; findings: questions 4; reviewer: claude-code
- 2026-10-25T11:30:35Z (design/tasks-a58a90): spec drafted in .worktrees/tasks-a58a90 (docs/specs/2026-10-25-creation-occurrence-provenance-design.md): session stamp on an always-written created note and stamped feedback recurrences; origin user|agent|mixed as a correctable frontmatter field plus per-occurrence evidence; origin_session and Codex root_session on occurrence notes; filing sessions recovered: 2e7d03 was agent-filed from the user's own question in claude-code:572ee51e-2f53-4729-96f8-f122a2cc8305 (mixed origin); the 2026-10-25 extension came from a $scope pass with no further user words (codex:01a172c0-9891-77c2-9b4c-b673b0f1633d)
  provenance: {"harness_session":"claude-code:900c1161-c4b1-4ba6-8bca-efadba4b94c9","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-25T11:31:44Z (design/tasks-a58a90): parked (waiting on user, review): user reviews the spec at design/tasks-a58a90 506253d:docs/specs/2026-10-25-creation-occurrence-provenance-design.md; on approval the agent writes the implementation plan in .worktrees/tasks-a58a90 (worktree hydrated, baseline deferred to execution setup)
  provenance: {"harness_session":"claude-code:900c1161-c4b1-4ba6-8bca-efadba4b94c9","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
