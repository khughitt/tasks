---
id: tasks-e55788
title: Keep rename recovery roots consistent through symlinked registry paths
status: done
priority: 1
size: s
owner: design/prefix-rename
created: 2026-09-09T00:33:52Z
updated: 2026-09-09T00:57:11Z
depends: []
tags: [cli]
spec: docs/specs/2026-09-08-prefix-rename-design.md
plan: docs/plans/2026-09-08-prefix-rename.md
---

Final whole-branch review found raw registry roots differ from canonical Inventory roots, stranding interrupted renames. Normalize identity consistently across invocation, snapshot, replay and freeze checks; add focused interrupted recovery regressions.

## Notes

- 2026-09-09T00:38:29Z (design/prefix-rename): Normalize roots only at rename identity comparison boundaries; cover symlink/.. recovery before and after registry replacement plus older-alias replay/freeze.
- 2026-09-09T00:47:10Z (design/prefix-rename): Canonicalized rename root identity at routing, observation, replay, and freeze boundaries; added symlink, dot-dot, alias replay, and missing-root recovery regressions.
- 2026-09-09T00:57:11Z (design/prefix-rename): Removed two accidentally tracked temporary SDD reports during final cleanup; implementation rulings remain in the plan and final scoped review passed.
