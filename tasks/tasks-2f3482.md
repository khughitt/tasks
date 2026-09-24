---
id: tasks-2f3482
title: "sample --older-than rejects a bare 0, though zero needs no unit"
status: idea
priority: 2
created: 2026-09-24T13:35:30Z
updated: 2026-09-24T13:35:30Z
depends: []
tags: [feedback, friction, "from:tasks"]
agent: claude-code/claude-opus-5-5
---

tasks sample --older-than 0 fails with bad interval "0": expected a count followed by d or w; --older-than 0d works. Turning the age window off is the natural use of 0, and the unit carries no meaning there. Expected 0 to be accepted, or the error to suggest 0d.
