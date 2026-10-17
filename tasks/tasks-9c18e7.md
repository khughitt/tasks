---
id: tasks-9c18e7
title: "dep accepts an edge to the task's own ancestor, which can never close first"
status: idea
priority: 2
created: 2026-10-17T12:51:55Z
updated: 2026-10-17T12:51:55Z
depends: []
tags: []
source: tasks-aeeef1
agent: claude-code/claude-opus-5-5
---

tasks dep <child> --on <its parent or ancestor> succeeds, but done refuses on a goal while any descendant is open, so the edge can never be satisfied: the child waits forever and, under wait-aware review, a reviewed idea with such an edge never wakes. Found during the wait-backfill freeze (one auto entry would have written one; overridden to leave). Decide whether dep (and add --depends, the editor) refuse an ancestor edge, and whether check warns on existing ones.
