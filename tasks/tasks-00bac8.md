---
id: tasks-00bac8
title: tasks quiet lists unclaimed work that needs an exclusive host resource
status: idea
priority: 3
reviewed: true
reviewed_at: 2026-10-17T18:10:39Z
created: 2026-10-03T16:36:11Z
updated: 2026-10-17T18:10:39Z
depends: [tasks-b65dbc]
parent: tasks-46d207
tags: []
source: docs/specs/2026-10-03-lanes-needs-groups-design.md
agent: claude-code
---

Today `tasks quiet` lists only parks with reason quiet, so a todo step declared `needs: [quiet]` (exclusive) that nobody has prepared or parked is invisible to the 'what could run tonight' entry point. The lanes/needs design §11 left this, and merging the park's needs recipe (idle|headless) with task needs, out of scope.
Open questions: should quiet show such todo steps beside the parked recipes (they lack minutes and next step)? Should the park recipe's needs become a reference into the project vocabulary? Is 'exclusive' the right selector, or a vocabulary flag such as `quiet = true`?
Where to start: src/commands/quiet.rs, src/commands/parked.rs::rows_preferring, docs/specs/2026-09-13-quiet-queue-design.md §4-5.

## Notes

- 2026-10-08T10:53:55Z (main): scope: briefed; quiet currently enumerates prepared park recipes, while list/ready already filter named needs; scope a bounded comparison before widening quiet or equating every exclusive resource with an idle host; brief: docs/notes/2026-09-30-work-selection-brief.md
- 2026-10-17T18:10:39Z (main): scope: briefed; stranded only for a missing edge: research tasks-b65dbc (todo, P3/s/mid/direct) already names this idea in Ideas it wakes, so added dep --on tasks-b65dbc; premise unchanged at 8e1ff08 (quiet.rs lists only Reason::Quiet parks); new evidence for b65dbc: material now declares [needs.quiet] exclusive, so its fixture matrix has a real vocabulary to test; brief: docs/notes/2026-09-30-work-selection-brief.md
