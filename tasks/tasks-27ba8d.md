---
id: tasks-27ba8d
title: Show inherited urgency in ready without reordering it
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-26T15:00:34Z
created: 2026-10-26T01:55:21Z
updated: 2026-10-26T15:00:34Z
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
- 2026-10-26T15:00:34Z (main): scope: briefed; retained full body/source; reused work-discovery lane and existing brief; allocator completion readmits the idea for contract reassessment, not automatic urgency implementation; crew-a0952b still owns active beneficiaries and says no tasks feature is needed yet; wait recorded on crew-a0952b; premises checked against 9cd4699 and shipped report merge 074672f; origin: stage 1 inferred; unique filing call toolu_01Ln8QArefcMFd4m21KrRVfn in claude-code:bafecb5c-78ad-4c0f-b8fc-6772b931692d at 2026-10-26T01:55:14Z, captured id-only jq result at 01:55:35Z with no reuse warning; user asked generically for follow-ups at 01:53:33Z, these specific proposals were agent additions, origin agent unchanged; stage 2 not applicable (no origin_session); brief: docs/notes/2026-10-25-blocker-ranking-brief.md
  provenance: {"harness_session":"codex:01a178b3-d016-7562-a8e1-983f62cc0118","harness_session_source":"CODEX_THREAD_ID"}
