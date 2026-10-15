---
id: tasks-3eda29
title: "edit --spec rejects a spec that exists only in a linked worktree (doc_not_found, even with -C <worktree>), so planned work whose spec precedes merge cannot link it"
status: idea
priority: 2
reviewed: true
created: 2026-10-14T20:37:24Z
updated: 2026-10-15T16:55:47Z
depends: [tasks-82bd08]
parent: tasks-a73dce
tags: [feedback, gap, "from:nexc"]
---

## Notes

- 2026-10-15T16:55:47Z (main): scope: briefed; parented under tasks-a73dce, waits on research tasks-82bd08. Premise holds: nexc is a linked project, so -C <code worktree> opens the overlay and documents resolve under the overlay root (linked-checkouts §168); a spec in the code worktree is outside every root. Open: whether a linked project may link team documents in the code repository; brief: docs/notes/2026-10-15-branch-only-document-links-brief.md
