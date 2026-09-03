---
id: tasks-b48dfa
title: Make spec and plan roots configurable per project
status: done
priority: 1
size: s
owner: open-items
created: 2026-09-03T11:41:14Z
updated: 2026-09-03T11:49:59Z
depends: []
tags: [cli, config]
---

Replace the hard-coded SPEC_DIRS/PLAN_DIRS with optional spec_dirs/plan_dirs keys in tasks/.config.toml; absent keys keep today's defaults. The configured roots serve both short-name search and explicit-path validation. Project-level only: a per-machine setting would make tasks check pass on one collaborator's machine and fail on another. Update design §2/§7/§9, README adoption notes, and SKILL.md in the same change. Motivated by the 2026-09-02 extension of SPEC_DIRS for a downstream project.

## Notes

- 2026-09-03T11:49:59Z (open-items): spec_dirs/plan_dirs in tasks/.config.toml replace the hard-coded roots; defaults unchanged; project-level only; init creates the first root of each; design §2/§7/§9, README, and SKILL.md updated
