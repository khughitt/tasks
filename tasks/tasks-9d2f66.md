---
id: tasks-9d2f66
title: Distinguish closeout hubs from real blockage in dependency counts
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-26T15:00:40Z
created: 2026-10-26T01:55:26Z
updated: 2026-10-26T15:00:40Z
depends: [tasks-e176a0]
parent: tasks-46d207
tags: [cli]
source: tasks-e637a1
agent: claude-code/claude-opus-5-5
origin: agent
---

tasks blockers counts a childless cross-project closeout hub (a goal holding its pieces as depends) as ordinary work, because nothing in the schema says an edge is goal closeout rather than real blockage; the report states this inflation risk in every footer. Explicit goal or edge-purpose metadata would let it separate the two; it is a schema change needing its own reviewed design.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-25-blocker-ranking-brief.md — shipped report boundaries and follow-on design questions

## Notes

- 2026-10-26T01:55:26Z (add): created
  provenance: {"harness_session":"claude-code:bafecb5c-78ad-4c0f-b8fc-6772b931692d","harness_session_source":"CLAUDE_CODE_SESSION_ID","origin":"agent"}
- 2026-10-26T15:00:40Z (main): scope: briefed; retained full body/source; reused work-discovery lane and existing brief; filed P2/s/high/planned hub-identity design with mixed-purpose/legacy-record/counting fixtures and user design/plan gates; wait recorded on tasks-e176a0; premises checked against 9cd4699 and shipped report merge 074672f; origin: stage 1 inferred; unique filing call toolu_01Ln8QArefcMFd4m21KrRVfn in claude-code:bafecb5c-78ad-4c0f-b8fc-6772b931692d at 2026-10-26T01:55:14Z, captured id-only jq result at 01:55:35Z with no reuse warning; user asked generically for follow-ups at 01:53:33Z, these specific proposals were agent additions, origin agent unchanged; stage 2 not applicable (no origin_session); brief: docs/notes/2026-10-25-blocker-ranking-brief.md
  provenance: {"harness_session":"codex:01a178b3-d016-7562-a8e1-983f62cc0118","harness_session_source":"CODEX_THREAD_ID"}
