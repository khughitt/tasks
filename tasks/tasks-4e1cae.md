---
id: tasks-4e1cae
title: Document where a new task field must be recorded
status: done
priority: 3
size: xs
complexity: low
process: direct
owner: field-checklist
created: 2026-09-07T08:29:07Z
updated: 2026-09-14T11:50:41Z
started: 2026-09-14T11:47:19Z
completed: 2026-09-14T11:50:41Z
depends: []
tags: [docs]
---

The final review of the source field found the JSON shapes block in docs/specs/2026-08-29-tasks-design.md had not been updated; the plan's file list missed it. Write the checklist once, in that spec's section 3 or AGENTS.md: section 3.1 field table, the JSON shapes block and its += addenda, the add/edit usage blocks in section 5, skills/tasks/SKILL.md (both the add recipe and the edit flag list), README examples, and the Task struct-literal sites in src. Future field plans copy it into their Files lists.

## Notes

- 2026-09-13T16:47:44Z (main): Complexity low: the task already enumerates the field table, JSON shapes, usage blocks, skill recipes, README examples, and Task literal sites. Writing and checking that single checklist needs no remaining design decision.
- 2026-09-14T11:46:16Z (main): Process direct: the body enumerates every place a field must be recorded; the work is writing that checklist into the design spec, and verification is each item naming a real location.
- 2026-09-14T11:50:41Z (field-checklist): Checklist for recording a new task field written as design spec 3.4, in the spec rather than AGENTS.md so plans copy from the doc that holds five of the six items; every anchor verified against the tree.
