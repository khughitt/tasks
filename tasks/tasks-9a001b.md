---
id: tasks-9a001b
title: Color tasks show output when colors are enabled
status: done
priority: 2
owner: feat/show-color
created: 2026-09-04T11:24:43Z
updated: 2026-09-05T20:15:56Z
depends: []
tags: []
---

`tasks list` now supports color output when enabled, e.g. using `--color=always`. Let's extend this to `tasks show`, being sure to use the same colors used in `list`.

## Notes

- 2026-09-05T20:15:56Z (feat/show-color): show --pretty paints its frontmatter values with the list table's roles: id/parent/depends/owner/tags dim, status hued, P0-P1 bold; keys, body and notes stay plain
