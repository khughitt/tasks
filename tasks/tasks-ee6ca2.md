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
updated: 2026-09-25T15:56:18Z
started: 2026-09-25T14:31:14Z
depends: []
tags: [cli]
source: tasks-92757e follow-up
agent: claude-code/claude-opus-5-5
spec: docs/specs/2026-09-25-color-source-design.md
plan: docs/plans/2026-09-25-color-source.md
---

Let users choose where pretty output's truecolor scales (date recency, priority) get their colors: the terminal theme's ANSI slots, read by the OSC query or TASKS_PALETTE (today's only source), or a built-in 'default' palette tasks ships. A generated theme can make slot 5 an olive nearly equal to slot 6 (docs/specs/2026-09-25-priority-color-design.md §2.3), so the columns share a hue; built-in colors would give a stable, distinct look and no query. Design questions: the setting's name and home (env var beside TASKS_COLOR/TASKS_PALETTE, or config.toml); whether 'default' still needs fg/bg from the terminal for the old end or ships its own; how it composes with TASKS_PALETTE; per-scale or global.

## Notes

- 2026-09-25T14:31:14Z (main): started
- 2026-09-25T14:35:06Z (color-source): parked (waiting on agent, review): Review the draft design spec docs/specs/2026-09-25-color-source-design.md on branch color-source (.worktrees/color-source); on approval, write the implementation plan.
- 2026-09-25T14:50:16Z (color-source): spec review: 3 substantive + 2 minor findings applied (precedence table, no partial overrides, dark-background framing with visual check, NO_COLOR order, test-helper env hygiene)
- 2026-09-25T14:52:18Z (color-source): review accepted in full; spec revised; implementation plan drafted with three step children
- 2026-09-25T14:52:19Z (color-source): parked (waiting on agent, review): Review the implementation plan .worktrees/color-source/docs/plans/2026-09-25-color-source.md (the revised spec is at docs/specs/2026-09-25-color-source-design.md); on approval, execute tasks-017e6d first.
- 2026-09-25T14:57:26Z (color-source): plan review round 2: tasks merged into one commit (dead code at the gate), PTY no-query test added with a control, precedence test reclassified as regression, final smoke fixed (pretty, no palette, binary by path), spec §2.1 contrast wording corrected
- 2026-09-25T14:57:27Z (color-source): parked (waiting on agent, review): Review the revised plan .worktrees/color-source/docs/plans/2026-09-25-color-source.md (spec §2.1 and §4 updated to match); on approval, execute tasks-017e6d.
- 2026-09-25T15:16:59Z (color-source): plan review round 3: PTY helper fixed to compile (unsafe blocks, setsid checked), terminate (drop command, EIO as EOF), isolate the control (raw + explicit TASKS_THEME removal), and clean up on deadline (owned master, kill+reap); compiled and run as a standalone probe
- 2026-09-25T15:17:00Z (color-source): parked (waiting on agent, review): Review the corrected PTY helper in .worktrees/color-source/docs/plans/2026-09-25-color-source.md (compiled and run standalone); on approval, execute tasks-017e6d.
- 2026-09-25T15:35:00Z (color-source): plan review round 4: helper deadline made global (checked every iteration; output-done and child-exit both required, try_wait under the deadline), OwnedFd on both descriptors with nonblocking set before spawn, every post-spawn failure kills and reaps before reporting, control cannot inherit TASKS_THEME; helper compiled and run verbatim, failure paths probe-tested
- 2026-09-25T15:35:01Z (color-source): parked (waiting on agent, review): Review the final PTY helper in .worktrees/color-source/docs/plans/2026-09-25-color-source.md (compiled and run verbatim, failure paths probe-tested); on approval, execute tasks-017e6d.
- 2026-09-25T15:56:17Z (color-source): plan review round 5: stdio duplication via Stdio::from(slave.try_clone()) — checked and owned immediately, no unchecked libc::dup; helper compiled and run verbatim
- 2026-09-25T15:56:18Z (color-source): parked (waiting on agent): Design review is complete per the reviewer; execute tasks-017e6d (implementation) in .worktrees/color-source, then tasks-2ea977 (final verification).
