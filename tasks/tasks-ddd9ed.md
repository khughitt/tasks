---
id: tasks-ddd9ed
title: show resolves foreign ids through the registry
status: done
priority: 1
size: s
owner: feat/feedback
created: 2026-09-03T13:57:08Z
updated: 2026-09-03T23:38:45Z
depends: []
parent: tasks-059b2f
tags: [feedback, cli]
spec: docs/specs/2026-09-03-feedback-design.md
plan: docs/plans/2026-09-03-feedback.md
step: "Task 2: show resolves foreign ids through the registry"
---

Feedback design §6.1: tasks show <id> with another project's prefix reads that task through the resolver, read-only, with spec_path/plan_path absolute against that project's root; unreachable prefix fails with unresolvable_id. Original design §5 show entry updated. e2e: show a foreign id from a second registered project; unregistered prefix errors.

## Notes

- 2026-09-03T23:38:45Z (feat/feedback): show reads a foreign id through the registry; unregistered prefix is unresolvable_id
