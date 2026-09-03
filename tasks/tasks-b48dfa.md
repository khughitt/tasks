---
id: tasks-b48dfa
title: Make spec and plan roots configurable per project
status: todo
priority: 1
size: s
created: 2026-09-03T11:41:14Z
updated: 2026-09-03T11:41:14Z
depends: []
tags: [cli, config]
---

Replace the hard-coded SPEC_DIRS/PLAN_DIRS with optional spec_dirs/plan_dirs keys in tasks/.config.toml; absent keys keep today's defaults. The configured roots serve both short-name search and explicit-path validation. Project-level only: a per-machine setting would make tasks check pass on one collaborator's machine and fail on another. Update design §2/§7/§9, README adoption notes, and SKILL.md in the same change. Motivated by the 2026-09-02 extension of SPEC_DIRS for a downstream project.
