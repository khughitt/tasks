---
id: tasks-c3c0d1
title: "Task curation: a curate skill and tasks sample"
status: done
priority: 2
size: m
owner: design/curation
created: 2026-09-09T03:01:18Z
updated: 2026-09-09T10:12:06Z
depends: []
tags: [curation, cli, skill]
spec: docs/specs/2026-09-08-task-curation-design.md
plan: docs/plans/2026-09-08-task-curation.md
---

A curate skill, skills/curate/SKILL.md, that samples open tasks and refines each in a bounded pass: clearer prose, implicit assumptions made explicit, open questions collected, links fixed, stale and duplicate tasks proposed for drop. Backed by a tasks sample command that draws uniformly from open, unclaimed, not recently updated tasks. Zero new tasks per pass; drops, priority changes, and new tasks are proposals. Task kinds (templates) are a follow-up derived from passes.

## Notes

- 2026-09-09T08:42:19Z (design/curation): spec review: pass runs every command as tasks -C <root>; only pending proposals persist a skip, the age window governs repeat reviews; revalidate status, claim, and updated before the first write (check, not lock); summary gains a questions group
- 2026-09-09T09:23:28Z (design/curation): plan review: --older-than bounded 0..=36500 at the CLI and 0 skips the age check; draw uses Rng::u64 with a known-answer test; root resolved via tasks --pretty root
- 2026-09-09T10:11:56Z (design/curation): first pass: 0 tasks, verdicts none, 0 proposals (pool empty: every open record updated within the 7-day window); no skill defects exposed
- 2026-09-09T10:12:06Z (design/curation): tasks sample and the curate skill landed; kinds follow in tasks-5b73bf
