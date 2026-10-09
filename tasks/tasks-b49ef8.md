---
id: tasks-b49ef8
title: "Color tags consistently, with styles drawn from the tag vocabulary"
status: idea
priority: 3
created: 2026-10-03T16:55:26Z
updated: 2026-10-08T10:17:41Z
depends: [tasks-606552, tasks-6ac7fd]
parent: tasks-cea445
tags: [cli]
source: "user:2026-10-03 session (show screenshot)"
agent: claude-code/claude-opus-5-5
---

Scoping finding (2026-10-08): tasks-606552 landed in 3b58cd7. src/present.rs::tags still joins every tag into one Chrome span; src/show_text.rs::overview_rows and src/output.rs::task_row use that helper. Output::Tags prints row.tag directly, so a change to the shared helper alone will not cover all requested views. src/repo.rs currently accepts tag-to-meaning strings, without a style field.

Handoff: reuse tasks-6ac7fd and docs/notes/2026-10-02-tag-vocabulary-brief.md for optional style ownership, precedence, cross-project conflicts and offline behavior before choosing palette or rendering changes. Keep this as an idea; the original color alternatives below remain proposals. The eventual acceptance checks must cover the same tag across list/show/prime/tags, tags without styles, malformed styles, plain output and JSON; the user judges contrast and distinguishability in a rendered sample. No new design task or independent style store.

Original request and alternatives (preserved):

Tags render as one dim Chrome span in every view, so a list of tags reads as noise. Want each tag to keep a recognizable color everywhere (list, show, prime, tags).

Options to weigh when scoped:
- a stable hash-to-palette fallback, so every tag has a color with no configuration;
- an optional style per entry in the tag dictionary ([tags] in tasks/.config.toml, and the shared layer tasks-6ac7fd is designing);
- let mindful own tag styles as part of the shared vocabulary (tasks-eb2b4e) and have tasks read them, keeping tasks standalone when mindful is absent.

Rendering goes through the field-role mapping from tasks-606552, so this depends on it, and the style source should fall out of tasks-6ac7fd's precedence and offline rules rather than invent its own.

## Notes

- 2026-10-06T16:50:06Z (main): dependency: use tasks-6ac7fd's reviewed vocabulary/style-source contract; retain the existing rendering dependency and scope the color idea after that decision.
- 2026-10-08T10:16:45Z (main): scope: briefed; rendering prerequisite tasks-606552 is done; recorded shared-span and separate tags-command paths; reuse tasks-6ac7fd for style-source decisions and a finding note that wakes this idea; original options, source, parent and dependencies retained; brief: docs/notes/2026-10-02-tag-vocabulary-brief.md
