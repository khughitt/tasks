---
id: tasks-a8e28e
title: "tasks docs: list and search working documents under each project's records with their linking tasks"
status: done
priority: 2
size: m
complexity: mid
process: direct
owner: tasks-a8e28e
created: 2026-10-22T20:54:25Z
updated: 2026-10-25T12:33:29Z
started: 2026-10-25T12:03:08Z
completed: 2026-10-25T12:33:29Z
closed: 2026-10-25T12:33:29Z
depends: [tasks-211e28]
tags: []
source: tasks-dc16eb
agent: claude-code/claude-fable-5-1
spec: docs/specs/2026-10-22-records-home-design.md
---

tasks:docs/specs/2026-10-22-records-home-design.md §5.5 and §12 case 7: tasks docs [--project|--all-projects] [--kind spec|plan|brief|handoff|note] [--search <phrase>] lists documents under each project's records docs directories with the tasks that link them (spec, plan, --ref), kind from directory and filename, phrase matching as search does. CLI rows land on ops cli.toml first.

## Notes

- 2026-10-25T12:03:08Z (main): started
  provenance: {"harness_session":"claude-code:55c55d6b-a1e9-4f80-9ac8-9cd52d047e88","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-25T12:03:26Z (tasks-a8e28e): resumed
  provenance: {"harness_session":"claude-code:55c55d6b-a1e9-4f80-9ac8-9cd52d047e88","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-25T12:06:11Z (tasks-a8e28e): baseline: just test in .worktrees/tasks-a8e28e ran 1685 tests, 0 failed; no setup route declared (cargo builds into the worktree's own target)
- 2026-10-25T12:24:30Z (tasks-a8e28e): ops: docs row landed on ops main as 29d0305 (merge of e6a800c, feat(cli): declare tasks docs; ops just test baseline 1910 OK); adopted into tools/cli.toml; ops worktree removed (pycache only ignored files)
- 2026-10-25T12:30:32Z (tasks-a8e28e): review: impl round 1 — verdict: accept; findings: Minor 7; reviewer: claude-code/claude-opus-5-5
- 2026-10-25T12:33:17Z (tasks-a8e28e): review minors fixed in this branch: io errors name the path; non-UTF-8 warning correct under --search; dangling .md symlink warns; README scope lists name docs. minor (deferred): title_of ignores code fences and a leading BOM (same as resolve::heading_text); pretty tasks: line omits link status; no tests yet for a linked overlay, --group, overlapping spec/plan dirs, a --ref outside the docs dirs, or a moved tombstone's link
- 2026-10-25T12:33:29Z (tasks-a8e28e): done
  provenance: {"harness_session":"claude-code:55c55d6b-a1e9-4f80-9ac8-9cd52d047e88","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-25T12:33:29Z (tasks-a8e28e): tasks docs lists working documents (spec/plan dirs and docs/notes under records) with linking tasks; --kind, --search, scope flags; ops cli.toml row landed as ops 29d0305; acceptance case 7 tested on a detached layout
  provenance: {"harness_session":"claude-code:55c55d6b-a1e9-4f80-9ac8-9cd52d047e88","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
