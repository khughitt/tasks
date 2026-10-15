---
id: tasks-b124ab
title: "The project identity color set in tasks/.config.toml is not in any JSON output (projects, where, resolve), so another tool cannot paint a path in its project's color without parsing config files"
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-15T11:52:10Z
updated: 2026-10-15T13:38:35Z
depends: []
tags: [feedback, gap, "from:hq"]
agent: claude-code/claude-opus-5-5
---

Why: a session lister (hq-760db0, under ops-30b965) wants to paint each row's project prefix in that project's color, read through the tasks CLI and never by parsing tasks/.config.toml or the registry. tasks already loads the optional top-level `color = "#RRGGBB"` into Project.color (src/repo.rs), but no JSON carries it: ProjectRow.color is `#[serde(skip)]` (src/output.rs) because tasks-2578c3 scoped "JSON output remains unchanged" for its pretty-only change. That was that task's constraint, not a standing contract; this task is the one that changes the shape.

Done:
- `tasks projects` rows carry `color: "#rrggbb"` (lowercase, normalized from the config's either-case value) for a reachable project that sets one; the key is omitted otherwise (unset, unreachable).
- `tasks where` carries the same `color` on its `project` object when the opened project sets one.
- `tasks resolve` rows with status `resolved` (id, prefix, path kinds) carry `color` when the resolved project's root is reachable and its config sets one. resolve reads only the registry and location index today, so this adds a config read: open each resolved prefix at most once per invocation (a --stdin batch must not reopen per row). An unreachable root omits the field (resolve's job is mapping, which must keep working for moved checkouts). A reachable but malformed config omits the field and adds one warning naming the project and the config error; it does not fail the row or the command.
- The value is the color tasks uses for the project in pretty output, so when tasks-8203e6 changes where that color comes from, the field follows without a shape change.
- Pretty output unchanged; stored records unchanged; existing keys unchanged (additive only).
- README JSON contract, the command reference in docs/specs/2026-08-29-tasks-design.md, and skills/tasks/SKILL.md (the project identity colors paragraph, which says JSON is unchanged) updated.

Where to look: src/output.rs ProjectRow/WhereProject/ResolveRow, src/commands/projects.rs, src/commands/where_.rs::shown, src/commands/resolver.rs (row_for/path_row), src/repo.rs Config/Project::open, src/palette.rs::Rgb, tests/cli.rs existing projects/where/resolve and project-color tests.

Verification: CLI tests for each of the three commands with a colored project (mixed-case config value comes out lowercase), an uncolored project (key absent), resolve against an unreachable root (key absent, row still resolved) and a malformed color (warning, row still resolved), and a resolve batch over two projects. just test-one for the focused cases, just test-fast before committing; cargo install --path . afterwards.

Original report: The project identity color set in tasks/.config.toml is not in any JSON output (projects, where, resolve), so another tool cannot paint a path in its project's color without parsing config files. Wanted: a session lister painting each row's cwd with its project's color via 'tasks resolve <path>' or 'tasks projects'; neither JSON carries color, and only the pretty --color=always output uses it.

## Notes

- 2026-10-15T13:38:35Z (main): scope: scoped; P2/s/mid/direct; additive color field on projects, where and resolve rows (hex, omitted when unset/unreachable; resolve warns on a malformed config); ProjectRow.color serde(skip) came from tasks-2578c3's pretty-only scope, not a standing contract; consumer hq-760db0
