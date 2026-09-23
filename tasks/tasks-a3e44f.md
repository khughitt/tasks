---
id: tasks-a3e44f
title: Vendor relay's handles.json at a pinned revision and test the snapshot reader against it
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-09-23T20:55:42Z
updated: 2026-09-23T20:55:42Z
depends: []
tags: [cli]
agent: "claude-code/claude-opus-5-5[1m]"
---

The relay snapshot reader (src/relay/snapshot.rs) is tested against handle cases copied inline from relay-06b1da, not against relay's published test/fixtures/handles.json. ancestry.json is now vendored byte for byte at tests/fixtures/relay/ with a README naming its revision (tasks-2dd094); do the same for handles.json at the current relay revision, and run every case through snapshot::parse, so a relay-side change to the handle format shows up as a failing test after a refresh rather than as silent drift. While there, make the ancestry corpus test also fail when an entry in NULL_SESSION_EXPECTED names a chain no longer in the corpus (a deferred minor from the tasks-2dd094 review). Answers one of the options relay-4a5559 lists.
