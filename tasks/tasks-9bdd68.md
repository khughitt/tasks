---
id: tasks-9bdd68
title: "Focus marker: prefer one goal's subtree in ready, next, and prime"
status: idea
priority: 2
size: m
complexity: high
process: planned
created: 2026-09-11T01:03:10Z
updated: 2026-10-03T15:52:54Z
depends: []
parent: tasks-46d207
tags: [picker, hierarchy]
---

The gap a "sprint" would fill is narrower than a sprint: a way to say "this goal is what we are doing now" so the picker prefers it. Priority approximates it but is per-task, not a scope; a goal already carries the bundle, the plan, and the closing semantics.

Proposed shape:

- `tasks focus <goal>` marks one goal per project as focused; `tasks focus --clear` unmarks; `tasks focus` prints it.
- Stored beside claims, outside git, so it is visible from every worktree and never a commit.
- `ready` and `next` sort the focused goal's open descendants first (within the usual priority/size order); `prime` shows the focused tree above the roadmap.
- A focused goal that closes clears the focus; `prime` warns when the focus points at a closed or missing task.
- Registry-wide: `--all-projects` views list each project's focus; composes with project groups (tasks-77dbc6) for a cross-project bundle.

Deliberately not: a second hierarchy, dates or time boxes, capacity or velocity. If a time box is ever wanted, an optional `until` on goals with a `prime` warning when it passes is the smallest honest version; wait for that need to be real.

Origin: a "dynamics sprint" filed as a goal in another project; the goal did everything a sprint would except tell the picker to prefer it.

## Notes

- 2026-09-30T10:05:31Z (main): scope: briefed; focus must settle ordering against parked-agent resumes and priority while preserving eligibility; waits on tasks-ece1e2; brief: docs/notes/2026-09-30-work-selection-brief.md
- 2026-10-03T15:52:54Z (ece1e2-work-selection): scope: drop; proposal: drop as superseded by lanes (spec docs/specs/2026-10-03-lanes-needs-groups-design.md §7): lane priority orders the lanes view, next --under selects within one effort, and the committed lane field replaces the host-local focus
