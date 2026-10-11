---
id: tasks-e34b99
title: "tasks list cannot show whether an idea has been through scope: the scope skill's verdict (briefed, question, pending drop proposal, readmitted) lives only in the latest note, so a briefed idea and a never-reviewed one look identical in list rows; wanted a derived scope-state column and a filter such as --unscoped that applies the skill's eligibility rule, and the waiting-on task id for briefed ideas"
status: idea
priority: 2
created: 2026-10-08T10:43:40Z
updated: 2026-10-11T15:08:58Z
depends: []
parent: tasks-46d207
tags: [feedback, gap, "from:hq"]
---

## Notes

- 2026-10-08T10:48:15Z (main): User 2026-10-08, design input: replace the note-parsing with a top-level boolean on the task, shown as a '*' in a left-most list column for ideas not yet reviewed, like an unread marker; history and detail stay in the scope: audit note. Agent view: a boolean suffices if it is cleared explicitly, the way mail is marked unread again: the research completion that wakes an idea (the scope skill's 'Ideas it wakes') clears it in the same command, instead of the current rule where any later note readmits; a second pass then sets it again and adds a second audit note, so multiple passes need no richer type. It is meaningful only on ideas (leaving idea implies it); new ideas start unset; a one-time backfill applies the current latest-note rule. A pending drop proposal is a different signal (waits on the user) and fits park --waiting-on user rather than this flag. Naming: 'scoped' collides with the skill's 'scoped' verdict (promoted to todo), since a briefed idea would be scoped=true; 'reviewed' avoids that.
- 2026-10-08T10:53:55Z (main): scope: briefed; joined work-discovery lane tasks-46d207; tasks-a9b8d3 will design the requested boolean and unread marker, separating explicit re-review from note history and plan approval; original report and user-input note retained; brief: docs/notes/2026-09-30-work-selection-brief.md
- 2026-10-11T15:08:58Z (design/tasks-a9b8d3-review-state): Approved review-state design: reviewed boolean on ideas, explicit atomic --no-reviewed wakes, needs_review derived for unassessed leaf todo/blocked rows, and left-most * mark. Spec docs/specs/2026-10-11-review-state-design.md and plan docs/plans/2026-10-11-review-state.md are reviewed; implementation follows separately, with all-host upgrades before live writes.
