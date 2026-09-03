---
id: tasks-80fec3
title: prime warns about uncommitted files under tasks/
status: done
priority: 2
size: xs
owner: feat/feedback
created: 2026-09-03T13:57:10Z
updated: 2026-09-03T23:40:10Z
depends: []
parent: tasks-059b2f
tags: [feedback, cli]
spec: docs/specs/2026-09-03-feedback-design.md
plan: docs/plans/2026-09-03-feedback.md
step: "Task 3: prime warns about uncommitted files under tasks/"
---

Feedback design §3 'Where the inbox is': prime runs git status --porcelain -- tasks/ in the project root and adds one warning listing uncommitted task files; when the root is not a git checkout or git is unavailable the warning is skipped (documented, not a silent fallback). Original design §5 prime entry updated. e2e: warning appears for an uncommitted task file in a git checkout; absent in a plain directory.

## Notes

- 2026-09-03T23:40:10Z (feat/feedback): prime warns with the list of uncommitted files under tasks/ in a git checkout; silent outside git
