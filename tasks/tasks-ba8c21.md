---
id: tasks-ba8c21
title: Explain the idea-to-ready lifecycle in the README
status: todo
priority: 2
size: s
complexity: low
process: direct
created: 2026-10-25T18:15:24Z
updated: 2026-10-26T15:34:19Z
depends: []
parent: tasks-6ed79b
tags: [feedback, gap, "from:lore"]
agent: claude-code
origin: agent
---

Why: readers cannot find a single overview of how captured ideas become executable work across sessions. The current README describes fields, while the scope skill contains the procedure. Source inspection at 0845def confirms the report; the filing session's user asked how the stages differ and where the process is documented, while the proposed README addition was the agent's remedy.

Done: add one short reader-facing overview in README.md, with a link from Agent skills and a link to skills/scope/SKILL.md for the procedure. Explain capture as idea; scope as evidence review and a verdict; when unresolved decisions need a brief; when a bounded unanswered question needs an ordinary research task; research pickup through ready/next and start (no dedicated research command); dependencies and finding notes with --no-reviewed; a later /scope that reuses the handoff; and re-scoping todo/blocked work before implementation when premises change. Preserve the distinction between scoping and doing research, and between scoped direct work and planned work's design/plan reviews. Do not add commands, change the workflow, or duplicate the full skill.

Where to look: README.md Statuses, Process and workspace, and Agent skills; skills/scope/SKILL.md sections 2-7; skills/tasks/SKILL.md Session protocol and Process and workspace. Existing documentation-review task tasks-d5ddb4 records audits rather than implementing this specific overview.

Verification: walk the overview against the current scope and tasks skills; a reader can answer what scoping does, when briefs/research are needed, how research runs, what wakes an idea, and what the next /scope does. Check local links and run just check.

Original report (preserved):
A user asked how scoping differs from research, what decides when a brief or a research task is written, and whether research has its own command. The answers exist only inside skills/scope/SKILL.md's agent procedure, spread across its verdict, follow-up and rerun sections. The README's Statuses section covers the reviewed flag and review_reason values, and its Agent skills section gives /scope one sentence. The word 'research' appears in neither the README nor the tasks skill, and nothing says that research tasks are ordinary ready work with no dedicated entry point, or that a second /scope follows their completion. Expected: a short reader-facing overview in the README of the idea lifecycle across sessions, linking to the scope skill for the procedure.

## Notes

- 2026-10-25T18:15:24Z (feedback): created
  provenance: {"harness_session":"claude-code:96139a24-901e-4b9f-9dd9-fc2e1d277366","harness_session_source":"CLAUDE_CODE_SESSION_ID","origin":"agent"}
- 2026-10-26T15:34:19Z (main): scope: scoped; 0845def README Statuses/Agent skills and current scope/tasks skills establish a bounded reader-facing overview, P2/s/low/direct in tasks-6ed79b; original report preserved; origin: filing matched, claude-code:96139a24-901e-4b9f-9dd9-fc2e1d277366 lines 804-805, unique feedback call and created tasks-ba8c21 result; human turns 724 and 773 ask how the process works and whether it is described, while the agent proposes the README addition at 811, so origin agent retained; no origin_session, older-request stage not applicable
  provenance: {"harness_session":"codex:01a178d6-0f5f-7d01-8ac1-a01cfe262c14","harness_session_source":"CODEX_THREAD_ID"}
