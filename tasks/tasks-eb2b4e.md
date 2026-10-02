---
id: tasks-eb2b4e
title: "A shared tag vocabulary: tasks draws tags from layered sources, with mindful as an optional provider"
status: idea
priority: 2
size: m
complexity: high
process: planned
created: 2026-09-30T14:23:41Z
updated: 2026-10-02T15:14:13Z
depends: []
parent: tasks-cea445
tags: [cross-project]
source: "user:2026-09-30 thought-sweep session"
agent: claude-code/claude-opus-5-5
---

Problem: tag meaning lives in two places. tasks keeps a per-project dictionary ([tags] in tasks/.config.toml, tasks-603ef9; check warns undefined_tag), and mindful keeps tags as thoughts with an alias and a body. A tag that makes sense on both sides (question, cross-project, quick-add, subject tags) is defined separately or not at all: quick-add filed obs-b3b6a0 with tag question, which obs does not define, and had to drop it (ops-d93da8).

Want: when both tools are installed, tasks and mindful draw from one common set of tags; a user who installs only tasks loses nothing.

Possible shape (to settle when scoped):
- tasks resolves a tag dictionary from layers: the project's [tags], then a shared dictionary for every registered project (e.g. beside projects.toml in the tasks config dir), then an optional external provider named in the global config (a command that prints tags as JSON). Only the first layer is required.
- mindful is one such provider: an op or CLI command that lists tag thoughts (alias, one-line meaning from the body, archived state). It could also import the shared dictionary as tag thoughts.
- tasks check and quick-add validate against the resolved set; the provider is read at check time or cached, so tasks never needs the mindful service running.

Open questions: which side is authoritative when both define a tag (the shared file, or mindful's tag thoughts); whether project dictionaries can narrow or rename shared tags; how renames and aliases propagate; whether tag meanings sync both ways or only mindful -> tasks; offline and missing-provider behaviour (fail, or warn and fall back to local layers).

## Notes

- 2026-10-02T15:14:13Z (main): scope: briefed; shared definitions need authority, precedence, offline, alias, and enforcement decisions; preserve the full captured proposal and source; waits on tasks-6ac7fd; brief: docs/notes/2026-10-02-tag-vocabulary-brief.md
