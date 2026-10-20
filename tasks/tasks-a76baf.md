---
id: tasks-a76baf
title: Report lane candidates when scope reuses existing goals
status: todo
priority: 2
size: xs
complexity: low
process: direct
created: 2026-10-18T10:27:11Z
updated: 2026-10-20T09:09:32Z
depends: []
parent: tasks-5408fe
tags: [feedback, idea, "from:forge"]
---

Why: Scope currently gives lane guidance only when creating a goal; a pass that reuses a top-level goal or re-scopes its children can miss a standalone effort that belongs in the lanes view.

Done: Update skills/scope/SKILL.md to assess the existing goals touched by a pass and report plausible lane candidates with a short reason in its final summary. Report candidates without setting --lane or changing priority; preserve the rule against nested lanes. No whole-backlog scan or CLI change. Verify the wording against three cases: a reused standalone top-level goal is reported, a goal already marked as a lane is not, and a sub-effort inside an existing lane is not proposed as another lane.

Where to look: skills/scope/SKILL.md sections 4 and 7; skills/tasks/SKILL.md lane guidance; docs/specs/2026-10-03-lanes-needs-groups-design.md. Code and guidance checked at 7a7c85c; scope's lane paragraph is limited to new-goal creation. This belongs under the existing review-pass follow-through goal tasks-5408fe, inside the work discovery lane.

Original report: scope skill: lane guidance only fires when a pass creates a new goal; a pass that reuses an existing top-level goal or re-scopes its children never assesses whether that goal should be a lane. Suggest: report lane candidates in the summary (flag, don't set), since lane choice is scheduling the user owns, like priority on a re-scope
