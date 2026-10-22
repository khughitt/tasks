---
id: tasks-ea0f60
title: Establish safe claim recovery when a resumed conversation changes session ID
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-22T23:10:50Z
updated: 2026-10-22T23:10:50Z
depends: []
parent: tasks-2fa8c6
tags: []
source: tasks-d8d97e
agent: codex
---

Question: What evidence can distinguish a legitimate resumed conversation with a new native ID from another live session, and what recovery is safe?
Where to start: [ref:brief] records the report, current ownership checks and missing capture. Trace src/claims.rs::proves_ownership, src/relay/resolve.rs::same_session, src/commands/park.rs and the relay identity spec.
Bound: Establish the exact incident operation and version if evidence is available; compare one genuine continuation with distinct-session and shared-process controls in disposable state. Check upstream issues, PRs, default/development branches and changelog before reproducing a suspected external defect. No live-claim takeover, host change or ownership implementation. If lineage cannot be obtained, record that limit and recommend explicit recovery.
Expected result: Record the evidence, authoritative lineage availability, owning component and smallest safe recovery recommendation on this task and in the brief. Preserve foreign-live refusal and distinguish observations from the original report.
Ideas it wakes: Before this result's branch merges, run tasks note tasks-d8d97e --no-reviewed "<finding>" with the recovery recommendation and evidence limits.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-22-session-continuity-brief.md — evidence, constraints and open questions
