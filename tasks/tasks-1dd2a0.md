---
id: tasks-1dd2a0
title: "tasks feedback refuses to file when the target project has no [feedback] section, with no way to file friction about a tool that project owns"
status: idea
priority: 2
created: 2026-10-01T23:14:04Z
updated: 2026-10-01T23:14:04Z
depends: []
tags: [feedback, friction, "from:material"]
agent: claude-code/claude-opus-5-5
---

Filing friction about a repo-local tool with `tasks feedback --project <that project>` failed: '"<project>" does not accept feedback (no [feedback] in its tasks/.config.toml)'. The error lists the projects that do accept feedback, but none of them own the tool, so the agent had to fall back to a plain `tasks add` in the owning project and file by hand here. Options: let feedback land in any registered project as an idea tagged feedback (the [feedback] scope only steering routing), or have the error name that fallback.
