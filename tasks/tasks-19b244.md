---
id: tasks-19b244
title: Name the PID-namespace cause in relay identity refusals
status: done
priority: 2
size: s
complexity: low
process: direct
owner: fix/pidns-refusal
created: 2026-10-14T19:05:34Z
updated: 2026-10-15T08:19:33Z
started: 2026-10-15T08:15:48Z
completed: 2026-10-15T08:19:33Z
depends: []
parent: tasks-2fa8c6
tags: []
source: tasks-fe6c59
agent: claude-code/claude-opus-5-5
---

From tasks-fe6c59: process proof cannot cross a private-/proc PID namespace. The supported path there stays explicit identity, so the refusal must say so. Today a sandboxed Codex command gets "no relay agent matches the nearest codex ancestor, pid 1; set TASKS_SESSION and TASKS_SESSION_PID". Following that advice literally records a namespace-local pid, which the host misjudges (tasks-860db2).

Change `relay::resolve::no_match` (and the shared SET_EXPLICIT suffix where it applies): when the nearest harness boundary is pid 1 with ppid 0, which is only possible in a child PID namespace because the initial pid 1 is init, name the cause. The command runs in a PID namespace that hides the harness session process (a sandbox helper is pid 1), so relay cannot match it from here. Recommend `TASKS_SESSION` set to the exact session, without `TASKS_SESSION_PID`, or running with the host PID view. Keep it value-free like the other refusals. No change to matching, proof or liveness. Update the README paragraph on PID namespaces to match. Tests: a unit row in resolve tables with a pid-1/ppid-0 boundary, plus the existing no-match text for other cases.

## Notes

- 2026-10-14T19:35:17Z (main): scope reminder from review: recommending TASKS_SESSION without a namespace-local PID corrects the advice only; it does not stop unrelated host claims being pruned from a sandbox. tasks-860db2 owns that.
- 2026-10-15T08:05:30Z (main): tasks-860db2 landed (0.3.0): a namespace-local TASKS_SESSION_PID is no longer misjudged by the host; it records no view and stays held (Unverifiable) until reboot, release or --force. The advice to set TASKS_SESSION without TASKS_SESSION_PID still stands, now because such a claim never clears on its own. When this lands, also update docs/notes/2026-09-30-delegated-identity-brief.md lines ~153, ~351, ~372, which still describe tasks-860db2 as open
- 2026-10-15T08:15:48Z (main): started
  provenance: {"harness_session":"claude-code:00511062-6128-494b-aa3d-a144beb11015","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-15T08:15:54Z (fix/pidns-refusal): resumed
  provenance: {"harness_session":"claude-code:00511062-6128-494b-aa3d-a144beb11015","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-15T08:19:33Z (fix/pidns-refusal): done
  provenance: {"harness_session":"claude-code:00511062-6128-494b-aa3d-a144beb11015","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-15T08:19:33Z (fix/pidns-refusal): Relay no-match refusal names the PID namespace when the nearest harness is pid 1 with ppid 0, recommending TASKS_SESSION without TASKS_SESSION_PID or the host PID view; README and identity brief updated
  provenance: {"harness_session":"claude-code:00511062-6128-494b-aa3d-a144beb11015","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
