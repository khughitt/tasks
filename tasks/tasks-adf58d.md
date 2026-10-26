---
id: tasks-adf58d
title: "A recurring task cannot be given a first due date: add refuses --defer together with --every"
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-26T15:36:15Z
created: 2026-10-26T14:47:22Z
updated: 2026-10-26T15:36:15Z
depends: [tasks-ba4528]
parent: tasks-46d207
tags: [feedback, gap, "from:obs"]
agent: claude-code/claude-opus-5-5
origin: agent
---

Action: filing a monthly reassessment task whose first check should fall a month from now, using add --every 30d --defer <date>. The add was refused because defer and every cannot both be set, and a new recurrence is ready immediately. Expected a way to start a recurrence on a chosen date. Workaround: a plain deferred task whose body tells each occurrence to reset its status and re-defer by 30 days, in two commands.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-26-first-recurrence-date-brief.md — first-occurrence alternatives and open lifecycle decisions

## Notes

- 2026-10-26T14:47:22Z (feedback): created
  provenance: {"harness_session":"claude-code:b0f7fde8-95d2-4d12-a287-5daf820d7b6b","harness_session_source":"CLAUDE_CODE_SESSION_ID","origin":"agent"}
- 2026-10-26T15:36:15Z (main): scope: briefed; 0845def src/format.rs validation, periodic::due/is_due, query deferral and defer-refusal integration assertions confirm deliberate every/defer exclusion; first-occurrence lifecycle and old-reader behavior need design; reuse lane tasks-46d207 and wait on tasks-ba4528 (P2/s/high/planned), with no redundant research task; origin: filing inferred, claude-code:b0f7fde8-95d2-4d12-a287-5daf820d7b6b lines 620-621, unique matching feedback call at 14:47:21.922Z and filtered result tasks-adf58d created; lines 592-593 record the actual add refusal and 608 the workaround; human line 509 requests monthly reassessment, not a new CLI capability, so origin agent retained; no origin_session, older-request stage not applicable; brief: docs/notes/2026-10-26-first-recurrence-date-brief.md
  provenance: {"harness_session":"codex:01a178d6-0f5f-7d01-8ac1-a01cfe262c14","harness_session_source":"CODEX_THREAD_ID"}
