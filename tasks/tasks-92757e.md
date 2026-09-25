---
id: tasks-92757e
title: Pretty priority column carries a three-level magenta scale
status: doing
priority: 2
size: m
complexity: mid
process: planned
owner: feat/priority-color
created: 2026-09-25T12:27:49Z
updated: 2026-09-25T13:28:13Z
started: 2026-09-25T12:56:52Z
depends: []
tags: [cli]
source: tasks-142d2f follow-up
agent: claude-code/claude-opus-5-5
spec: docs/specs/2026-09-25-priority-color-design.md
plan: docs/plans/2026-09-25-priority-color.md
---

Paint P1-P3 in three steps of the terminal theme's magenta (palette slot 5), strongest at P1 and fading toward the dimmed foreground, reusing the date scale's Recency/OKLab machinery (docs/specs/2026-09-25-date-recency-color-design.md). Replaces today's bold on P0/P1. Design questions: what P0 and P4 get (P0 full magenta + bold? P4 plain?); the query gains OSC 4;5, and TASKS_PALETTE gains a magenta key, which must not turn existing three-key values into config errors; whether the ramp is three fixed mixes or the same continuous function sampled at three points.

## Notes

- 2026-09-25T12:56:52Z (main): started
  provenance: {"harness_session":"claude-code:b8004ec1-87da-42d9-a0c8-3e5a4376d3be","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-25T12:59:58Z (feat/priority-color): spec drafted: P0 magenta+bold, P1 magenta, P2 0.5, P3 0.8, P4 dimmed fg; magenta optional in TASKS_PALETTE with a warning when absent; no-scale look keeps P0/P1 bold
- 2026-09-25T13:00:16Z (feat/priority-color): parked (waiting on user, review): user reviews docs/specs/2026-09-25-priority-color-design.md (swatch: scratchpad priority-swatch.py); on approval, writing-plans in .worktrees/priority-color
  provenance: {"harness_session":"claude-code:b8004ec1-87da-42d9-a0c8-3e5a4376d3be","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-25T13:11:59Z (feat/priority-color): resumed
  provenance: {"harness_session":"claude-code:b8004ec1-87da-42d9-a0c8-3e5a4376d3be","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-25T13:11:59Z (feat/priority-color): spec approved with review fixes: magenta warning only on views showing priorities; lightness separation is theme-dependent. Color-source choice filed as tasks-ee6ca2
- 2026-09-25T13:16:07Z (feat/priority-color): parked (waiting on user, review): user reviews docs/plans/2026-09-25-priority-color.md and picks execution (recommended: native); then implement Task 1 (tasks-ab49dc) in .worktrees/priority-color
  provenance: {"harness_session":"claude-code:b8004ec1-87da-42d9-a0c8-3e5a4376d3be","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-25T13:28:13Z (feat/priority-color): plan review fixes: Tasks 2-3 merged (dead-code lint), lightness moved to Task 2, close Task 3 before parent, env -u TASKS_PALETTE in kitty check
