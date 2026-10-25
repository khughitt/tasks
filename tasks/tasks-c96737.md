---
id: tasks-c96737
title: Roll out the provenance binary to every host
status: doing
priority: 2
complexity: low
process: direct
owner: main
created: 2026-10-25T13:43:25Z
updated: 2026-10-25T15:09:55Z
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
- 2026-10-25T14:00:29Z (main): parked (waiting on user, approval): user: approve installing the merged binary (cargo install --path . from main) on this host and name the other hosts that read synced task records; then the merging agent installs here, verifies one add and one feedback show a stamped created note, and notes each host as updated
  provenance: {"harness_session":"claude-code:0f11453f-b2c1-4b7e-949d-cb12a26ba1da","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-25T15:07:49Z (main): resumed
  provenance: {"harness_session":"claude-code:f5a5b2af-aae8-4252-99e0-46dedc11650b","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-25T15:09:51Z (main): host titan updated: cargo install --locked --path . from main eb24f8e (includes merge 7c3b9d2), sha256 3b3b5c57; scratch add and feedback each show a stamped created note (claude-code session via CLAUDE_CODE_SESSION_ID)
- 2026-10-25T15:09:53Z (main): host europa updated: titan-built binary copied (same arch, glibc 2.44; europa Dropbox was mid-sync, so no build from its tree), sha256 3b3b5c57 replaces 3a9e06f1 of 2026-10-23; scratch add and feedback with a placeholder CLAUDE_CODE_SESSION_ID each show a stamped created note
- 2026-10-25T15:09:55Z (main): hosts reading synced task records, as named by the user: titan, europa
