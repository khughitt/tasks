---
id: tasks-22f407
title: "Decisions queue: list tasks with an unanswered curate proposal"
status: todo
priority: 2
size: s
complexity: low
process: direct
created: 2026-09-09T11:36:50Z
updated: 2026-10-13T11:10:10Z
depends: []
tags: [curation, cli]
spec: docs/specs/2026-09-08-task-curation-design.md
---

Why: a curate or scope pass leaves its proposal (drop, merge, priority, children, questions) as the task's latest note, and `sample` keeps such tasks out of its pool until a later note answers them. The only read of that queue is the warnings of a zero-count sample: `tasks sample --limit 0 --older-than 0d --all-projects` (the `--older-than 0` form in the original idea fails: the age needs a unit). The queue held 2 proposals when this was shelved on 2026-09-30, 13 across 8 projects when it woke on 2026-10-13, and 12 at scoping the same day, so it now needs a direct read. The duplicate report tasks-84d2a9 was dropped into this task (brief: docs/notes/2026-10-13-review-pass-followthrough-brief.md).

Decided at scoping (2026-10-13): a `list` filter, not a new command, because `list` already has `--project`/`--group`/`--all-projects`, the status/tag filters, sort and the pretty table, and `sample` stays the curate draw. A separate `tasks proposals` command was rejected: it would duplicate list's scoping and row shape for one predicate.

Done:
- `tasks list --pending` keeps only tasks whose latest note is a pending proposal, by the rule `sample` uses today (`pending_proposal` in src/commands/sample.rs: a latest note starting `curate:` or `scope:` that contains a `proposal:` segment; any later note answers it). Move that helper to one shared place so `sample` and `list` cannot drift.
- With no `--status`, the filter applies over list's default pool (open minus shelved, ideas included); an explicit `--status` narrows it. It composes with the other filters and the scope flags. `--pending` with `--parked`, `--periodic` or `--deferred` is refused by clap (`conflicts_with`) unless supporting it is trivial in `list_parked`.
- JSON: `TaskSummary` gains `proposal: Option<String>` (the trimmed text after `proposal:`), serialized only when present (`skip_serializing_if`), and filled on every list row whose task is pending, with or without `--pending`. This is an additive JSON contract change, and this task is its authorization.
- Pretty: a pending row shows its proposal text, e.g. on an indented line under the row, without widening the table for other rows.
- `sample` behavior and its `<id> pending: <proposal>` warnings are unchanged.
- Docs: skills/curate/SKILL.md names `tasks list --pending --all-projects` as the way to see the decisions the human owes (replacing the zero-count sample workaround); skills/tasks/SKILL.md mentions the filter on its list-filter line.
- Tests in tests/cli.rs: a curate proposal and a scope proposal are listed with their text; a later plain note removes the row; a `curate:` note without `proposal:` is not pending; `--status` narrows; the JSON row of a non-pending task has no `proposal` key; across projects with `--all-projects`.

Where: src/commands/sample.rs (`pending_proposal`), src/commands/list.rs, src/cli.rs (`ListArgs` filters near `needs_review`), src/output.rs (`TaskSummary`, pretty list rows), docs/specs/2026-09-08-task-curation-design.md (proposal rule), docs/specs/2026-10-11-review-state-design.md §9 (left this view here).

## Notes

- 2026-09-24T13:35:22Z (main): 2026-09-24: the workaround command in the body fails ('--older-than 0' needs a unit); 'tasks sample --limit 0 --older-than 0d --all-projects' works and showed 2 pending proposals after the third pass, so the queue does not yet need its own command.
- 2026-09-30T10:05:33Z (main): shelved: Revisit when repeated curation/scope passes show unanswered proposals being missed or the existing sample --limit 0 --older-than 0d --all-projects view no longer suffices.
- 2026-09-30T10:05:33Z (main): scope: shelved; the latest prior review found only two pending proposals and no need for a command; this checkout currently reports none via sample --limit 0 --older-than 0d; revisit on missed decisions or an inadequate existing view; brief: docs/notes/2026-09-30-work-selection-brief.md
- 2026-10-13T09:58:28Z (main): unshelved
- 2026-10-13T09:58:29Z (main): Wake condition met (scope pass 2026-10-13, user-approved): sample --limit 0 --older-than 0d --all-projects reports 13 pending proposals across 8 projects, against 2 when shelved; duplicate report tasks-84d2a9 (from ops) dropped into this task. Brief: docs/notes/2026-10-13-review-pass-followthrough-brief.md
