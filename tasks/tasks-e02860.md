---
id: tasks-e02860
title: No first-class way to declare concurrent work lanes (goal groups that can run side by side) or the shared resources that serialise them; the organisation had to live in a notes doc that prime and ready never surface
status: idea
priority: 2
created: 2026-10-03T09:50:16Z
updated: 2026-10-03T09:53:20Z
depends: []
parent: tasks-46d207
tags: [feedback, gap, "from:material"]
agent: claude-code
---

Workaround: top-level lane goals tagged 'lane', ad hoc needs-quiet/needs-nested/needs-owner tags, and a notes brief holding the lane table, milestones and cross-lane constraints. Gaps: (1) prime/ready/next do not show lanes, their guidance or first milestone, so a session starting from the picker never sees the organisation; (2) parallel is per task, while the real constraint between groups is contention for a shared resource (an idle host, a person's review, files another branch is rewriting); pairwise lane-to-lane parallel marks would be n-squared and go stale, whereas per-step resource needs would let the picker derive which lanes can run now; (3) park already records needs (idle, headless) but only for parked work, not as a field on todo tasks. Relates to the focus-marker idea and the project-groups/goal-focus design: a focus SET of active lanes, rather than one focused goal, would fall back to the next lane when one is blocked.
