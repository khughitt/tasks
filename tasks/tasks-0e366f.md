---
id: tasks-0e366f
title: Default column headers from a tasks config file
status: todo
priority: 3
size: s
complexity: mid
process: planned
created: 2026-10-06T13:01:34Z
updated: 2026-10-06T13:01:36Z
depends: [ops-9122f2, tasks-02b1ea]
tags: [pretty]
agent: claude-code/claude-opus-5-5
---

Why: the comfy-table migration (tasks-02b1ea) adds an option to show column headers; the user wants it settable as a default in a tasks config file. Which file, format and precedence (flag, env, config) waits on ops-9122f2, the cross-project config conventions, so tasks matches the other projects. Done: a documented config key that defaults headers on or off, overridden by the flag, following the ops convention.
