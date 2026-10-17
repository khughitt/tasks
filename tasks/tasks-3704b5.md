---
id: tasks-3704b5
title: "A plain note written after a --reviewed note carrying 'proposal:' readmits the idea to the scope queue as stranded: pending-proposal detection reads only the latest note, while the scope skill says incidental notes do not change a reviewed idea's wait"
status: idea
priority: 2
created: 2026-10-17T21:15:38Z
updated: 2026-10-17T21:15:38Z
depends: []
tags: [feedback, friction, "from:prism"]
agent: claude-code/claude-opus-5-5
---

Sequence: tasks note <id> --reviewed 'scope: drop; ...; proposal: ...' (idea leaves list --needs-review), then tasks note <id> 'correction ...' (no flag; warns 'stays reviewed'), then list --needs-review shows the idea again with review_reason stranded. Expected: the proposal still counts as the recorded wait until answered with --no-reviewed. Worked around by restating the audit note with --reviewed.
