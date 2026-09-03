---
id: tasks-1e0bbe
title: "Feedback recurrence: title matching, --recur, --new"
status: todo
priority: 1
size: s
created: 2026-09-03T13:45:11Z
updated: 2026-09-03T13:45:11Z
depends: [tasks-d7ba4e]
tags: [feedback, cli]
spec: docs/specs/2026-09-03-feedback-design.md
---

Feedback design §3 steps 3-4 (recur branch) and §4: candidates are open target tasks tagged feedback; lowercase alnum tokens >= 3 chars, Jaccard >= 0.6, best match, ties to the older task; append 'feedback from <prefix>: <summary>' and optional single-line detail note; --recur ID validates open feedback; --new skips matching; action: recurred. Unit tests for normalization and threshold; e2e per §8.
