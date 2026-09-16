---
id: tasks-96b215
title: Does a task record conform to the nodes on-disk standard as-is?
status: idea
priority: 2
created: 2026-09-16T21:40:16Z
updated: 2026-09-16T21:45:25Z
depends: []
tags: [question]
agent: "claude-code/claude-opus-5[1m]"
---

Question: can the nodes STANDARD (nodes/docs/STANDARD.md) read a tasks record — markdown body, frontmatter, typed relations depends/parent/spec/plan, notes — as a node without tasks depending on the nodes library? Where to start: STANDARD.md's frontmatter and relation sections against src/frontmatter.rs and src/model.rs. Bound: a read-only comparison and, at most, a fixture of one task record run through the nodes conformance oracles; no format change. Expected result: a list of the differences (field names, relation encoding, identity) and a recommendation: conform by format only, or leave it. Why: the science stack's design (beliefs 2026-08-29 user-and-autonomy §4.1, §4.4) treats task/decision/note as coordination records; if tasks records are readable as nodes, science can index them without a second task system and without tasks learning the science stack exists. Ideas it wakes: none yet; the connection decision is a conversation in ai on 2026-09-16.

## Notes

- 2026-09-16T21:45:25Z (main): Waiting ideas: beliefs-ff2529 (the coordination seam) — note it with the finding when this resolves.
