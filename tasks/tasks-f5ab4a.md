---
id: tasks-f5ab4a
title: "No one-shot due or defer date: a task cannot be hidden from ready until a future date"
status: dropped
priority: 2
created: 2026-09-11T01:13:18Z
updated: 2026-09-15T14:19:46Z
depends: []
tags: [feedback, gap, "from:prism"]
---

Wanted 'revisit this idea in two months'. The only date mechanism is --every <n>d|w, measured from each completion, so a fresh recurring task is ready immediately and only its second occurrence is deferred. Expected a one-shot field such as --due <date> or --defer <date> (or --after <n>d) that keeps the task out of ready and next until then, surfaces it in prime when due, and clears on done.

## Notes

- 2026-09-11T01:15:26Z (main): Scoped as tasks-be6fcc at the user's request; this report stays in the triage queue for the maintainer.
- 2026-09-15T14:19:46Z (be6fcc-defer): Landed as tasks-be6fcc: --defer <date|Nd|Nw> hides a task from the pickers until the date; cleared by any status change.
- 2026-09-15T14:19:46Z (be6fcc-defer): implemented by tasks-be6fcc
