---
id: tasks-c96737
title: Roll out the provenance binary to every host
status: doing
priority: 2
complexity: low
process: direct
owner: main
created: 2026-10-25T13:43:25Z
updated: 2026-10-25T14:00:27Z
started: 2026-10-25T14:00:27Z
depends: []
parent: tasks-053e89
tags: []
agent: claude-code
---

After the implementation branch merges: on this host, run cargo install --path . from main and verify end to end (one add and one feedback show a stamped created note). Every other host that reads synced task records installs a current binary before records carrying origin or the new note keys reach it; older binaries reject them (spec §4.9). Done when each such host is named in a note as updated.

## Notes

- 2026-10-25T14:00:27Z (main): started
  provenance: {"harness_session":"claude-code:0f11453f-b2c1-4b7e-949d-cb12a26ba1da","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
