---
id: tasks-084a9f
title: Respect Git discovery ceilings in failed linked discovery
status: doing
priority: 3
size: s
complexity: low
process: direct
owner: main
created: 2026-10-07T11:30:31Z
updated: 2026-10-09T00:50:11Z
started: 2026-10-09T00:50:11Z
depends: [tasks-9b0a2e]
parent: tasks-a7460c
tags: []
source: tasks-9b0a2e
agent: codex
---

Deferred Minor from final corrective re-review: GitTree::of failed-discovery ancestor .git marker scan ignores GIT_CEILING_DIRECTORIES. In repo/excluded/child with ceiling repo/excluded, Git intentionally reports no repository but tasks where returns io for healthy repo above ceiling. Respect discovery boundaries while preserving corrupt-HEAD error propagation and linked-init no-write protection; add RED→GREEN regression and use focused/fast/check front doors.

## Notes

- 2026-10-09T00:50:11Z (main): started
  provenance: {"harness_session":"claude-code:4b1d7eef-4783-40e0-b224-bba892d8a7e3","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
