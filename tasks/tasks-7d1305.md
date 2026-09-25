---
id: tasks-7d1305
title: Pretty rows misalign when ids differ in length (list --all-projects)
status: doing
priority: 1
size: s
complexity: low
process: direct
owner: main
created: 2026-09-25T10:39:04Z
updated: 2026-09-25T10:39:08Z
started: 2026-09-25T10:39:08Z
depends: []
tags: [bug]
agent: claude-code/claude-opus-5-5
---

table() prints the id unpadded, so columns align only while every id has the same length. Across projects the prefixes differ (forge-, nrp-), and every later column shifts. Pad the id to one width decided per output, like the parallel and type columns.

## Notes

- 2026-09-25T10:39:08Z (main): started
  provenance: {"harness_session":"claude-code:ce1d647a-9a21-48b4-9a7c-e8f25a23b2f9","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
