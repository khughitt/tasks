---
id: tasks-7a7386
title: "Wire the lock into Ctx, read commands unlocked"
status: done
priority: 2
size: s
owner: feat/doing-claims
created: 2026-09-05T10:21:31Z
updated: 2026-09-05T12:19:04Z
depends: [tasks-8dcf64]
parent: tasks-d184e3
tags: [claims]
plan: docs/plans/2026-09-05-work-claims.md
step: "Task 4: Wire the lock into Ctx without locking the read commands"
---

## Notes

- 2026-09-05T12:19:04Z (feat/doing-claims): Wired mutation locking into the eight existing write commands while keeping reads and create-only paths unlocked; interactive edit releases and reacquires the lock.
