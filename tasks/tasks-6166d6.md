---
id: tasks-6166d6
title: Make high-impact and persistent blockers visible
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-25T21:37:28Z
updated: 2026-10-25T22:00:16Z
depends: []
parent: tasks-46d207
tags: []
agent: codex
origin: agent
---

Surface and prioritize blockers by downstream impact, and establish trustworthy evidence for persistent blocking. First milestone: design and ship forward dependency-event and scope-confirmation collection early, while the ranked downstream report proceeds independently. History coverage begins at deployment; do not delay capture for report UI or optional backfill.

Why: [ref:brief] separates the two user-requested measures and records the reviewed correction to a single combined design. There are exactly two substantive children: tasks-e637a1 (ranked downstream reporting) and tasks-71c74d (dependency events and confirmed-wait observations). Both require their own design and implementation-plan reviews; no new records were added for this revision.

Done: an explainable ranked downstream blocker report is shipped with explicit coverage and user-held-blocker cases, and forward dependency/confirmed-wait collection is shipped with documented coverage start and interval semantics. Any duration view must expose unknown pre-collection history; optional git backfill gets an explicit disposition and does not gate the first two deliverables. Each child retains accepted implementation steps beneath it and closes only after its remaining work has a final disposition. Closing merely because an idea was promoted or a design was written does not satisfy this goal.

Where to look: [ref:brief], tasks-e637a1, tasks-71c74d, tasks-e8d471, tasks-606dd3, tasks-298107 and crew-a0952b. Keep report ranking explicit; leave ready/next ordering unchanged by default, with display-only inherited urgency retained as a design alternative. This remains an ordinary child of Work discovery and resumption (tasks-46d207), not a new lane. The required brief reference is the sole document link; the redundant agent-added source copy is withdrawn.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-25-blocker-ranking-brief.md — originating ask, graph evidence, reporting alternatives and open decisions

## Notes

- 2026-10-25T21:37:28Z (add): created
  provenance: {"harness_session":"codex:01a174fb-0fa0-7571-93ce-b3218da174d1","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
