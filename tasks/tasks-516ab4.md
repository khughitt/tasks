---
id: tasks-516ab4
title: "Feedback entries record no session provenance, so a scope pass must reconstruct a report's context by hand; stamp feedback with the reporting session as lifecycle notes are, and have the scope skill read the user's turns around it"
status: idea
priority: 2
created: 2026-10-20T11:06:59Z
updated: 2026-10-20T11:07:16Z
depends: []
tags: [feedback, idea, "from:lore"]
agent: claude-code
---

A pilot recovered the user's original request behind a title-only report only by finding the reporter's session through a stamp on a different task, then scanning that transcript for the filing call and filtering injected context from human turns. The request there separated two asks the title had merged, and the scope pass, lacking it, had wrongly folded one ask into another task. Proposal: tasks feedback stamps harness provenance at filing (as tasks note --stamp does); the scope skill's evidence step reads the human turns before the filing call through session-logs, and records a failed lookup as unavailable context. The entry carries only the pointer and the content stays in the local session store, so the describe-the-tool rule still holds. Retention depends on the session archive keeping transcripts readable by session-logs.

## Notes

- 2026-10-20T11:07:16Z (main): context: the user proposed this in the session stamped on this note (2026-10-20). The user asked either for capture when a report is filed (exact user input and a summary of the surrounding context) or for retroactive extraction from session logs, noting the risk that logs do not persist; and separately for a trace system across the task, harness, evidence and flow tools to locate where friction originates.
  provenance: {"harness_session":"claude-code:737a5757-83af-4e31-9fdd-4ce70dd9e6f1","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
