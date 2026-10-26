---
id: tasks-27ba8d
title: Show inherited urgency in ready without reordering it
status: idea
priority: 2
created: 2026-10-26T01:55:21Z
updated: 2026-10-26T15:00:32Z
depends: [crew-a0952b]
parent: tasks-46d207
tags: [cli]
source: tasks-e637a1
agent: claude-code/claude-opus-5-5
origin: agent
---

The blocker-ranking design's approach 2, deferred: keep ready's eligibility and order, but show a prerequisite's declared priority beside a temporary urgency inherited from an active beneficiary, with the path that confers it. Blocked on an active-beneficiary input contract (crew-a0952b's allocator owns those semantics); deriving urgency from every open dependent would invent a second policy. Reordering ready/next (approach 3) needs its own explicit reviewed choice.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-25-blocker-ranking-brief.md — shipped report boundaries and follow-on design questions

## Notes

- 2026-10-26T01:55:21Z (add): created
  provenance: {"harness_session":"claude-code:bafecb5c-78ad-4c0f-b8fc-6772b931692d","harness_session_source":"CLAUDE_CODE_SESSION_ID","origin":"agent"}
