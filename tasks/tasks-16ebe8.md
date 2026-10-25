---
id: tasks-16ebe8
title: Deadline cleanup tests can panic before a child publishes its pid under scheduler contention
status: idea
priority: 2
created: 2026-10-25T11:54:58Z
updated: 2026-10-25T11:54:58Z
depends: []
tags: [feedback, friction, "from:tasks"]
agent: codex
---

The full-suite baseline passed. A subsequent unchanged-code fast-suite run failed two document-provider cleanup checks when they read a pid file absent after a 150 ms deadline. Observed host load was 135 on 32 CPUs; the fast suite still ran all cases. The checks should establish child startup separately from the deadline behavior they verify, so contention does not become a missing-file panic. No deadline or implementation change is included in the documentation work that exposed this.
