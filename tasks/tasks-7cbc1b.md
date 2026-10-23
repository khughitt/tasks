---
id: tasks-7cbc1b
title: "done could warn when the task's per-task scratch directory still exists, so artifacts left there are given a disposition before close"
status: idea
priority: 2
created: 2026-10-23T12:43:58Z
updated: 2026-10-23T12:43:58Z
depends: []
tags: [feedback, idea, "from:hq"]
agent: claude-code/claude-opus-5-5
---

Asked by the user: avoid losing track of scratch made during a task. The global instructions place per-task scratch at $WORK_ROOT/<project>/<task-id>/. A task was closed with done while that directory still held a reproduction script and verification output that no record named. A warning on done (when WORK_ROOT is set and that directory exists and is non-empty), naming the directory and the attach command, would catch this mechanically; it could share a mechanism with declared resume inputs for parked tasks, which address park rather than close.
