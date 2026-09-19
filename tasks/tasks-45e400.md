---
id: tasks-45e400
title: "dep --on and --rm are mutually exclusive, so adding and removing a dependency takes two invocations"
status: idea
priority: 2
created: 2026-09-19T00:50:15Z
updated: 2026-09-19T00:50:15Z
depends: []
tags: [feedback, friction, "from:material"]
agent: "claude-code/claude-opus-5[1m]"
---

tasks dep <id> --on <a> --rm <b> fails with 'cannot be used with'; expected one call to apply both edits
