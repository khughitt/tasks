---
id: tasks-5caeae
title: "Periodic tasks: a task that reappears on a cadence"
status: idea
priority: 2
created: 2026-09-07T10:25:11Z
updated: 2026-09-09T11:05:43Z
depends: []
tags: [quick-add, cli]
source: "mindful:thought:ec1726a8595c47f78ddaad57f65eb89e"
---

Tasks that come back on a schedule instead of closing for good: doc and code curation sweeps, dependency review, stale-doc checks. The curate skill (docs/specs/2026-09-08-task-curation-design.md) runs by hand today; a periodic sweep is the obvious first consumer.

## Open questions

- Where the cadence lives: a field on the task, or a separate recurrence record.
- What `done` means for an occurrence and how the next one is minted. tasks-d40e8e (lifecycle hooks) is the alternative: on done, a hook mints the next occurrence and no field is added. Whichever is scoped first constrains the other.
- How `ready`, `next`, and `prime` present a recurrence that is not yet due.
- Whether history is one task or one task per occurrence.

## Notes

- 2026-09-07T10:39:15Z (main): Shares a design fork with tasks-d40e8e: recurrence could be the first consumer of lifecycle hooks (on done, mint the next occurrence) rather than a field on the model. Whichever is scoped first constrains the other.
- 2026-09-09T11:05:43Z (design/curation): curate: refined; open questions moved under their heading, the hooks alternative folded into the question it answers, curate skill named as the likely first consumer
