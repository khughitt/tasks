---
id: tasks-56b450
title: Decide where claim-guard's claim policy lives; read the wake facts from harness support
status: todo
priority: 1
size: s
complexity: mid
process: planned
created: 2026-10-04T10:54:36Z
updated: 2026-10-06T17:05:32Z
depends: [tasks-c0bad3]
parent: tasks-2fa8c6
tags: []
source: ops-cb9749
agent: claude-code
---

Why: ops hooks/claim-guard decides whether a turn may stop given live claims and running children (tasks policy) and carries CHILD_WAKES, WAKING and IN_FLIGHT (capability facts). The agent layer split (ops docs/specs/2026-10-03-agent-layer-split-design.md §4.4, §5.3) dissolves hooks to their owners and leaves this decision to tasks.

Done: a recorded decision whether the claim rule moves into tasks or stays in ops until tasks takes it, and the facts read from harness support's consolidated view rather than the constants in the hook, in whichever order tasks chooses.

Hook boundary: use parent session_id with SubagentStart/SubagentStop agent_id and agent_type to associate workers; tasks-c0bad3 establishes the agent_id/CODEX_THREAD_ID mapping. Design around worker SubagentStop and controller Stop rather than the investigation's synthetic child Stop; verify actual event delivery separately from pure policy controls. Do not depend on child SessionEnd or Interrupt for cleanup.

Verified prerequisite: tasks-c0bad3 established SubagentStart/SubagentStop agent_id equals CODEX_THREAD_ID at Codex 0.160.1 commit d27764b82f7118f674371e6d6e76271d9d606edb. Hook session_id is the shared root ID, not necessarily the immediate spawning parent; root grouping alone cannot reconstruct nested ownership. Worker turn completion dispatches SubagentStop and may continue; it does not establish final claim closure. SessionEnd and Interrupt exclude workers. Design policy with that source mapping; actual hook delivery remains to be verified. Evidence: docs/notes/2026-09-30-delegated-identity-brief.md.

## Notes

- 2026-10-06T10:07:12Z (investigate/tasks-1ece46): tasks-1ece46 capture separates worker claim identity from controller Stop aggregation: current hook matches only own live Stop session; pure injected-event check blocks controller on own claim and allows child with dead claim. Identity repair alone will not aggregate separately owned child claims; reuse this planned policy task. Evidence: docs/notes/2026-09-30-delegated-identity-brief.md.
- 2026-10-06T10:41:53Z (main): Identity review: current official Hooks docs specify parent session_id for subagent hooks and agent_id on subagent events. tasks-c0bad3 verifies the captured release; tasks-5e2472 evaluates worker thread identity plus parent association. Use that mapping as aggregation input, without merging claims or assuming actual Stop delivery was captured.
- 2026-10-06T10:59:44Z (main): Hook review: SubagentStart/SubagentStop carry parent session_id plus worker agent_id and agent_type. Design worker completion around SubagentStop plus agent_id and controller completion around its own Stop; do not require a child Stop or child SessionEnd/Interrupt. tasks-c0bad3 verifies agent_id equals CODEX_THREAD_ID and exact-release dispatch. The tasks-1ece46 injected child Stop with a child session_id exercises the pure decision function only; it is not a real subagent event or evidence of lifecycle coverage. Source: https://learn.chatgpt.com/docs/hooks.
- 2026-10-06T15:22:25Z (main): Hand-off from the residue (tack docs/specs/2026-10-06-residue-design.md §4.4, §4.7): the three facts live in tack facts/capabilities.toml and are read only through tack tools/harness-facts (get/list, or lookup(fact, harness) as a loaded module; the registry locates the checkout). When the guard reads them, delete two things with the constants: ops tests.test_claim_guard.FactsMirrorTests and the mirror step in tack's .githooks/pre-commit (with just facts-mirror).
- 2026-10-06T16:50:05Z (main): priority: raised to P1 under the adopted agent-identity lane; identity research precedes dependent design, and controller-owned closure stays a separate decision.
- 2026-10-06T16:50:05Z (main): dependency: tasks-c0bad3 supplies the worker/parent/event mapping needed for completion-policy design; capability-facts sourcing and ownership can be discussed independently, but closure requires the verified mapping.
- 2026-10-06T17:05:31Z (investigate/tasks-c0bad3): Exact Codex 0.160.1 source at d27764b82f7118f674371e6d6e76271d9d606edb deliberately writes shared root CODEX_SESSION_ID and concrete CODEX_THREAD_ID; SubagentStart/SubagentStop agent_id equals worker thread ID. Worker completion dispatches SubagentStop, not Stop; worker SessionEnd/Interrupt are excluded. Recommend tasks contract adaptation, not upstream export repair. Root grouping is not an immediate-parent link for nested workers. Findings and immutable source links: docs/notes/2026-09-30-delegated-identity-brief.md; live hook delivery remains unverified.
