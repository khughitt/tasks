---
id: tasks-2fa8c6
title: Agent identity and lifecycle
status: todo
priority: 1
size: l
complexity: high
process: planned
lane: true
created: 2026-09-30T09:58:25Z
updated: 2026-10-06T16:49:55Z
depends: []
tags: []
source: docs/notes/2026-09-30-delegated-identity-brief.md
agent: codex
---

Give workers stable identities, preserve claim ownership, and make controller completion account for the claims and events it actually observes. First milestone: tasks-c0bad3 establishes exact-release worker identity and hook semantics; tasks-5e2472 uses that finding for identity design, while tasks-f8c9cc settles controller-owned closure independently.

Resolve the reported missing/conflicting Codex identities using captured parent/worker evidence. Start with the bounded investigation linked in docs/notes/2026-09-30-delegated-identity-brief.md; do not change identity precedence or merge worker ownership into the controller by assumption. Done when each report has an evidence-backed disposition and any agreed fix and verification are complete. The conditional Claude live-check automation is related context, not a prerequisite.

Also group tasks-56b450's completion-policy handoff and tasks-27d515's claim-enumeration report here. The six conflict-to-fallback reports share tasks-5e2472's remediation and acceptance check; preserve their evidence and historical limits. Identity, provenance, parent association, closure authorization, claim enumeration, and Stop aggregation remain separate contracts. Use existing claim listings before adding a new enumeration surface. Coordinate shared claim/resolution changes with the record and location lanes; each child retains its process and review requirements. Done when all retained reports and follow-ups have verified delivery or explicit dispositions.

## Notes

- 2026-10-06T16:49:55Z (main): organization: adopted as a lane from the user's approved 2026-10-06 backlog organization; existing child records and their review gates remain intact.
