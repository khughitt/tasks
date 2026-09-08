---
id: tasks-bddcab
title: The inventory and the snapshot observer
status: done
priority: 2
size: m
owner: design/prefix-rename
created: 2026-09-08T20:53:27Z
updated: 2026-09-08T23:38:56Z
depends: [tasks-4107c8]
parent: tasks-8c9398
tags: [rename]
plan: docs/plans/2026-09-08-prefix-rename.md
step: "Task 9: The inventory and the snapshot observer"
---

## Notes

- 2026-09-08T23:27:01Z (design/prefix-rename): The inventory records source, target, root, config digests and per-task from/to digests under the state directory, and observe reads the world against it -- or without it, from the config's parsed prefix and a filename scan. sha2 added for a digest stable across Rust releases.
- 2026-09-08T23:33:13Z (design/prefix-rename): Review fix: task discovery now detects .md before UTF-8 conversion, so an invalid UTF-8 task filename is a typed parse error rather than omitted from inventory and snapshot observation.
- 2026-09-08T23:38:56Z (design/prefix-rename): Re-review fix: hidden non-UTF-8 markdown names remain excluded as dotfiles by checking the raw leading byte before task filename UTF-8 validation.
