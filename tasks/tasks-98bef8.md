---
id: tasks-98bef8
title: Name tools/cli.toml and the surface test in AGENTS.md as part of any CLI change
status: doing
priority: 3
size: xs
complexity: low
process: direct
owner: crew
created: 2026-10-15T10:33:27Z
updated: 2026-10-16T19:56:28Z
started: 2026-10-16T19:27:37Z
depends: []
tags: [docs]
agent: claude-code/claude-opus-5-5
---

Why: a new subcommand or flag must also land as a row in ops cli.toml (vendored here as tools/cli.toml, which ops-check refuses to edit in place), and src/surface.rs's parser_surface_equals_table fails until the adopted table matches. The tasks-f9fc01 plan missed both: AGENTS.md's Layout lists tools/tt but not tools/cli.toml, and nothing says a CLI change starts with the ops row.
Done: AGENTS.md (Layout and Gates or Rules) says tools/cli.toml is the vendored CLI surface, that the surface test compares it with the parser, and that a CLI change lands its row on ops first and then runs `python3 "$(tasks --pretty root ops-88a0fa)/bin/vendored" adopt cli.toml` (ops README, Consumers). Revisit if ops-a4ab22 introduces a project-first row path.
Where: AGENTS.md; src/surface.rs; ops README Consumers section.

## Notes

- 2026-10-16T19:27:37Z (crew): started
- 2026-10-16T19:29:09Z (crew): crew: attempt 01M532Y91YNDRG7E6STJHEBTFV started — kind code-xs, worker openrouter/qwen/qwen3-coder-next, worktree .worktrees/crew-tjhebtfv
- 2026-10-16T19:34:55Z (crew): crew: attempt 01M532Y91YNDRG7E6STJHEBTFV — verdict passed (worker openrouter/qwen/qwen3-coder-next, reviewer openrouter/openai/gpt-6.1-sol); report /home/keith/.local/state/crew/attempts/01M532Y91YNDRG7E6STJHEBTFV/attempt-report.md
- 2026-10-16T19:34:56Z (crew): parked (waiting on user, review): review crew attempt 01M532Y91YNDRG7E6STJHEBTFV (verdict passed): branch crew/tasks-98bef8-tjhebtfv in .worktrees/crew-tjhebtfv; merge and close, or discard
- 2026-10-16T19:52:16Z (crew): resumed
- 2026-10-16T19:53:43Z (crew): crew: attempt 01M534BCHBHP5M0WT9WYMH32SA started — kind code-xs, worker openrouter/deepseek/deepseek-v4-flash-0731, worktree .worktrees/crew-wymh32sa
- 2026-10-16T19:56:28Z (crew): crew: attempt 01M534BCHBHP5M0WT9WYMH32SA — verdict passed (worker openrouter/deepseek/deepseek-v4-flash-0731, reviewer openrouter/openai/gpt-6.1-sol); report /home/keith/.local/state/crew/attempts/01M534BCHBHP5M0WT9WYMH32SA/attempt-report.md
