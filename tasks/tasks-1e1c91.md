---
id: tasks-1e1c91
title: Paint the project prefix in prime's header and resolve's pretty rows
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-10-15T16:49:03Z
updated: 2026-10-15T16:49:03Z
depends: []
parent: tasks-1f05a7
tags: [pretty]
agent: claude-code/claude-opus-5-5
---

Why: ops-30b965 wants a project's colour wherever tasks names the project. After tasks-68cb2b two pretty surfaces still print a prefix plainly: prime's header (src/output.rs fn pretty, Output::Prime, 'project <prefix>', 'group …: projects …', 'projects …' near :1385-1399; PrimeOut has no colour) and resolve's pretty rows (src/output.rs:1176-1199, which ignore the ResolveRow.color already carried).

Done when both paint prefixes with present::project(prefix, color) (dim when uncoloured), JSON is unchanged (any new field serde-skipped), tests in tests/cli.rs assert the 38;2 sequence and strip_ansi equality like project_colors_paint_show_groups_and_where_without_moving_json, and README/SKILL colour paragraphs list them.

Out of scope, judged: graph (Mermaid/DOT source, colour would break it), warning text, quiet's checkout path (a path, not a name), and one-line acks of add/init/move/rename/retire/id.
