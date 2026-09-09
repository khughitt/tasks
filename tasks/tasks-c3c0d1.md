---
id: tasks-c3c0d1
title: "Task curation: a curate skill and tasks sample"
status: doing
priority: 2
size: m
owner: design/curation
created: 2026-09-09T03:01:18Z
updated: 2026-09-09T08:42:19Z
depends: []
tags: [curation, cli, skill]
spec: docs/specs/2026-09-08-task-curation-design.md
---

A curate skill, skills/curate/SKILL.md, that samples open tasks and refines each in a bounded pass: clearer prose, implicit assumptions made explicit, open questions collected, links fixed, stale and duplicate tasks proposed for drop. Backed by a tasks sample command that draws uniformly from open, unclaimed, not recently updated tasks. Zero new tasks per pass; drops, priority changes, and new tasks are proposals. Task kinds (templates) are a follow-up derived from passes.

## Notes

- 2026-09-09T08:42:19Z (design/curation): spec review: pass runs every command as tasks -C <root>; only pending proposals persist a skip, the age window governs repeat reviews; revalidate status, claim, and updated before the first write (check, not lock); summary gains a questions group
