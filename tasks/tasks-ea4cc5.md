---
id: tasks-ea4cc5
title: "Amend the records-home design §4 and §5.4: hq rename-cutover refuses a detached project until it can save its records repository"
status: todo
priority: 3
size: xs
complexity: low
process: direct
created: 2026-10-23T12:42:07Z
updated: 2026-10-23T12:42:07Z
depends: []
tags: [docs]
source: hq-d05d5c
agent: claude-code/claude-opus-5-5
---

hq-d05d5c landed hq's two reader changes. session-episodes matches §4. rename-cutover does not: §4's row says it takes records for the record and tasks/files paths, and §5.4 says it reads the new location through tasks projects, implying a detached rename works. It now reads prefix and records through tasks projects but refuses any snapshot repository whose records lie outside it, because rollback resets only the repositories it saves and init --force --records cannot re-point a moved detached checkout (tasks-e9ce63). Amend both places to say so and name hq-5300fd as the piece that adds detached support.
