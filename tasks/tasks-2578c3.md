---
id: tasks-2578c3
title: Assign project colors and use them in pretty output
status: done
priority: 2
size: m
complexity: mid
process: direct
owner: feat/project-colors
created: 2026-10-01T15:44:32Z
updated: 2026-10-01T17:11:14Z
started: 2026-10-01T17:03:36Z
completed: 2026-10-01T17:11:13Z
depends: []
tags: [cli, output]
agent: codex
---

Why: allow users to assign a persistent color to each project so project identity is easy to recognize in pretty output.

Done:
- Provide and document assignment and changes through an optional top-level color = "#RRGGBB" in tasks/.config.toml. Removing the key restores existing styling; invalid values fail with a typed config error. Reuse palette::Rgb::parse_hex.
- TASKS_FORMAT=pretty tasks --color=always projects displays project names (the prefix column) in their assigned colors.
- TASKS_FORMAT=pretty tasks --color=always list --all-projects displays task names (the title text) in the color assigned to their project.
- Use the same configured RGB in both views. Preserve existing styling for unassigned or unreachable projects; status, priority, date, tags, and metadata retain their existing roles.
- Respect existing color controls, including --color=never, NO_COLOR precedence, and auto behavior. JSON output and stored task IDs remain unchanged; any rendering-only metadata is skipped by serialization.
- Preserve visible column alignment and title wrapping when ANSI colors are enabled, including resets before line breaks. Keep color configuration through init/reinitialization and prefix rename.
- Document the configuration and both example commands in README.md and keep skills/tasks/SKILL.md in step.

Where to look: src/repo.rs::Config and Project::open, src/palette.rs::Rgb::parse_hex, src/style.rs::Painter, src/commands/projects.rs, src/commands/list.rs, src/output.rs::grid_text/table/render_row, src/scope.rs, and tests/cli.rs. The RGB parser, truecolor painter, scoped project lookup, and pad-before-paint/wrapped-span machinery already exist. Carry project identity styling through these paths without adding a registry color map or a dependency.

Verification: focused integration coverage for two differently colored projects in both requested commands, missing and invalid colors, disabled color and identical JSON, wrapped titles and visible alignment, and config preservation on rename/reinitialization. Use just test-one for relevant cases and just test-fast before committing. Use deterministic palettes in tests so terminal-query behavior does not obscure project-color assertions.

Process: direct; scope settles storage, accepted values, painted text, and verification. Size m, complexity mid because configuration must reach both project and task rendering without changing JSON.

Original assessment request: choose storage location, accepted color values, and interaction with existing output styles. Recommendation: project-local optional hex color, reusing existing parsing and rendering; a separate host registry mapping adds synchronization and rename handling without a requested need.

## Notes

- 2026-10-01T16:31:18Z (main): scope: scoped; P2/m/mid/direct; optional project-config hex color, shared RGB parser and painter, project-prefix and task-title rendering, unchanged JSON and color controls; no registry color map needed
- 2026-10-01T17:03:36Z (main): started
  provenance: {"harness_session":"codex:01a0f86a-bd5d-7db0-8349-126bfe1752ab","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-01T17:03:46Z (feat/project-colors): resumed
  provenance: {"harness_session":"codex:01a0f86a-bd5d-7db0-8349-126bfe1752ab","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-01T17:07:41Z (feat/project-colors): Implementation uses project-local validated RGB plus serde-skipped output fields; the existing painter handles padding and wrapped title resets. Integration coverage checks both commands, color controls, JSON equality, invalid values, and rename/reinitialization.
- 2026-10-01T17:11:13Z (feat/project-colors): review: impl round 1 — verdict: accept; findings: none; reviewer: codex
- 2026-10-01T17:11:13Z (feat/project-colors): done
  provenance: {"harness_session":"codex:01a0f86a-bd5d-7db0-8349-126bfe1752ab","harness_session_source":"CODEX_SESSION_ID"}
- 2026-10-01T17:11:13Z (feat/project-colors): Added validated project-local hex colors for pretty project prefixes and list titles; preserved JSON, color controls, wrapping, and configuration through reinitialization/rename. README and task skill updated; focused integration tests, just test-fast, and just check pass; independent review accepted.
  provenance: {"harness_session":"codex:01a0f86a-bd5d-7db0-8349-126bfe1752ab","harness_session_source":"CODEX_SESSION_ID"}
