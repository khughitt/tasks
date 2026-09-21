---
id: tasks-0f8043
title: "check reports step_missing on dropped tasks whose plan heading was removed; a dropped record should not fail the drift check, or edit needs --no-step/--no-plan"
status: idea
priority: 2
created: 2026-09-20T23:58:58Z
updated: 2026-09-20T23:58:58Z
depends: []
tags: [feedback, friction, "from:mind6"]
agent: claude-code/claude-fable-5-1
---

tasks check → error step_missing for three status: dropped records after a plan revision merged their headings. Expected dropped records to be exempt from plan-heading drift, or a way to clear step/plan on edit. Workaround: pointed the dropped records' --step at the heading they merged into.
