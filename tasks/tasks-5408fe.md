---
id: tasks-5408fe
title: Make curate and scope passes finish their own follow-through
status: todo
priority: 2
created: 2026-10-13T09:20:02Z
updated: 2026-10-13T09:20:02Z
depends: []
parent: tasks-46d207
tags: []
source: docs/notes/2026-10-13-review-pass-followthrough-brief.md
agent: claude-code
---

Why: curate and scope passes leave loose ends that a later turn or the human must find by hand: waiting ideas carry no dependency edge, edits made across projects have no commit rule, misfiled reports can only be proposed for drop, proposals pile up without a read, and sample keeps redrawing freshly briefed ideas.
Done: each member below is delivered, ruled on, or dropped; the brief's alternatives are settled.
Where to look: docs/notes/2026-10-13-review-pass-followthrough-brief.md; skills/scope/SKILL.md, skills/curate/SKILL.md, src/commands/sample.rs.
