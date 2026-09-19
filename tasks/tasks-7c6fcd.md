---
id: tasks-7c6fcd
title: "Notes accept trailing whitespace that whitespace gates reject, but append-only validation prevents even a whitespace-only repair"
status: idea
priority: 2
created: 2026-09-18T15:31:41Z
updated: 2026-09-18T15:31:41Z
depends: []
tags: [feedback, friction, "from:obs"]
agent: codex
---

The note command accepts a single-line value ending in a space. A whitespace-check gate then fails on the generated record. Editing only that trailing space through the editor command is rejected as an append-only note change. Trim trailing whitespace at insertion or allow this normalization through the CLI.
