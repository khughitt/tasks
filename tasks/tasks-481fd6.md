---
id: tasks-481fd6
title: "A repeatable live check for relay identity: outer and nested Claude Code sessions against a scratch registry"
status: shelved
priority: 3
created: 2026-09-23T20:55:42Z
updated: 2026-09-30T09:58:28Z
depends: []
tags: [cli]
agent: "claude-code/claude-opus-5-5[1m]"
---

tasks-2dd094's live check (plan docs/plans/2026-09-23-relay-session-process.md, Task 5) was run by hand: scratch XDG config/state and RELAY_STATE_DIR, relay hooks attached via claude --settings, an outer interactive session on a private tmux socket, a nested claude -p held open on a release file while the registry and claim store are captured. The next relay change to process selection will need it again. Worth scripting (with its own pid/tmux cleanup) if relay's rule changes again; not before.

## Notes

- 2026-09-30T09:58:28Z (main): shelved: A relay/tasks change alters session-process selection or registry identity matching and requires another outer/nested Claude live check; reuse the documented Task 5 recipe when it wakes.
- 2026-09-30T09:58:28Z (main): scope: shelved; the original outer/nested check is recorded as passed in tasks-fc61f4 and the reusable recipe remains in docs/plans/2026-09-23-relay-session-process.md; automate when the matching/process-selection contract changes; brief: docs/notes/2026-09-30-delegated-identity-brief.md
