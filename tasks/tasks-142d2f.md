---
id: tasks-142d2f
title: Pretty date columns carry a continuous recency colorscale
status: doing
priority: 2
size: m
complexity: mid
process: planned
owner: main
created: 2026-09-25T10:47:15Z
updated: 2026-09-25T11:02:11Z
started: 2026-09-25T10:47:20Z
depends: []
tags: [cli]
agent: claude-code/claude-opus-5-5
spec: docs/specs/2026-09-25-date-recency-color-design.md
---

In --pretty, color each date column on a continuous scale by age: today is fully saturated (cyan), older dates fade toward the terminal foreground or a dim grey. Map age through a log scale and/or clip it so outliers do not squash the range: the past ~2-4 weeks should change color visibly, while anything 2+ years old looks about the same. Reverses the color-output spec's no-256-color/truecolor decision (docs/specs/2026-09-03-color-output-design.md §2, §6), so it needs a design spec.

## Notes

- 2026-09-25T10:47:20Z (main): started
  provenance: {"harness_session":"claude-code:ce1d647a-9a21-48b4-9a7c-e8f25a23b2f9","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-25T10:47:20Z (main): Process planned: reverses the color-output spec's terminal-palette-only rule (no 256/truecolor), so the color depth and endpoint choice need a reviewed spec before code.
- 2026-09-25T11:01:44Z (feat/date-colors): Spec drafted: query via terminal-trx + xterm-color (xterm-query rejected: writes the query to stdout); TASKS_PALETTE override; plain dates + warning when the terminal does not answer.
- 2026-09-25T11:02:11Z (feat/date-colors): Replayed from the main checkout's uncommitted copy (10:58): Direction: query the terminal's own theme (OSC 10/11/4;6) rather than parse theme files; recent end = palette cyan, old end = fg blended toward bg, OKLab, log1p clip at 730d. Parked 10:58 on the user's decision; user confirmed and it resumed 11:00.
