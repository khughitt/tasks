---
id: tasks-68cb2b
title: "Paint project prefixes outside tables: show's related and lane ids, groups, where"
status: idea
priority: 2
created: 2026-10-15T15:52:44Z
updated: 2026-10-15T15:52:44Z
depends: []
parent: tasks-1f05a7
tags: [pretty]
agent: claude-code/claude-opus-5-5
---

Why: ops-30b965 wants a project's colour wherever a tool names the project. Table views already paint ids and titles (comfy-tables spec §5), and projects paints its prefix column. Still uncoloured: show's in-lane and related ids (dim chrome today, and cross-project rows like ops-… are where colour would help most), groups' member prefixes, and where's project. claims stays uncoloured on purpose (it opens no checkout). Decide per surface whether project colour replaces the chrome role, and whether reading other projects' configs for show's related rows is acceptable.
