---
id: tasks-be6fcc
title: "One-shot defer date: hide an open task from ready until a date"
status: done
priority: 2
size: m
complexity: mid
process: planned
owner: be6fcc-defer
created: 2026-09-11T01:15:26Z
updated: 2026-09-15T14:31:47Z
started: 2026-09-15T12:38:29Z
completed: 2026-09-15T14:31:47Z
depends: []
tags: [quick-add, cli, periodic]
source: "mindful:thought:1e2513d2f5ea48609022559f3c687d01"
spec: docs/specs/2026-09-15-defer-design.md
plan: docs/plans/2026-09-15-defer.md
---

A one-shot date that hides an open task until it arrives: 'revisit this idea in two months' has no home today. The only date mechanism is --every, anchored on each completion (docs/specs/2026-09-09-periodic-design.md), so a fresh recurring task is ready at once and only its second occurrence is deferred.

Shape, following the periodic design's derived-not-scheduled principle: a `defer` (or `after`) field holding an RFC 3339 date on an open record; dueness is computed at read time. `ready` and `next` omit a task whose defer date is in the future and say so in warnings, the way live claims and parked work are reported; `prime` lists deferred work with its date, and lists it as due once the date passes; `list --deferred` (or extending --periodic) shows what is scheduled. `done` and `drop` clear the field. Grammar: an absolute date (`--defer 2026-11-10`) and a relative form measured from now (`--defer 60d`, `--defer 8w`), stored as the absolute date; `--no-defer` clears it. Ideas may carry a defer date too, since revisiting an idea is the motivating case; `sample` should leave deferred tasks alone until they are due.

Open: whether a deferred todo should also be excluded from dependency-readiness of its dependents (probably not: defer is about attention, not blocking), and whether a task can carry both `every` and `defer` (probably refuse; recurrence already defers).

Motivating records: prism-49a068 and prism-8a8eac, both carrying 'revisit 2026-11-10' in prose; move them onto the field when it lands. Feedback report: tasks-f5ab4a.

## Notes

- 2026-09-13T16:47:42Z (main): Complexity mid: the outcome and read-time date approach are defined; bounded choices remain for date semantics, recurrence coexistence, dependency readiness, and consistent omission across ready, next, prime, and sample. Reassess after the design resolves those choices.
- 2026-09-15T12:38:29Z (main): Process planned: the body leaves design choices open (coexistence with --every, dependency readiness of deferred tasks, idea deferral, sample behaviour, grammar of relative dates) and the field touches every picker; a reviewed spec settles them before code.
- 2026-09-15T14:16:46Z (be6fcc-defer): took over a live claim held by session 935bf565-993e-4f8e-bc6c-637567fcd051 (owner main, host titan, pid 1974023, worktree /mnt/ssd/Dropbox/tasks, since 2026-09-15T12:38:29Z, age 5897s, live)
- 2026-09-15T14:16:46Z (be6fcc-defer): User requested implementation of the reviewed plan in the existing worktree; resumed the parent claim for implementation closeout. User chose to keep both motivating Prism records shelved and skip their date migration.
- 2026-09-15T14:19:46Z (be6fcc-defer): defer date: --defer/--no-defer, pickers skip it with one warning, list --deferred, prime line, check findings; Prism migration skipped by user decision
- 2026-09-15T14:28:40Z (be6fcc-defer): Final review found repeated block clears defer and two targeted coverage gaps; applying shared-transition fix and tests.
- 2026-09-15T14:31:47Z (be6fcc-defer): defer date feature finalized: repeated block preserves the date; due-only prime and malformed defer scan coverage added
