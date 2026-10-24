---
id: tasks-c750e1
title: Large task-show note histories can overflow an agent read
status: idea
priority: 2
created: 2026-10-24T17:36:29Z
updated: 2026-10-24T17:36:29Z
depends: []
tags: [feedback, friction, "from:tasks"]
agent: codex
---

A read of an implementation goal with many retained-item notes, batched with related task queries, exceeded the agent tool output budget and was truncated. The caller had to repeat the reads with selected JSON fields and filtered notes. Consider a discoverable bounded task-detail read that preserves warnings and provides a full-data recovery route; this is separate from bounded prime.
