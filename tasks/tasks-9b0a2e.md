---
id: tasks-9b0a2e
title: "Linked projects: records outside the code checkout ([checkout] config, locate on miss bounded at the git top, where, init --checkout)"
status: todo
priority: 2
size: l
complexity: high
process: planned
created: 2026-10-01T16:01:43Z
updated: 2026-10-01T16:23:47Z
depends: []
tags: [cross-project]
agent: claude-code/claude-opus-5-5
---

Piece 1 of ops docs/specs/2026-10-01-linked-checkouts-design.md (§4.1, §4.1a, §8). A project's tasks/.config.toml may declare [checkout] path; when the walk up (stopped at the git top level) finds no config, locate matches the git main checkout of cwd against registered projects' [checkout]. New: tasks init --checkout <path> (validates a main checkout outside every registered tree; docs/ created in the project root), tasks where [PATH] --json with three outcomes ({project:{prefix,root,checkout,linked}}, {project:null}, error envelope incl. checkout_conflict) plus holds_records (§8.1), and checkout on linked rows of tasks projects --json (§8.2). Claims and parks keep the code worktree as resume location; parked::open_recorded/scan_recorded and their readers (quiet, next, prime parked, show fallback) resolve the project from that path via locate. No command writes into a linked checkout. CLI surface lands in ops cli.toml first (ops plan Task 1); vendored adopt cli.toml in the same commit. Tests: spec §6 tasks (1), nested foreign checkout, resolver outcomes.

## Notes

- 2026-10-01T16:23:47Z (main): spec amended in ops plan review round 1: holds_records counts a top-level tasks/.config.toml only when tracked in HEAD (§8.1, decision 5); the CLI's error JSON goes to stderr as today and consumers read it there
