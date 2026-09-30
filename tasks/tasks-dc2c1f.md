---
id: tasks-dc2c1f
title: Include required reason in halt takeover hints
status: done
priority: 2
size: xs
complexity: low
process: direct
owner: fix/halt-takeover-hint
created: 2026-09-30T16:22:28Z
updated: 2026-09-30T16:39:09Z
started: 2026-09-30T16:23:07Z
completed: 2026-09-30T16:39:09Z
depends: []
tags: []
source: tasks-c543ae
agent: codex
---

Under a registered halt, ready and prime still suggest tasks start --force <id> for a stale claim, but start requires --reason. Make both takeover hints show a valid command under a halt while preserving unhalted wording. Cover both views in one focused CLI test.

## Notes

- 2026-09-30T16:23:07Z (fix/halt-takeover-hint): started
  provenance: {"harness_session":"codex:01a0f1f2-8c7e-71c3-9d0e-3ec49cdcdfef","harness_session_source":"CODEX_SESSION_ID"}
- 2026-09-30T16:34:49Z (fix/halt-takeover-hint): review: impl round 1 — verdict: accept; findings: P3 1; reviewer: codex/gpt-6.1-sol
- 2026-09-30T16:39:09Z (fix/halt-takeover-hint): done
  provenance: {"harness_session":"codex:01a0f1f2-8c7e-71c3-9d0e-3ec49cdcdfef","harness_session_source":"CODEX_SESSION_ID"}
- 2026-09-30T16:39:09Z (fix/halt-takeover-hint): Ready, next, and prime takeover hints now include --reason when this project has an open halt; unhalted hints remain unchanged. Focused CLI test, just test-fast, and just check pass.
  provenance: {"harness_session":"codex:01a0f1f2-8c7e-71c3-9d0e-3ec49cdcdfef","harness_session_source":"CODEX_SESSION_ID"}
