---
id: tasks-28ee3d
title: Scrub machine layout from tracked files and commit ops-check 5
status: done
priority: 2
size: xs
complexity: low
process: direct
owner: tasks-28ee3d
created: 2026-09-21T13:06:08Z
updated: 2026-09-21T13:51:57Z
started: 2026-09-21T13:51:42Z
completed: 2026-09-21T13:51:57Z
depends: []
tags: []
source: ops-0c42b9
agent: "claude-code/claude-opus-5[1m]"
---

ops-check 5 (ops ops-0c42b9) fails on this machine's layout in any tracked file: the home directory, the hostname, WORK_ROOT, a registered checkout or its parent, the banned tracker host. tools/ops-check is already updated in the working tree but uncommitted, because the pre-commit hook refuses every commit here until these findings are gone. Per file: rewrite the path or hostname neutrally; drop the file when it is captured scratch that does not belong in the repository; or, for evidence that must stay verbatim, list its path prefix under layout_allowed in a root .ops-check.toml. Commit tools/ops-check in the same change.

Findings (6 lines in 1 files):
- docs/plans/2026-09-17-lifecycle-provenance.md: the hostname

## Notes

- 2026-09-21T13:51:42Z (tasks-28ee3d): started
  provenance: {"harness_session":"claude-code:95c6e29f-0d94-4e60-9943-1aeedd49379f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-21T13:51:57Z (tasks-28ee3d): done
  provenance: {"harness_session":"claude-code:95c6e29f-0d94-4e60-9943-1aeedd49379f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-21T13:51:57Z (tasks-28ee3d): The lifecycle-provenance plan names the rollout host as 'the development host' (Europa, the other host, is not this machine's name); tools/ops-check 5 committed with it.
  provenance: {"harness_session":"claude-code:95c6e29f-0d94-4e60-9943-1aeedd49379f","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
