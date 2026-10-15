---
id: tasks-b124ab
title: "The project identity color set in tasks/.config.toml is not in any JSON output (projects, where, resolve), so another tool cannot paint a path in its project's color without parsing config files"
status: idea
priority: 2
created: 2026-10-15T11:52:10Z
updated: 2026-10-15T11:52:10Z
depends: []
tags: [feedback, gap, "from:hq"]
agent: claude-code/claude-opus-5-5
---

Wanted: a session lister painting each row's cwd with its project's color via 'tasks resolve <path>' or 'tasks projects'; neither JSON carries color, and only the pretty --color=always output uses it.
