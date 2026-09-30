---
id: tasks-2bb0ba
title: "A run: note entered with a wrong total cannot be corrected: notes are append-only, and a free-text correction note is invisible to the parsers that read run: notes, so estimate-vs-actual measures keep the wrong figure"
status: idea
priority: 2
created: 2026-09-30T09:10:59Z
updated: 2026-09-30T09:54:02Z
depends: []
parent: tasks-ea2a79
tags: [feedback, gap, "from:material"]
agent: claude-code/claude-opus-5-5
---

tasks edit refuses any note change (notes are append-only). A second run: line would count as a second attempt. Expected: a sanctioned correction form (e.g. 'run-correction:' or an amend flag) that the run-note parsers honour.

## Notes

- 2026-09-30T09:54:02Z (main): scope: briefed; correction semantics must identify one original attempt and agree with downstream run-note consumers; waits on tasks-1f6d08; brief: docs/notes/2026-09-30-note-integrity-brief.md
