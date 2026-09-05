---
id: tasks-012a61
title: Integrate work claims with main and resolve merge conflicts
status: done
priority: 1
size: s
owner: feat/doing-claims
created: 2026-09-05T14:49:05Z
updated: 2026-09-05T14:53:30Z
depends: []
tags: []
spec: docs/specs/2026-09-05-work-claims-design.md
---

Preserve date/sort list changes and claim behavior, resolve document and CLI test conflicts, verify combined gate, merge to main and clean the feature worktree.

## Notes

- 2026-09-05T14:52:22Z (feat/doing-claims): Kept all CLI tests from both branches and combined created/claim summary fields; merged code passes the full gate.
- 2026-09-05T14:53:30Z (feat/doing-claims): Resolved main integration conflicts while preserving date/sort and claims behavior; full gate passes 189 tests and integration review is clear.
