---
id: tasks-3333bc
title: Scope's default selection reads the whole --needs-review queue
status: todo
priority: 2
size: xs
complexity: low
process: direct
created: 2026-10-14T12:53:25Z
updated: 2026-10-14T13:22:48Z
depends: []
tags: [feedback, friction, "from:ops"]
---

Why: the pretty-table '*' and --needs-review merge two states with different remedies: an unreviewed idea, which the scope skill's default selection picks up, and a leaf todo/blocked task with no process, which that selection never returns (it filters --status idea), so starred todos can only be cleared by naming their ids as re-scopes. (Filed from ops; original proposal: make the default selection read the whole --needs-review set, unreviewed ideas first, then process-less todos as re-scopes, so '*' means 'in the scope queue'; and label the two kinds apart in pretty output — the mark for ideas, the existing '-' process column for todos, or a reason field in JSON.)

Decided at scoping: take the selection half; no CLI or JSON change. Rows already tell the kinds apart (status idea vs todo/blocked; process '-' in pretty, absent in JSON; reviewed on ideas), so a reason field would duplicate derivable data and change a JSON shape for nothing.

Done: skills/scope/SKILL.md §1 discovers with `tasks list --needs-review [--tag] [--project]`; unreviewed ideas form the batch when any are eligible, else process-less todo/blocked rows do, each a re-scope (§"Re-scope a scoped task", whose `confirmed` verdict already sets a missing process). Drop "Default selection never returns a non-idea". skills/tasks/SKILL.md's `--needs-review` line says '*' is the scope queue. docs/specs/2026-10-11-review-state-design.md §5.2/§7.1 get a dated amendment.

Where: skills/scope/SKILL.md §1 and the re-scope section; skills/tasks/SKILL.md (~line 58); the review-state spec.

## Notes

- 2026-10-14T13:22:48Z (main): scope: scoped; todo P2 xs/low/direct; selection half taken (scope default selection reads all of --needs-review, ideas first, then process-less todo/blocked as re-scopes); pretty relabel and JSON reason field declined: status, process and reviewed already distinguish the kinds
