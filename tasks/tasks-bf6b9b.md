---
id: tasks-bf6b9b
title: Design individual task migration from the project-movement brief
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-08T10:42:11Z
updated: 2026-10-08T10:42:11Z
depends: []
parent: tasks-a7460c
tags: []
source: docs/notes/2026-10-08-project-discovery-movement-brief.md
agent: codex
---

Why: tasks-7ba557 needs moving individual tasks between registered projects without losing history or breaking references. Project-wide prefix aliases cannot represent that operation.

Where to start: docs/notes/2026-10-08-project-discovery-movement-brief.md; src/registry.rs::canonical_id, src/resolve.rs::Resolver::resolve_task, src/commands/add.rs, src/hierarchy.rs::validate_parent, src/attachments.rs, src/claims.rs and src/rename/; the implemented project-resolution, prefix-rename and attachment specs.

Bound: produce a design spec and then an implementation plan, each reviewed by the user. Compare the existing source-and-title add/drop workflow with explicit migration to a new destination id and with per-task forwarding. Decide whether transparent old-id resolution is required before adding an identity mechanism. Settle destination collisions, inbound dependencies (including unavailable projects), parent/child boundaries, notes and provenance, attachment ledgers and bytes, spec/plan links, claim/park ownership, multi-host registry limits, partial failure and reruns. Reuse the shared registry/resolver boundaries; do not alias a still-live source prefix or silently route writes. No live migration or implementation in this task.

Done: reviewed design and plan state preserved versus intentionally changed identity/history, the smallest supported move, refusal/recovery behavior and acceptance cases. Include a single task with an inbound foreign dependency and attachment, a parented task, a live claim, a destination collision and interrupted retry. File implementation children only after plan review; retaining a documented add/drop workflow is a valid explicit disposition if it meets the agreed need.

Ideas it wakes: on completion, add the approved finding and artifact paths to tasks-7ba557 with tasks note in the same commit and update the brief so scope can reconsider it.
