---
id: tasks-864b4d
title: Make attachment temp allocation tolerate PID reuse
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-10-09T11:17:27Z
updated: 2026-10-09T11:18:50Z
depends: []
parent: tasks-0e432f
tags: []
agent: codex
spec: docs/specs/2026-10-08-task-move-design.md
---

Deferred task-move review D8: src/attachments.rs::write_new creates .<name>.tmp-<pid> exclusively. An interrupted copy can leave that temp; a later process reusing the PID then fails despite a valid source and absent destination. Use safely allocated unique temps with bounded collision retry, preserving atomic final publication, storage validation and owned-temp cleanup. Do not delete unverified orphan data. Add a focused regression with a pre-existing temp for the current PID: publication succeeds with exact bytes, orphan remains untouched, and move recovery can finish. Audit attach and move callers; no new cleanup command or dependency.

## Notes

- 2026-10-09T11:18:50Z (main): Filed from deferred pre-close review D8; no loss was observed, but an orphan temp with a reused PID can block attachment publication and forward recovery. This is shared attachment-helper maintenance, not a new move mode.
