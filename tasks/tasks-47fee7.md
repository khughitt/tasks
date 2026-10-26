---
id: tasks-47fee7
title: Correct the README relay recovery summary for PID namespaces
status: todo
priority: 3
size: xs
complexity: low
process: direct
created: 2026-10-26T21:22:02Z
updated: 2026-10-26T21:22:18Z
depends: []
tags: [docs]
source: tasks-d5ddb4
agent: codex
origin: agent
---

Why: curate-docs rubric v3 at c1bbcae found README.md Relay identity lines 684-689 still say every relay error recommends TASKS_SESSION plus TASKS_SESSION_PID and that this recovers all errors. The PID-namespace paragraph earlier in the same section, and src/relay/resolve.rs SET_SESSION_ONLY/no_match, explicitly require omitting TASKS_SESSION_PID when the harness appears as pid 1 with ppid 0. A namespace-local PID leaves the claim held after the session exits. tasks-19b244 added the correct special-case advice in f2ab2cb but left the universal recovery summary unchanged.

Done: shorten/correct the final Relay identity recovery paragraph to remove its universal two-variable recommendation and preserve the namespace-specific no-PID advice, explicit identity precedence and distinction from native note provenance. No code or matching/liveness changes.

Where to look: README.md Relay identity, src/relay/resolve.rs SET_EXPLICIT, SET_SESSION_ONLY and no_match, and tasks-19b244. Verify the resulting advice against these source paths and the earlier README paragraph; use the project's documentation gates when implementing.

## Notes

- 2026-10-26T21:22:02Z (add): created
  provenance: {"harness_session":"codex:01a17a10-ca68-7152-a6d9-52e06712d48c","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
- 2026-10-26T21:22:18Z (main): concerns: tasks-19b244 defect — f2ab2cb corrected PID-namespace recovery advice but left the README summary claiming every relay error recommends both identity variables; this correction removes that contradictory universal advice.
