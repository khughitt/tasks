---
id: tasks-0c2c39
title: "Codex subagent claims fall back to sid:<pid> and are dead at once"
status: idea
priority: 2
created: 2026-09-25T02:09:23Z
updated: 2026-10-06T10:07:13Z
depends: []
parent: tasks-2fa8c6
tags: []
source: ai-2f1271
agent: claude-code/claude-opus-5-5
---

Seen 2026-09-24 (ai-2f1271 live Stop cases, codex-cli 0.156.1): a subagent spawned with spawn_agent ran tasks start; the claim recorded session sid:1112013 (the Unix-session fallback), not codex:<thread>, and read live:false immediately. The subagent's rollout carries its own thread id (session_meta.source.subagent.thread_spawn with parent_thread_id), but its shell evidently exports neither CODEX_SESSION_ID nor CODEX_THREAD_ID where tasks reads them. Effect: a Codex subagent's claim is invisible to ready/next and to ops hooks/claim-guard (the controller's Stop is not refused, unlike Claude Code where the subagent claims under the controller's session). Next: check which env the subagent shell gets; decide whether a subagent claim should key on its own thread or its parent's.

## Notes

- 2026-09-30T09:58:26Z (main): scope: briefed; missing native IDs remain an unverified live-harness report; preserve the source and establish child-versus-controller ownership; waits on tasks-1ece46; brief: docs/notes/2026-09-30-delegated-identity-brief.md
- 2026-10-06T10:07:12Z (investigate/tasks-1ece46): finding: tasks-1ece46 — Original source tack-2f1271 transcript confirms dead sid: claim, but its removed scratch never captured native-ID presence; current Codex 0.160.1 pair has inherited controller session plus child thread, not missing IDs. Evidence: attached capture and docs/notes/2026-09-30-delegated-identity-brief.md; follow-up tasks-5e2472.
- 2026-10-06T10:07:12Z (investigate/tasks-1ece46): scope: briefed; reconsidered using tasks-1ece46 capture; preserve historical source and separate identity, provenance, delegated close and controller Stop; wait on planned tasks-5e2472 and reuse tasks-56b450 for Stop policy; brief: docs/notes/2026-09-30-delegated-identity-brief.md
