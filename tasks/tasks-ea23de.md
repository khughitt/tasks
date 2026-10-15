---
id: tasks-ea23de
title: Show why a claim is unverifiable in pretty claims and show output
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-10-15T01:48:42Z
updated: 2026-10-15T01:48:42Z
depends: []
parent: tasks-2fa8c6
tags: []
source: tasks-860db2
agent: claude-code/claude-opus-5-5
---

From the tasks-860db2 final review (Minor 2). tasks claims --pretty and show --pretty print 'unverifiable' without the reason the JSON carries in 'unverified' (spec docs/specs/2026-10-14-claim-pid-view-design.md §4: claims and show carry the reason). A person cannot tell a legacy claim from one recorded in another PID view. Append the reason in the claims table and the show claim line; add CLI tests for claims, show and prime --pretty.
