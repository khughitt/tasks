---
id: tasks-2518ad
title: Move integration fixtures inherit host Git hooks and fail during seed commits
status: idea
priority: 2
created: 2026-10-25T22:41:10Z
updated: 2026-10-25T22:41:10Z
depends: []
tags: [feedback, friction, "from:tasks"]
agent: codex
origin: agent
---

The git fixture helper sets author and committer identity but inherits HOME, XDG configuration and Git routing variables. Three move integration tests fail at their seed commit because the host identity hook resolves a temporary fixture against the live registry. The focused target reproduced 87 passes and 3 failures. Build subprocess environments explicitly, isolate global/system Git config and unset inherited Git routing before fixture writes; do not bypass the host hook as a remedy.

## Notes

- 2026-10-25T22:41:10Z (feedback): created
  provenance: {"harness_session":"codex:01a17532-7bd2-7062-bf78-835c57d1662f","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
