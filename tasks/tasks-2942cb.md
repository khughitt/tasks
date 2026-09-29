---
id: tasks-2942cb
title: "dep --on a retired-prefix id is a silent no-op when the task already depends on it, so repointing a dependency to the renamed prefix by --on then --rm drops it entirely"
status: idea
priority: 2
created: 2026-09-29T20:37:17Z
updated: 2026-09-29T20:37:17Z
depends: []
tags: [feedback, friction, "from:tasks"]
agent: claude-code/claude-sonnet-5-5
---

tasks dep <id> --on tack-d5a56c then tasks dep <id> --rm ai-d5a56c, where ai is a retired alias of tack: the first call reports no warning and changes nothing because both ids resolve to one task, and the second removes the only copy. Expected: --on reports that the dependency already exists, or check's retired_prefix warning names a one-step repoint.
