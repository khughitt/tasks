---
id: tasks-00723a
title: "Agent-docs wording in the tasks, scope and curate skills: documents under the records root"
status: doing
priority: 2
size: s
complexity: low
process: direct
owner: main
created: 2026-10-10T23:02:26Z
updated: 2026-10-25T11:44:49Z
started: 2026-10-25T11:44:49Z
depends: []
tags: []
source: "hq:docs/specs/2026-10-10-agent-docs-collection-design.md"
agent: claude-code/claude-fable-5-1
---

Re-scoped 2026-10-22 by tasks-dc16eb (tasks:docs/specs/2026-10-22-records-home-design.md §7.1, §8). The skills keep --spec docs/specs/<name>.md and --plan as they are (they resolve under records); the scope skill's brief lands under <records>/<prefix>/docs/notes/; the curate skill's fixable link may point under records; the plan-snapshot and mindful-op wording is dropped. Lands with the tasks CLI piece.

## Notes

- 2026-10-22T20:56:05Z (records-home): Re-scoped 2026-10-22: mindful capture and plan-snapshot wording superseded by tasks-dc16eb's design.
- 2026-10-23T01:04:43Z (main): Scope note 2026-10-22 from tasks-211e28 (landed, main 85b90f3): the tasks skill should also name tasks init --prefix <p> --records <records-root>, the [records] registry table, the three locations and kind in projects and where (an unreachable projects row carries records but no checkout or kind), records_moved, records_unpaired and records_in_tree with their remedies, that rename moves the records directory and leaves it for the user to commit, and that rename --adopt pairs a directory another host renamed. README.md Linked and detached projects has the wording to draw from.
- 2026-10-25T11:44:49Z (main): started
  provenance: {"harness_session":"codex:01a172e0-035d-7420-9c13-60fe7e644221","harness_session_source":"CODEX_THREAD_ID"}
