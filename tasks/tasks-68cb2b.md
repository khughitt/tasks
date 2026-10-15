---
id: tasks-68cb2b
title: "Paint project prefixes outside tables: show's related and lane ids, groups, where"
status: todo
priority: 2
size: m
complexity: mid
process: direct
created: 2026-10-15T15:52:44Z
updated: 2026-10-15T16:24:44Z
depends: []
parent: tasks-1f05a7
tags: [pretty]
agent: claude-code/claude-opus-5-5
---

Why: ops-30b965 wants a project's colour wherever a tool names the project. Table views already paint ids and titles (comfy-tables spec §5), and projects paints its prefix column. Still uncoloured: show's in-lane and related ids (dim chrome today, and cross-project rows like ops-… are where colour would help most), groups' member prefixes, and where's project. claims stays uncoloured on purpose (it opens no checkout).

Settled by precedent (scope, 2026-10-15): the tables already answer the open questions. Where a project has a colour it replaces the chrome role on ids (present::row_id) and paints titles (present::title); a project without one keeps today's roles. Colours of other projects are read where the command already opens them, so no surface gains a new filesystem read beyond the project config it resolves anyway.

Done when, in pretty output with colour on:
- show (and next, which renders through it): the in-lane id, moved to/from ids, and every related row (parent, depends on, child) take their project's colour on id and title. Parent, children and lane are same-project (project.color, already on ShowFields); a dependency takes the colour of the project its resolution opened (src/commands/show.rs describe; resolver.resolve_task); an unresolved dependency keeps chrome.
- groups: each member prefix uses present::project(prefix, color), colour read from the member's config when reachable; an unreachable member keeps today's text.
- where: the prefix uses present::project with WhereProject.color, already loaded.
- JSON is unchanged: any new field is #[serde(skip)] (GroupMember has no color today).
- Tests in tests/cli.rs assert the 38;2 sequence for each surface and a colourless project's unchanged output, like the existing project-colour tests near tests/cli.rs:7821.
- Docs: show-layout spec §3.4 (docs/specs/2026-10-03-show-layout-design.md) and comfy-tables §5 name the new painting; README's colour section if it lists painted views; skills/tasks/SKILL.md's project-colour paragraph.

Where to look: src/show_text.rs (overview_rows, related), src/present.rs, src/output.rs (Where and Groups render arms near the Projects arm, which already paints), src/commands/group.rs list, src/commands/show.rs describe, src/locations.rs scan_checkouts for how other commands collect colours.
