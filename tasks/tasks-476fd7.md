---
id: tasks-476fd7
title: An older tasks binary reading records written by a newer one reports a malformed record (bare scalar contains a reserved character; quote it) instead of saying the binary is older than the record format
status: idea
priority: 2
created: 2026-10-19T09:19:07Z
updated: 2026-10-19T09:19:07Z
depends: []
tags: [feedback, friction, "from:hq"]
agent: claude-code/claude-opus-5-5
---

A host with a stale binary ran tasks projects --paths through another tool; the parse error named a record line and told the reader to quote it, which reads as a corrupt record. Expected: an error naming a format newer than this binary and the upgrade route.
