---
id: tasks-c96737
title: Roll out the provenance binary to every host
status: todo
priority: 2
complexity: low
process: direct
created: 2026-10-25T13:43:25Z
updated: 2026-10-25T13:43:25Z
depends: []
parent: tasks-053e89
tags: []
agent: claude-code
---

After the implementation branch merges: on this host, run cargo install --path . from main and verify end to end (one add and one feedback show a stamped created note). Every other host that reads synced task records installs a current binary before records carrying origin or the new note keys reach it; older binaries reject them (spec §4.9). Done when each such host is named in a note as updated.
