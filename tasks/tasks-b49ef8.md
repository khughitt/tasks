---
id: tasks-b49ef8
title: "Color tags consistently, with styles drawn from the tag vocabulary"
status: idea
priority: 3
created: 2026-10-03T16:55:26Z
updated: 2026-10-03T16:55:30Z
depends: [tasks-606552]
parent: tasks-cea445
tags: [cli]
source: "user:2026-10-03 session (show screenshot)"
agent: claude-code/claude-opus-5-5
---

Tags render as one dim Chrome span in every view, so a list of tags reads as noise. Want each tag to keep a recognizable color everywhere (list, show, prime, tags).

Options to weigh when scoped:
- a stable hash-to-palette fallback, so every tag has a color with no configuration;
- an optional style per entry in the tag dictionary ([tags] in tasks/.config.toml, and the shared layer tasks-6ac7fd is designing);
- let mindful own tag styles as part of the shared vocabulary (tasks-eb2b4e) and have tasks read them, keeping tasks standalone when mindful is absent.

Rendering goes through the field-role mapping from tasks-606552, so this depends on it, and the style source should fall out of tasks-6ac7fd's precedence and offline rules rather than invent its own.
