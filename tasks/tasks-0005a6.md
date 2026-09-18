---
id: tasks-0005a6
title: Stamp gate notes with session provenance
status: idea
priority: 2
created: 2026-09-18T19:09:47Z
updated: 2026-09-18T19:09:47Z
depends: []
tags: [quick-add, cli, observability]
source: "mindful:thought:7b1b0c108103464388357acb293ad0ed"
agent: claude-code/claude-opus-5
---

Lifecycle markers (started, resumed, parked, done) carry harness_session provenance; plain notes do not. The flow skill records every state transition as a plain note with a gate: prefix, so a stage boundary has a timestamp but no session id, and per-stage cost attribution (obs-a6c7d4) has to infer the session from the nearest marker. Give a note a way to carry the same provenance: a --stamp flag on note, or a small set of recognised prefixes, whichever keeps the reader contract intact. Do not interpret gate text; tasks stays ignorant of flow's states.

Consumers use generated text, not the presence of provenance fields, to identify transitions, so stamping a user note must not make it look like a marker.

Source: mindful:thought:7b1b0c108103464388357acb293ad0ed
