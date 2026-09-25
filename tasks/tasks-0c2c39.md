---
id: tasks-0c2c39
title: "Codex subagent claims fall back to sid:<pid> and are dead at once"
status: idea
priority: 2
created: 2026-09-25T02:09:23Z
updated: 2026-09-25T02:09:23Z
depends: []
tags: []
source: ai-2f1271
agent: claude-code/claude-opus-5-5
---

Seen 2026-09-24 (ai-2f1271 live Stop cases, codex-cli 0.156.1): a subagent spawned with spawn_agent ran tasks start; the claim recorded session sid:1112013 (the Unix-session fallback), not codex:<thread>, and read live:false immediately. The subagent's rollout carries its own thread id (session_meta.source.subagent.thread_spawn with parent_thread_id), but its shell evidently exports neither CODEX_SESSION_ID nor CODEX_THREAD_ID where tasks reads them. Effect: a Codex subagent's claim is invisible to ready/next and to ops hooks/claim-guard (the controller's Stop is not refused, unlike Claude Code where the subagent claims under the controller's session). Next: check which env the subagent shell gets; decide whether a subagent claim should key on its own thread or its parent's.
