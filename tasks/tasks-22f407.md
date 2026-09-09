---
id: tasks-22f407
title: "Decisions queue: list tasks with an unanswered curate proposal"
status: idea
priority: 2
created: 2026-09-09T11:36:50Z
updated: 2026-09-09T11:36:50Z
depends: []
tags: [curation, cli]
spec: docs/specs/2026-09-08-task-curation-design.md
---

A curate pass leaves proposals (drop, merge, priority, children, questions) as the task's latest note, and sample excludes such tasks until a later note answers them. The only way to see the queue today is the warnings of a zero-count sample across all projects with the age check off: tasks sample -n 0 --older-than 0 --all-projects. If passes accumulate proposals faster than they get answered, give the queue a direct read: a list filter (--pending) or a small command that prints id, project, and the proposal text, in the list row shape plus one field, which is a JSON contract change and needs this task first. Decide after a few passes whether the workaround is enough.
