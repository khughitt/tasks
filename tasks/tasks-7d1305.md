---
id: tasks-7d1305
title: Pretty rows misalign when ids differ in length (list --all-projects)
status: done
priority: 1
size: s
complexity: low
process: direct
owner: main
created: 2026-09-25T10:39:04Z
updated: 2026-09-25T10:41:14Z
started: 2026-09-25T10:39:08Z
completed: 2026-09-25T10:41:14Z
depends: []
tags: [bug]
model: claude-opus-5-5
agent: claude-code/claude-opus-5-5
---

table() prints the id unpadded, so columns align only while every id has the same length. Across projects the prefixes differ (forge-, nrp-), and every later column shifts. Pad the id to one width decided per output, like the parallel and type columns.

## Notes

- 2026-09-25T10:39:08Z (main): started
  provenance: {"harness_session":"claude-code:ce1d647a-9a21-48b4-9a7c-e8f25a23b2f9","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-25T10:41:14Z (fix/id-width): done
  provenance: {"harness_session":"claude-code:ce1d647a-9a21-48b4-9a7c-e8f25a23b2f9","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-25T10:41:14Z (fix/id-width): Pretty rows pad the id to the longest id in the output (list, parked, quiet, tree, prime, claims), decided once per output like the parallel and type columns
  provenance: {"harness_session":"claude-code:ce1d647a-9a21-48b4-9a7c-e8f25a23b2f9","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
