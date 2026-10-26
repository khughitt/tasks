---
id: tasks-583efd
title: Retained-item closeout needs batching of independent lifecycle notes
status: idea
priority: 2
created: 2026-10-26T18:23:07Z
updated: 2026-10-26T18:23:07Z
depends: []
tags: [feedback, gap, "from:ops"]
agent: codex
origin: agent
---

Closing retained evidence can require dozens of per-file release records. The note command accepts one note per invocation and commits each write, so a mechanically checked cleanup produces many separate writes and commits. A documented batch route should preserve each item identity and normal checks while permitting one checked commit. Current workaround is sequential note calls; no checks are disabled.

## Notes

- 2026-10-26T18:23:07Z (feedback): created
  provenance: {"harness_session":"codex:01a1784d-b1c1-76d3-97a6-8726368044c3","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
