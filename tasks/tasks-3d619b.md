---
id: tasks-3d619b
title: "Paint task ids in a lighter shade of their project colour, show's overview id included"
status: doing
priority: 3
size: s
complexity: low
process: direct
owner: feat/id-shade
created: 2026-10-18T18:54:35Z
updated: 2026-10-18T19:00:59Z
started: 2026-10-18T19:00:26Z
depends: []
parent: tasks-1f05a7
tags: [pretty]
agent: claude-code/claude-opus-5-5
---

Why: show and next print the overview line as '<title> (<id>)' with the title bold in the project colour but the id in dim chrome (src/show_text.rs overview, present::id), so the task's own id is the one id show leaves uncoloured. Every table, lanes, tree and show's related/in-lane/moved rows already paint ids in the full project colour (present::row_id), where id and title sit side by side in the same colour. A lighter shade for ids colours the overview id and sets ids apart from titles everywhere.

Decided at scoping: ids get one look in every view (present.rs's rule: a field reads the same everywhere). Rejected: the lighter shade on show's overview id alone, which would make show's id differ from the same id in a table. The shade is the project colour mixed toward white in OKLab (Rgb::mix) by one named constant, decided once in style.rs as a role (for example Style::Id(Rgb)) and not computed at call sites. It does not depend on the palette, like the project colour itself, so it still paints with --color=always piped and with no TASKS_PALETTE.

Done when:
- show/next's overview '(<id>)' paints in the id shade of the task's project colour, dim when the project has none; the title keeps its bold base colour.
- Every id painted through present::row_id (list, search, ready, prime, tree, lanes, sample, quiet, list --parked, show's related, in-lane and moved rows) uses the same shade; titles keep the base colour; uncoloured projects keep chrome.
- JSON is unchanged and strip_ansi of coloured output equals plain pretty output.
- tests/cli.rs project_colors_* expectations move to the shade, with a case for the overview id; a unit test asserts the shade's OKLab lightness exceeds the base for the fixture colours.
- README colour section, skills/tasks/SKILL.md colour paragraph, and docs/specs/2026-10-06-comfy-tables-design.md ('Ids and titles take the project colour') describe the id shade.

Out of scope, judged: ids inside prose (notes, lane guides, next steps, warnings), source (opaque, never interpreted), claims (uncoloured by design), graph output, and light-background terminals, where mixing toward white lowers contrast; adapting the mix to the palette's foreground would be a separate task.

## Notes

- 2026-10-18T18:54:43Z (main): process: direct — scoping settled the id role, the shade method, the surfaces and the checks; the mix fraction is a bounded choice made against real project colours
- 2026-10-18T19:00:26Z (main): started
  provenance: {"harness_session":"claude-code:6c3bce1d-9258-49a0-98a5-0f7c415f7c43","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-18T19:00:59Z (feat/id-shade): resumed
  provenance: {"harness_session":"claude-code:6c3bce1d-9258-49a0-98a5-0f7c415f7c43","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
