---
id: tasks-66120e
title: "Canonicalize input, resolution, and comparison"
status: done
priority: 2
size: m
owner: design/prefix-rename
created: 2026-09-08T20:53:27Z
updated: 2026-09-08T22:28:09Z
depends: [tasks-03a9c2]
parent: tasks-8c9398
tags: [rename]
plan: docs/plans/2026-09-08-prefix-rename.md
step: "Task 3: Canonicalize input, resolution, and comparison"
---

## Notes

- 2026-09-08T22:16:49Z (design/prefix-rename): Spec section 4 also requires canonical parent lookups and comparisons in hierarchy; preserve retired spellings already stored on disk.
- 2026-09-08T22:28:09Z (design/prefix-rename): canonical_id now applies at every user-input parse site, both resolution entry points, hierarchy parent lookups and comparisons, and dependency dedup, removal, self-check, and cycle edges. Stored dependency and parent spellings remain unchanged on unrelated saves.
