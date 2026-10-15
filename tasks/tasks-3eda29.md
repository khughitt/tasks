---
id: tasks-3eda29
title: "edit --spec rejects a spec that exists only in a linked worktree (doc_not_found, even with -C <worktree>), so planned work whose spec precedes merge cannot link it"
status: idea
priority: 2
reviewed: true
created: 2026-10-14T20:37:24Z
updated: 2026-10-15T17:32:23Z
depends: [tasks-6360ac]
parent: tasks-a73dce
tags: [feedback, gap, "from:nexc"]
---

## Notes

- 2026-10-15T16:55:47Z (main): scope: briefed; parented under tasks-a73dce, waits on research tasks-82bd08. Premise holds: nexc is a linked project, so -C <code worktree> opens the overlay and documents resolve under the overlay root (linked-checkouts §168); a spec in the code worktree is outside every root. Open: whether a linked project may link team documents in the code repository; brief: docs/notes/2026-10-15-branch-only-document-links-brief.md
- 2026-10-15T17:21:59Z (main): scope: drop; user decided 2026-10-15 that a linked project never links documents in the code repository: tasks stays silent toward the project, so the team carries no tasks config or links. The reported spec (nexcode .worktrees/nex-342/docs/superpowers/specs/2026-10-14-hap-union-render-design.md) is a team document, so doc_not_found is the designed outcome (linked-checkouts §168); removed the wait on tasks-82bd08; proposal: drop as working as designed
