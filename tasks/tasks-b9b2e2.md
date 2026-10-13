---
id: tasks-b9b2e2
title: Attachment size cap requires a separate archive and reference when preserving a larger set of cleanup artifacts.
status: idea
priority: 2
reviewed: true
created: 2026-10-10T13:22:07Z
updated: 2026-10-13T10:49:07Z
depends: []
tags: [feedback, gap, "from:mind6"]
agent: codex
---

Context (from mind6 commit 5f736e8 and mind6-9acb7a's notes): before removing a worktree, an agent preserved its ignored files as a 4,643,655-byte tar.gz. That is over the default per-file attachment cap (2 MiB, `[attachments] max_bytes`, docs/specs/2026-09-28-task-attachments-design.md), so it stored the archive in the host's work storage beside `.worktrees/` (`worktree-archives/`) and attached a 1,194-byte manifest with its location and SHA-256 instead. The same task holds a 946,661-byte attachment that fit. mind6 sets no `[attachments]` table. The cap is per file and per project by design; no Git LFS, no recompression, and repository files are referenced from notes rather than attached.

## Open questions

- Should bulk cleanup archives (ignored worktree files, raw run logs) live in git at all? If yes, mind6 raises its own `max_bytes` (a config edit, no tasks change) and this idea drops. If no, the host-storage archive plus an attached manifest is the intended pattern: should tasks document it (skill text on preserving large artifacts) or give it first-class support (an attachment that records an out-of-repo path and checksum without copying the bytes)? Note the host-storage copy is host-local, so another host cannot restore it. Answerable by: the user.

Original report (preserved): Attachment size cap requires a separate archive and reference when preserving a larger set of cleanup artifacts.

## Notes

- 2026-10-13T10:49:07Z (main): scope: question; traced to mind6 5f736e8 (4.6 MB worktree-ignored archive over the 2 MiB per-file cap, kept in host storage with an attached manifest); open question whether bulk archives belong in git (raise mind6 max_bytes, drop) or need a documented/first-class out-of-repo reference
