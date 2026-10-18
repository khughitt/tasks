---
id: tasks-3704b5
title: "A plain note written after a --reviewed note carrying 'proposal:' readmits the idea to the scope queue as stranded: pending-proposal detection reads only the latest note, while the scope skill says incidental notes do not change a reviewed idea's wait"
status: dropped
priority: 2
created: 2026-10-17T21:15:38Z
updated: 2026-10-18T17:48:41Z
closed: 2026-10-18T17:48:41Z
depends: []
tags: [feedback, friction, "from:prism"]
agent: claude-code/claude-opus-5-5
---

Sequence: tasks note <id> --reviewed 'scope: drop; ...; proposal: ...' (idea leaves list --needs-review), then tasks note <id> 'correction ...' (no flag; warns 'stays reviewed'), then list --needs-review shows the idea again with review_reason stranded. Expected: the proposal still counts as the recorded wait until answered with --no-reviewed. Worked around by restating the audit note with --reviewed.

## Notes

- 2026-10-18T09:43:39Z (main): scope: drop; the behavior is what the user-reviewed wait-aware spec says (docs/specs/2026-10-17-wait-aware-review-design.md §4.1 and §8 case 6: any later note answers a pending proposal, the rule sample and tasks-22f407's list --pending share); the contradicting text is scope skill §1's 'incidental notes' sentence and the plain-note warning, which tasks-d2cdfe already lists for fixing; proposal: drop as covered by tasks-d2cdfe, whose fix should make the plain-note warning say the note answers the pending proposal and that restating with --reviewed keeps the wait; answer with --no-reviewed to keep the wait past plain notes instead
- 2026-10-18T17:48:41Z (main): dropped
  provenance: {"harness_session":"claude-code:3c5572c2-df02-48be-9b3a-d87e7474a975","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-18T17:48:41Z (main): dropped with user approval: covered by tasks-d2cdfe, which fixes scope skill §1 and the plain-note warning to match wait-aware spec §8 case 6 (a later plain note answers a pending proposal)
  provenance: {"harness_session":"claude-code:3c5572c2-df02-48be-9b3a-d87e7474a975","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
