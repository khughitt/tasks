---
id: tasks-084a9f
title: Respect Git discovery ceilings in failed linked discovery
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-10-07T11:30:31Z
updated: 2026-10-07T11:30:31Z
depends: [tasks-9b0a2e]
parent: tasks-a7460c
tags: []
source: tasks-9b0a2e
agent: codex
---

Deferred Minor from final corrective re-review: GitTree::of failed-discovery ancestor .git marker scan ignores GIT_CEILING_DIRECTORIES. In repo/excluded/child with ceiling repo/excluded, Git intentionally reports no repository but tasks where returns io for healthy repo above ceiling. Respect discovery boundaries while preserving corrupt-HEAD error propagation and linked-init no-write protection; add RED→GREEN regression and use focused/fast/check front doors.
