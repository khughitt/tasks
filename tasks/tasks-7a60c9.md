---
id: tasks-7a60c9
title: "show and doc have no read that never invokes a document provider: show fetches an external plan when a step is set, so a caller that must stay read-only cannot read a task's body, notes or stored links without possibly running the configured fetch command"
status: idea
priority: 2
created: 2026-10-15T15:05:35Z
updated: 2026-10-15T15:05:35Z
depends: []
tags: [feedback, gap, "from:crew"]
agent: claude-code/claude-opus-5-5
---

Wanted: a flag (e.g. --no-fetch) on show and doc. With it, external references come back as stored, step_found is omitted, doc reports an external reference as unavailable, and no provider command is ever spawned. Local documents still read. Use: a read-only sweep that must not run host-configured commands. Expected: a host with [docs.schemes] configured can be read without spawning the command; test with a provider that writes a sentinel file.
