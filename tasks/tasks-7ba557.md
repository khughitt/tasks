---
id: tasks-7ba557
title: no way to move a task between registered projects
status: idea
priority: 2
created: 2026-09-12T09:35:46Z
updated: 2026-10-08T10:42:11Z
depends: []
parent: tasks-a7460c
tags: [feedback, gap, "from:dots"]
---

Moving five open tasks to a new project meant re-adding each with --source and dropping the original by hand; a move that keeps id history, notes, and cross-project deps would replace that.

Next action: scope task movement against tasks-7580d2's canonical-id/alias contract and existing source-based add/drop workflow. Establish what must retain identity, notes, attachments, dependencies and parent associations, including partial failure and rerun behavior; distinguish project moves from prefix renames. Reuse the shared registry/resolver and do not add a second identity mechanism. Keep the move feature as an idea until those requirements have evidence and a reviewed design.

Scoping evidence (2026-10-08): tasks-7580d2 and tasks-9b0a2e are delivered. Registry::canonical_id changes only a retired project prefix and preserves the suffix; it cannot redirect one task out of an otherwise live project. Resolver::resolve_task selects a project by that canonical prefix. Source-and-title reuse in add prevents duplicate filing but creates no redirect and carries no prior notes/attachments automatically. hierarchy::validate_parent rejects cross-project parents.

Handoff: docs/notes/2026-10-08-project-discovery-movement-brief.md. Compare keeping the source-and-title add/drop workflow, an explicit migration that creates a destination id and preserves a source record, and a reviewed per-task forwarding contract. Reuse current identity and resolver code; do not represent an individual move by aliasing the entire source project. The design must settle inbound dependencies, same-project parents/children, notes/provenance, attachments and document links, claims/parks, destination collisions, interrupted writes and reruns before implementation. No migration is authorized by this scope pass.

## Notes

- 2026-10-06T16:52:05Z (main): organization: assigned to project location/migration; reuse the canonical identity contract and scope the move's history/dependency semantics before proposing implementation.
- 2026-10-08T10:42:11Z (main): scope: briefed; canonical ids only rewrite whole project prefixes and cross-project parents are refused; added tasks-bf6b9b to compare add/drop, explicit migration and per-task forwarding with preservation/recovery checks; existing parent and original report retained; brief: docs/notes/2026-10-08-project-discovery-movement-brief.md
