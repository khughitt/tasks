---
id: tasks-9d2f66
title: Distinguish closeout hubs from real blockage in dependency counts
status: idea
priority: 2
created: 2026-10-26T01:55:26Z
updated: 2026-10-26T01:55:26Z
depends: []
tags: [cli]
source: tasks-e637a1
agent: claude-code/claude-opus-5-5
origin: agent
---

tasks blockers counts a childless cross-project closeout hub (a goal holding its pieces as depends) as ordinary work, because nothing in the schema says an edge is goal closeout rather than real blockage; the report states this inflation risk in every footer. Explicit goal or edge-purpose metadata would let it separate the two; it is a schema change needing its own reviewed design.

## Notes

- 2026-10-26T01:55:26Z (add): created
  provenance: {"harness_session":"claude-code:bafecb5c-78ad-4c0f-b8fc-6772b931692d","harness_session_source":"CLAUDE_CODE_SESSION_ID","origin":"agent"}
