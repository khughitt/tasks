---
id: tasks-14ba63
title: Surface the top blockers in prime
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-26T15:00:47Z
created: 2026-10-26T01:55:31Z
updated: 2026-10-26T15:00:47Z
depends: [tasks-7903ff]
parent: tasks-46d207
tags: [cli]
source: tasks-e637a1
agent: claude-code/claude-opus-5-5
origin: agent
---

The original blocker capture (under tasks-e637a1) suggested a short prime section next to a dedicated view; the first delivery shipped only tasks blockers. Open questions: whether the bounded prime summary has budget for it, which scope it uses (local is often partial because of cross-project edges), and how it avoids reading as a picker when ready keeps its order.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-25-blocker-ranking-brief.md — shipped report boundaries and follow-on design questions

## Notes

- 2026-10-26T01:55:31Z (add): created
  provenance: {"harness_session":"claude-code:bafecb5c-78ad-4c0f-b8fc-6772b931692d","harness_session_source":"CLAUDE_CODE_SESSION_ID","origin":"agent"}
- 2026-10-26T15:00:47Z (main): scope: briefed; retained full body/source; reused work-discovery lane and existing brief; filed P2/s/high/planned prime-context design covering current 12288-byte summary, scope/coverage, omission recovery, scan reuse and unchanged startup-trial gates; wait recorded on tasks-7903ff; premises checked against 9cd4699 and shipped report merge 074672f; origin: stage 1 inferred; unique filing call toolu_01Ln8QArefcMFd4m21KrRVfn in claude-code:bafecb5c-78ad-4c0f-b8fc-6772b931692d at 2026-10-26T01:55:14Z, captured id-only jq result at 01:55:35Z with no reuse warning; user asked generically for follow-ups at 01:53:33Z, these specific proposals were agent additions, origin agent unchanged; stage 2 not applicable (no origin_session); brief: docs/notes/2026-10-25-blocker-ranking-brief.md
  provenance: {"harness_session":"codex:01a178b3-d016-7562-a8e1-983f62cc0118","harness_session_source":"CODEX_THREAD_ID"}
