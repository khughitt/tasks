---
id: tasks-a22dd9
title: "Built-in duplicate check on add --source: same source and title"
status: todo
priority: 2
size: s
created: 2026-09-07T08:29:07Z
updated: 2026-09-07T10:39:15Z
depends: []
tags: [cli, quick-add]
---

The quick-add skill (ops docs/specs/2026-09-06-quick-add-design.md section 4.2) has the agent scan list JSON in the target project for a task with the same source and title before every add. The tracker could do that itself: add --source refuses with a typed duplicate error (or warns and returns the existing id) when an open or closed task already carries the same source and title. Removes the most fragile step from the skill and makes reruns idempotent for every caller. Scope after the skill's first real runs show whether title equality is the right key.

## Notes

- 2026-09-07T10:39:15Z (main): No longer speculative: the quick-add skill hand-rolls exactly this, listing every status in the target project and comparing (source, title) in Python before each add.
