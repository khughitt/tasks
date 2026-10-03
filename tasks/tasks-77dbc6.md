---
id: tasks-77dbc6
title: "Project groups: a named set of projects between one project and --all-projects"
status: done
priority: 2
size: m
complexity: high
process: planned
created: 2026-09-07T21:48:27Z
updated: 2026-10-03T15:52:55Z
completed: 2026-10-03T15:52:54Z
depends: []
parent: tasks-46d207
tags: [quick-add]
source: "mindful:thought:5ccf9506e36842cf8e635e55248040a9"
---

Scope today is one project (--project <prefix>, -C <dir>) or the whole registry
(--all-projects). Add a middle tier: a named group of projects, so "what should
we work on next from the verifiably projects?" is answerable.

Sketch:
- Declare groups in the registry (tasks config projects.toml), e.g. a [groups]
  table mapping a name to a list of prefixes.
- Accept a group wherever --all-projects is accepted today: list, ready, next,
  prime, tree, tags.
- Open: what to call the concept (groups / families / sets / collections /
  meta-projects); whether a project may belong to more than one; whether the
  group is declared in the registry or derived from something like the git
  remote org.

Motivating example: verifiably = nodes, atoms, beliefs, ... (the projects under
that GitHub org).

Related: tasks-3029be (done) added the registry-wide views this would narrow.

## Notes

- 2026-09-30T10:04:57Z (main): scope: briefed; group membership, storage, and rename behavior need a design contract; captured source retained but mindful lookup is unavailable; waits on tasks-ece1e2; brief: docs/notes/2026-09-30-work-selection-brief.md
- 2026-10-03T15:25:46Z (ece1e2-work-selection): implemented by tasks-ece1e2 slice 4 (spec 2026-10-03 §6); close with the slice
- 2026-10-03T15:52:54Z (ece1e2-work-selection): implemented by tasks-ece1e2 (spec §6)
- 2026-10-03T15:52:54Z (ece1e2-work-selection): done
  provenance: {"harness_session":"claude-code:83185704-f448-42a6-ac20-8857595bdb59","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-03T15:52:54Z (ece1e2-work-selection): Project groups: registry [groups], tasks group set|rm, tasks groups, --group scope on the read views and quiet (spec §6)
  provenance: {"harness_session":"claude-code:83185704-f448-42a6-ac20-8857595bdb59","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
