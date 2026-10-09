---
id: tasks-2f1476
title: "edit has no precondition on the record's updated stamp, so curate's revalidate-then-write leaves a window it documents as accepted"
status: idea
priority: 2
created: 2026-10-08T20:35:39Z
updated: 2026-10-08T20:35:39Z
depends: []
tags: [feedback, idea, "from:ops"]
agent: claude-code/claude-opus-5-5
---

curate re-reads show, compares task.updated, then edits. An option such as edit --if-updated <stamp> that refuses on mismatch would close the race the skill accepts and drop the separate re-read.
