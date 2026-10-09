---
id: tasks-084a9f
title: Respect Git discovery ceilings in failed linked discovery
status: done
priority: 3
size: s
complexity: low
process: direct
owner: discovery-docs-ceilings
created: 2026-10-07T11:30:31Z
updated: 2026-10-09T01:00:38Z
started: 2026-10-09T00:50:11Z
completed: 2026-10-09T01:00:36Z
depends: [tasks-9b0a2e]
parent: tasks-a7460c
tags: []
source: tasks-9b0a2e
model: claude-opus-5-5
agent: codex
---

Deferred Minor from final corrective re-review: GitTree::of failed-discovery ancestor .git marker scan ignores GIT_CEILING_DIRECTORIES. In repo/excluded/child with ceiling repo/excluded, Git intentionally reports no repository but tasks where returns io for healthy repo above ceiling. Respect discovery boundaries while preserving corrupt-HEAD error propagation and linked-init no-write protection; add RED→GREEN regression and use focused/fast/check front doors.

## Notes

- 2026-10-09T00:50:11Z (main): started
  provenance: {"harness_session":"claude-code:4b1d7eef-4783-40e0-b224-bba892d8a7e3","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-09T00:50:13Z (discovery-docs-ceilings): resumed
  provenance: {"harness_session":"claude-code:4b1d7eef-4783-40e0-b224-bba892d8a7e3","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-09T00:54:10Z (discovery-docs-ceilings): Scope: the marker scan now stops where git's discovery stopped — GIT_CEILING_DIRECTORIES (git's setup.c rules) and the filesystem boundary git names in its 'up to mount point' message (same class of gap, observed on this host's TMPDIR). Consequence: under a ceiling that hides a declared code checkout, init no longer refuses, matching git's view; the ceiling is the user's explicit choice.
- 2026-10-09T00:57:18Z (discovery-docs-ceilings): review: impl round 1 — verdict: revise; findings: Important 2, Minor 1; reviewer: claude-code/claude-opus-5-5
- 2026-10-09T01:00:36Z (discovery-docs-ceilings): review: impl round 2 — verdict: accept; findings: Minor 1; reviewer: claude-code/claude-opus-5-5
- 2026-10-09T01:00:36Z (discovery-docs-ceilings): minor (deferred): GIT_CEILING_DIRECTORIES entry '//' after an empty entry is trimmed to '/' and treated as a root stop, while git ignores it and examines /.git; matters only with a broken /.git. Fix: treat as root only when written exactly '/'. Also untested: root entries (need a /.git marker).
- 2026-10-09T01:00:36Z (discovery-docs-ceilings): done
  provenance: {"harness_session":"claude-code:4b1d7eef-4783-40e0-b224-bba892d8a7e3","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-09T01:00:36Z (discovery-docs-ceilings): The failed-discovery marker scan now examines only what git's discovery examined: strictly below GIT_CEILING_DIRECTORIES stops (git's setup.c rules: absolute entries, realpath before an empty entry, byte-exact with one trailing / forgiven after it) and below the mount point git names; an explicit GIT_DIR rejection stays unbounded. Regression tests in tests/linked.rs and a mount-point parser unit test.
  provenance: {"harness_session":"claude-code:4b1d7eef-4783-40e0-b224-bba892d8a7e3","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
