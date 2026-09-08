---
id: tasks-6a4742
title: "Registry lock, `add`'s lock, and post-lock revalidation"
status: done
priority: 2
size: m
owner: design/prefix-rename
created: 2026-09-08T20:53:27Z
updated: 2026-09-08T23:03:40Z
depends: [tasks-6725e3]
parent: tasks-8c9398
tags: [rename]
plan: docs/plans/2026-09-08-prefix-rename.md
step: "Task 7: Registry lock, `add`'s lock, and post-lock revalidation"
---

## Notes

- 2026-09-08T23:03:40Z (design/prefix-rename): Controller scope includes feedback create/recurrence and editor reacquisition through shared revalidation; Ctx retains original routing so editor writes stay in the intended checkout.
- 2026-09-08T23:03:40Z (design/prefix-rename): Serialized init/unregister registry mutations; add, ID writers, feedback and editor reacquisition now revalidate under the shared project lock while preserving routing, source idempotency and edit conflicts.
