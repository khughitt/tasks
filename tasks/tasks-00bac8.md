---
id: tasks-00bac8
title: tasks quiet lists unclaimed work that needs an exclusive host resource
status: idea
priority: 3
created: 2026-10-03T16:36:11Z
updated: 2026-10-03T16:36:11Z
depends: []
parent: tasks-46d207
tags: []
source: docs/specs/2026-10-03-lanes-needs-groups-design.md
agent: claude-code
---

Today `tasks quiet` lists only parks with reason quiet, so a todo step declared `needs: [quiet]` (exclusive) that nobody has prepared or parked is invisible to the 'what could run tonight' entry point. The lanes/needs design §11 left this, and merging the park's needs recipe (idle|headless) with task needs, out of scope.
Open questions: should quiet show such todo steps beside the parked recipes (they lack minutes and next step)? Should the park recipe's needs become a reference into the project vocabulary? Is 'exclusive' the right selector, or a vocabulary flag such as `quiet = true`?
Where to start: src/commands/quiet.rs, src/commands/parked.rs::rows_preferring, docs/specs/2026-09-13-quiet-queue-design.md §4-5.
