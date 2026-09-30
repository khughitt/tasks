---
id: tasks-22f407
title: "Decisions queue: list tasks with an unanswered curate proposal"
status: shelved
priority: 2
created: 2026-09-09T11:36:50Z
updated: 2026-09-30T10:05:33Z
depends: []
tags: [curation, cli]
spec: docs/specs/2026-09-08-task-curation-design.md
---

A curate pass leaves proposals (drop, merge, priority, children, questions) as the task's latest note, and sample excludes such tasks until a later note answers them. The only way to see the queue today is the warnings of a zero-count sample across all projects with the age check off: tasks sample -n 0 --older-than 0 --all-projects. If passes accumulate proposals faster than they get answered, give the queue a direct read: a list filter (--pending) or a small command that prints id, project, and the proposal text, in the list row shape plus one field, which is a JSON contract change and needs this task first. Decide after a few passes whether the workaround is enough.

## Notes

- 2026-09-24T13:35:22Z (main): 2026-09-24: the workaround command in the body fails ('--older-than 0' needs a unit); 'tasks sample --limit 0 --older-than 0d --all-projects' works and showed 2 pending proposals after the third pass, so the queue does not yet need its own command.
- 2026-09-30T10:05:33Z (main): shelved: Revisit when repeated curation/scope passes show unanswered proposals being missed or the existing sample --limit 0 --older-than 0d --all-projects view no longer suffices.
- 2026-09-30T10:05:33Z (main): scope: shelved; the latest prior review found only two pending proposals and no need for a command; this checkout currently reports none via sample --limit 0 --older-than 0d; revisit on missed decisions or an inadequate existing view; brief: docs/notes/2026-09-30-work-selection-brief.md
