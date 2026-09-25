---
id: tasks-ee6ca2
title: "Configurable color source: terminal theme or built-in defaults"
status: doing
priority: 3
size: m
complexity: mid
process: planned
owner: main
created: 2026-09-25T13:11:50Z
updated: 2026-09-25T14:50:16Z
started: 2026-09-25T14:31:14Z
depends: []
tags: [cli]
source: tasks-92757e follow-up
agent: claude-code/claude-opus-5-5
spec: docs/specs/2026-09-25-color-source-design.md
---

Let users choose where pretty output's truecolor scales (date recency, priority) get their colors: the terminal theme's ANSI slots, read by the OSC query or TASKS_PALETTE (today's only source), or a built-in 'default' palette tasks ships. A generated theme can make slot 5 an olive nearly equal to slot 6 (docs/specs/2026-09-25-priority-color-design.md §2.3), so the columns share a hue; built-in colors would give a stable, distinct look and no query. Design questions: the setting's name and home (env var beside TASKS_COLOR/TASKS_PALETTE, or config.toml); whether 'default' still needs fg/bg from the terminal for the old end or ships its own; how it composes with TASKS_PALETTE; per-scale or global.

## Notes

- 2026-09-25T14:31:14Z (main): started
- 2026-09-25T14:35:06Z (color-source): parked (waiting on agent, review): Review the draft design spec docs/specs/2026-09-25-color-source-design.md on branch color-source (.worktrees/color-source); on approval, write the implementation plan.
- 2026-09-25T14:50:16Z (color-source): spec review: 3 substantive + 2 minor findings applied (precedence table, no partial overrides, dark-background framing with visual check, NO_COLOR order, test-helper env hygiene)
