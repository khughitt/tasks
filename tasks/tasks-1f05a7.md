---
id: tasks-1f05a7
title: Shared vocabulary and presentation
status: todo
priority: 3
size: l
complexity: high
process: direct
lane: true
created: 2026-10-06T16:49:54Z
updated: 2026-10-06T16:50:00Z
depends: []
tags: []
source: "user:2026-10-06 tasks lane organization"
agent: codex
---

Give shared labels, colors, and display defaults explicit sources and precedence while preserving standalone use. First milestone: tasks-6ac7fd settles shared tag definitions; tag styles use that reviewed vocabulary contract instead of inventing a separate source.

Keep tasks-cea445 and its existing child tree intact. Add tasks-8203e6's project colors and tasks-0e366f's header defaults as separate milestones. Project colors retain the dependency on ops-b31634; header defaults retain the dependency on ops-9122f2. Neither external investigation is a prerequisite for tag vocabulary design.

Prefer existing project dictionaries or a local import over a runtime provider unless verified needs require one. Preserve tag filter semantics, explicit override/error rules, existing output contracts, and scheme/standalone behavior. Shared meaning, project color, and config defaults need coordinated interfaces rather than one combined schema or synchronization service. Done when retained work has reviewed decisions, agreed implementation and verification, or explicit dispositions.

## Notes

- 2026-10-06T16:50:00Z (main): organization: created from the user's approved 2026-10-06 lane proposal; coordination uses existing tasks and preserves nested goals and child review requirements.
