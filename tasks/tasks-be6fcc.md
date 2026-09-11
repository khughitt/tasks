---
id: tasks-be6fcc
title: "One-shot defer date: hide an open task from ready until a date"
status: todo
priority: 2
size: m
created: 2026-09-11T01:15:26Z
updated: 2026-09-11T01:15:26Z
depends: []
tags: [quick-add, cli, periodic]
source: "mindful:thought:1e2513d2f5ea48609022559f3c687d01"
---

A one-shot date that hides an open task until it arrives: 'revisit this idea in two months' has no home today. The only date mechanism is --every, anchored on each completion (docs/specs/2026-09-09-periodic-design.md), so a fresh recurring task is ready at once and only its second occurrence is deferred.

Shape, following the periodic design's derived-not-scheduled principle: a `defer` (or `after`) field holding an RFC 3339 date on an open record; dueness is computed at read time. `ready` and `next` omit a task whose defer date is in the future and say so in warnings, the way live claims and parked work are reported; `prime` lists deferred work with its date, and lists it as due once the date passes; `list --deferred` (or extending --periodic) shows what is scheduled. `done` and `drop` clear the field. Grammar: an absolute date (`--defer 2026-11-10`) and a relative form measured from now (`--defer 60d`, `--defer 8w`), stored as the absolute date; `--no-defer` clears it. Ideas may carry a defer date too, since revisiting an idea is the motivating case; `sample` should leave deferred tasks alone until they are due.

Open: whether a deferred todo should also be excluded from dependency-readiness of its dependents (probably not: defer is about attention, not blocking), and whether a task can carry both `every` and `defer` (probably refuse; recurrence already defers).

Motivating records: prism-49a068 and prism-8a8eac, both carrying 'revisit 2026-11-10' in prose; move them onto the field when it lands. Feedback report: tasks-f5ab4a.
