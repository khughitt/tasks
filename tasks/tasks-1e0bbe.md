---
id: tasks-1e0bbe
title: "Feedback recurrence: title matching, --recur, --new"
status: done
priority: 1
size: s
owner: feat/feedback
created: 2026-09-03T13:45:11Z
updated: 2026-09-03T23:44:22Z
depends: [tasks-d7ba4e]
parent: tasks-059b2f
tags: [feedback, cli]
spec: docs/specs/2026-09-03-feedback-design.md
plan: docs/plans/2026-09-03-feedback.md
step: "Task 4: Feedback recurrence: title matching, --recur, --new"
---

Feedback design §3 steps 3-4 (recur branch) and §4: candidates are open target tasks tagged feedback; lowercase alnum tokens >= 3 chars; an exact normalized token sequence recurs automatically; Jaccard >= 0.6 but inexact fails with ambiguous listing candidate ids and titles (descending similarity, ties to the older task); --recur ID validates open feedback; --new skips matching. Recurrence appends 'feedback from <prefix>: <summary>' plus optional single-line detail note and adds from:<prefix> and <category> tags if absent, through a guarded read-modify-write (hash on read, re-check before atomic replace, retry up to 8 times, then concurrent_modification). action: recurred. Unit tests for normalization, exact vs similar, and the guard; e2e per §8.

## Notes

- 2026-09-03T23:44:22Z (feat/feedback): exact-title recurrence with notes and tags via a hash-guarded write; similar titles are ambiguous; --recur and --new overrides
