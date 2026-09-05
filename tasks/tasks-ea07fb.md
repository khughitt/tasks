---
id: tasks-ea07fb
title: Add Bash/Zsh autocomplete support
status: done
priority: 2
owner: feat/completions
created: 2026-09-03T16:14:57Z
updated: 2026-09-05T23:09:52Z
depends: []
tags: []
spec: docs/specs/2026-09-05-shell-completions-design.md
plan: docs/plans/2026-09-05-shell-completions.md
---

Add support for Bash/Zsh tab completion for CLI commands/args and also task IDs, e.g. tasks show partial-id-TAB.

## Notes

- 2026-09-05T23:09:52Z (feat/completions): bash/zsh completion for subcommands, flags, value sets, project prefixes and task ids via clap_complete CompleteEnv
