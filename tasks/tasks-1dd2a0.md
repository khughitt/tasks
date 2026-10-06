---
id: tasks-1dd2a0
title: "tasks feedback refuses to file when the target project has no [feedback] section, with no way to file friction about a tool that project owns"
status: idea
priority: 2
created: 2026-10-01T23:14:04Z
updated: 2026-10-06T16:52:05Z
depends: []
parent: tasks-6ed79b
tags: [feedback, friction, "from:material"]
agent: claude-code/claude-opus-5-5
---

Filing friction about a repo-local tool with `tasks feedback --project <that project>` failed: '"<project>" does not accept feedback (no [feedback] in its tasks/.config.toml)'. The error lists the projects that do accept feedback, but none of them own the tool, so the agent had to fall back to a plain `tasks add` in the owning project and file by hand here. Options: let feedback land in any registered project as an idea tagged feedback (the [feedback] scope only steering routing), or have the error name that fallback.

Lane recommendation to evaluate: retain explicit feedback opt-in and make the refusal explain the supported tasks add fallback for a tool owned by a non-accepting project. Inspect the current diagnostic and feedback special-tag behavior before changing the contract; a guidance/diagnostic fix may cover the need without enabling unsolicited feedback everywhere. Keep this as an idea until that bounded choice is scoped.

## Notes

- 2026-10-06T16:52:04Z (main): organization: assigned to guidance/adoption; evaluate explaining the existing add fallback before broadening feedback routing and opt-in semantics.
