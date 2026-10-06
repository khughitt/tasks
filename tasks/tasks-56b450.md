---
id: tasks-56b450
title: Decide where claim-guard's claim policy lives; read the wake facts from harness support
status: todo
priority: 2
size: s
complexity: mid
process: planned
created: 2026-10-04T10:54:36Z
updated: 2026-10-06T10:59:45Z
depends: []
tags: []
source: ops-cb9749
agent: claude-code
---

Why: ops hooks/claim-guard decides whether a turn may stop given live claims and running children (tasks policy) and carries CHILD_WAKES, WAKING and IN_FLIGHT (capability facts). The agent layer split (ops docs/specs/2026-10-03-agent-layer-split-design.md §4.4, §5.3) dissolves hooks to their owners and leaves this decision to tasks.

Done: a recorded decision whether the claim rule moves into tasks or stays in ops until tasks takes it, and the facts read from harness support's consolidated view rather than the constants in the hook, in whichever order tasks chooses.

Hook boundary: use parent session_id with SubagentStart/SubagentStop agent_id and agent_type to associate workers; tasks-c0bad3 establishes the agent_id/CODEX_THREAD_ID mapping. Design around worker SubagentStop and controller Stop rather than the investigation's synthetic child Stop; verify actual event delivery separately from pure policy controls. Do not depend on child SessionEnd or Interrupt for cleanup.

## Notes

- 2026-10-06T10:07:12Z (investigate/tasks-1ece46): tasks-1ece46 capture separates worker claim identity from controller Stop aggregation: current hook matches only own live Stop session; pure injected-event check blocks controller on own claim and allows child with dead claim. Identity repair alone will not aggregate separately owned child claims; reuse this planned policy task. Evidence: docs/notes/2026-09-30-delegated-identity-brief.md.
- 2026-10-06T10:41:53Z (main): Identity review: current official Hooks docs specify parent session_id for subagent hooks and agent_id on subagent events. tasks-c0bad3 verifies the captured release; tasks-5e2472 evaluates worker thread identity plus parent association. Use that mapping as aggregation input, without merging claims or assuming actual Stop delivery was captured.
- 2026-10-06T10:59:44Z (main): Hook review: SubagentStart/SubagentStop carry parent session_id plus worker agent_id and agent_type. Design worker completion around SubagentStop plus agent_id and controller completion around its own Stop; do not require a child Stop or child SessionEnd/Interrupt. tasks-c0bad3 verifies agent_id equals CODEX_THREAD_ID and exact-release dispatch. The tasks-1ece46 injected child Stop with a child session_id exercises the pure decision function only; it is not a real subagent event or evidence of lifecycle coverage. Source: https://learn.chatgpt.com/docs/hooks.
