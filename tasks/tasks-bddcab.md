---
id: tasks-bddcab
title: The inventory and the snapshot observer
status: done
priority: 2
size: m
owner: design/prefix-rename
created: 2026-09-08T20:53:27Z
updated: 2026-09-08T23:27:01Z
depends: [tasks-4107c8]
parent: tasks-8c9398
tags: [rename]
plan: docs/plans/2026-09-08-prefix-rename.md
step: "Task 9: The inventory and the snapshot observer"
---

## Notes

- 2026-09-08T23:27:01Z (design/prefix-rename): The inventory records source, target, root, config digests and per-task from/to digests under the state directory, and observe reads the world against it -- or without it, from the config's parsed prefix and a filename scan. sha2 added for a digest stable across Rust releases.
