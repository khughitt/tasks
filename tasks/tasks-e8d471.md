---
id: tasks-e8d471
title: show lists a task's own dependencies but not the tasks that depend on it
status: idea
priority: 2
created: 2026-10-09T14:58:40Z
updated: 2026-10-09T14:58:40Z
depends: []
tags: [quick-add]
source: "mindful:thought:f489a269e04a438ab5732282730531aa"
agent: claude-code/claude-opus-5-5
---

tasks show prints the edges a task declares (depends on, parent) but not the edges other tasks hold toward it: a task that others depend on shows no dependents (e.g. a cross-project piece does not list the hub goal that depends on it). Show incoming relations too, by default or behind a flag.
