---
id: tasks-4107c8
title: Byte-preserving frontmatter rewrite
status: done
priority: 2
size: s
owner: design/prefix-rename
created: 2026-09-08T20:53:27Z
updated: 2026-09-08T23:09:36Z
depends: [tasks-6a4742]
parent: tasks-8c9398
tags: [rename]
plan: docs/plans/2026-09-08-prefix-rename.md
step: "Task 8: Byte-preserving frontmatter rewrite"
---

## Notes

- 2026-09-08T23:09:36Z (design/prefix-rename): rewrite_prefix moves a task's id and its local depends/parent through the frontmatter only, preserving every byte after the closing delimiter. The created/updated pre-quote and Raw-emit pair is factored out of parse_task and serialize_task and shared.
