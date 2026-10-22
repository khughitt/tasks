---
id: tasks-d8823c
title: Clarify parked walkthrough evidence and execution policy
status: todo
priority: 2
size: s
complexity: mid
process: planned
created: 2026-10-22T20:35:03Z
updated: 2026-10-22T20:36:47Z
depends: [ops-e4d678]
tags: []
source: ops-e4d678
agent: codex
---

Accepted design: [ref:design], frozen ops e21ea75 (sha256 df835af8428626da6284972352edeffa749429e1c209fc183e62a3801f30c70a), accepted in spec round 10. Reuse that design; prepare the bounded implementation plan for review before execution. Design acceptance does not authorize live instruction, hook or harness activation.

Update skills/tasks/SKILL.md Session protocol step 5 ("start resumes it") to distinguish parked discussion and append-only evidence from execution. Add a policy amendment beside docs/specs/2026-10-08-claim-guard-policy-design.md without rewriting its historical accepted revision. No Rust or CLI extension is presumed.

Preserve the accepted protocol: release all own claims before any walkthrough question, native or ordinary; one evidence-note-and-show call on ordinary answer turns; inspection of current status before proceeding; same-turn start and work when an answer authorizes a fix or final work. Keep artifact writes, task-field changes, gate notes and exclusive resources behind acquisition. Preserve exact review-note shape, revision-scoped rounds, trial arming and user-stop/worker rules.

Done: reviewed candidate policy/skill text with acceptance assertions supplied to Hq. Supply a Tasks policy review of Hq's later evidence when requested; this candidate-text task need not remain open waiting for that dependent run. Activation remains gated on Hq behavioral acceptance and separate user authorization.

## References

- design (input, required, kind=spec): ops:docs/specs/2026-10-22-interactive-walkthrough-design.md — accepted frozen ops e21ea75

## Notes

- 2026-10-22T20:36:47Z (main): attached: walkthrough-design-e21ea75.md (33256 bytes): Accepted design frozen at ops e21ea75; sha256 df835af8428626da6284972352edeffa749429e1c209fc183e62a3801f30c70a
