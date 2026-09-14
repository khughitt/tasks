---
id: tasks-202e1f
title: "Quick launch: pick a next step from a short list and resume or start the agent session"
status: dropped
priority: 2
created: 2026-09-09T12:31:46Z
updated: 2026-09-14T12:49:20Z
depends: [tasks-08b9d5]
tags: [cli]
---

A command (fzf-style picker or small tui) over the open tasks and their left-off phases, one keystroke to launch or resume a coding-agent session on the chosen task. 'next' already aims at low-friction continuation but only prints the first ready task. This needs the lifecycle phases (to show where each task was left) and the recorded session information (to resume rather than start cold); scope it after both.

## Notes

- 2026-09-14T12:49:20Z (main): Subsumed by tasks-tui's launch picker (tui-d6e352, docs/specs/2026-09-13-tasks-tui-v1-design.md §7)
- 2026-09-14T12:49:20Z (main): Implemented as tasks-tui quick launch
