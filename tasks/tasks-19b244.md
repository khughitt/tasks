---
id: tasks-19b244
title: Name the PID-namespace cause in relay identity refusals
status: todo
priority: 2
size: s
complexity: low
process: direct
created: 2026-10-14T19:05:34Z
updated: 2026-10-14T19:05:34Z
depends: []
parent: tasks-2fa8c6
tags: []
source: tasks-fe6c59
agent: claude-code/claude-opus-5-5
---

From tasks-fe6c59: process proof cannot cross a private-/proc PID namespace. The supported path there stays explicit identity, so the refusal must say so. Today a sandboxed Codex command gets "no relay agent matches the nearest codex ancestor, pid 1; set TASKS_SESSION and TASKS_SESSION_PID". Following that advice literally records a namespace-local pid, which the host misjudges (tasks-860db2).

Change `relay::resolve::no_match` (and the shared SET_EXPLICIT suffix where it applies): when the nearest harness boundary is pid 1 with ppid 0, which is only possible in a child PID namespace because the initial pid 1 is init, name the cause. The command runs in a PID namespace that hides the harness session process (a sandbox helper is pid 1), so relay cannot match it from here. Recommend `TASKS_SESSION` set to the exact session, without `TASKS_SESSION_PID`, or running with the host PID view. Keep it value-free like the other refusals. No change to matching, proof or liveness. Update the README paragraph on PID namespaces to match. Tests: a unit row in resolve tables with a pid-1/ppid-0 boundary, plus the existing no-match text for other cases.
