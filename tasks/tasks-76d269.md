---
id: tasks-76d269
title: "tasks list -n/--limit: at most N tasks after sorting"
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-10-02T19:19:44Z
updated: 2026-10-02T19:19:44Z
depends: []
tags: [cli]
agent: claude-code/claude-opus-5-5
---

Why: ready takes the shared -n/--limit, but list does not, so 'the first few by --sort updated' needs a pipe through head (which also breaks the single JSON value). The shared vocabulary already defines limit (--limit, -n, int, per-command default).

Done: list accepts -n/--limit <N> and truncates after filtering, sorting, and --reverse, in every list mode (default, --parked, --periodic, --deferred, --all-projects). No limit by default, so existing output is unchanged; the JSON shape is unchanged (the tasks array is just shorter). Add the shared limit row to the list command in ops cli.toml first, then vendored adopt it into tools/cli.toml in the same commit as the implementation. Update README and skills/tasks/SKILL.md where list's options are described.

Verification: tests/cli.rs covers -n with --sort updated, --reverse, and --parked; just test-fast; just check; reinstall.
