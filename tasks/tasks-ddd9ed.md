---
id: tasks-ddd9ed
title: show resolves foreign ids through the registry
status: todo
priority: 1
size: s
created: 2026-09-03T13:57:08Z
updated: 2026-09-03T14:22:57Z
depends: []
tags: [feedback, cli]
spec: docs/specs/2026-09-03-feedback-design.md
plan: docs/plans/2026-09-03-feedback.md
step: "Task 2: show resolves foreign ids through the registry"
---

Feedback design §6.1: tasks show <id> with another project's prefix reads that task through the resolver, read-only, with spec_path/plan_path absolute against that project's root; unreachable prefix fails with unresolvable_id. Original design §5 show entry updated. e2e: show a foreign id from a second registered project; unregistered prefix errors.
