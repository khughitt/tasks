---
id: tasks-059b2f
title: Add a tasks feedback command for structured upstream feedback
status: todo
priority: 1
size: m
created: 2026-09-03T02:23:22Z
updated: 2026-09-03T13:57:23Z
depends: [tasks-d7ba4e, tasks-1e0bbe, tasks-010f75, tasks-ddd9ed, tasks-80fec3]
tags: [feedback, cli]
spec: docs/specs/2026-09-03-feedback-design.md
---

Goal: let any project that uses tasks file structured feedback about the tool itself (friction, gaps, suggestions, positives) from inside a session, so lessons reach this repo instead of dying in chat. Spirit, not API, comes from science's feedback module (proto-science/science/src/science_tool/feedback.py + feedback_cli.py): one file per entry with id/date/project/target/category/status/summary/detail, recurrence as a list of occurrences rather than a counter, dedupe of similar open entries, and add/list/show/update/triage subcommands. Design our own: decide where entries live (a per-machine store like ~/.config/tasks/feedback/ vs. the upstream repo's tasks/ dir), how the reporting project is detected (registry prefix), and how an entry becomes a task here (triage -> tasks add). Keep JSON-first output and the single-writer rule. Needs a short design under docs/specs/ before implementation.

## Notes

- 2026-09-03T13:45:11Z (open-items): design written: docs/specs/2026-09-03-feedback-design.md; split into tasks-d7ba4e tasks-1e0bbe tasks-010f75; stays open as the umbrella until they land
- 2026-09-03T13:57:23Z (open-items): review round 1: recurrence carries tags; only exact titles auto-recur, similar ones are ambiguous; guarded write; human commit is the disclosure gate; added tasks-ddd9ed (show foreign ids) and tasks-80fec3 (prime uncommitted warning)
