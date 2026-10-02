---
id: tasks-8efda8
title: "tasks edit --depends replaces the whole dependency list silently, unlike --tag which appends on edit; an agent adding one dependency dropped five"
status: idea
priority: 2
size: m
complexity: high
process: planned
created: 2026-09-30T22:07:40Z
updated: 2026-10-02T14:22:42Z
depends: []
parent: tasks-671956
tags: [feedback, friction, "from:sci"]
agent: claude-code/claude-opus-5-5
---

Why: edit --depends replaces the entire dependency list in apply_fields, while edit --tag appends. The report describes five existing edges being lost when adding one. The replacement path is confirmed in code; the original incident has no attached transcript.

Decision to settle: Prefer additive edit --depends with an explicit clear/replace flag following the existing --no-tags pattern, or retain replacement and make it unmistakable in help and documentation. Check existing replacement use cases before changing the public command contract. No implementation is authorized by this idea yet.

Where to look: src/commands/mod.rs::apply_fields, src/commands/edit.rs::run, src/cli.rs::TaskFields, tests/cli.rs editor/dependency/tag checks, README.md, skills/tasks/SKILL.md, and docs/notes/2026-10-02-dependency-editing-brief.md.

Original report: tasks edit --depends replaces the whole dependency list silently, unlike --tag which appends on edit; an agent adding one dependency dropped five

## Notes

- 2026-10-02T14:22:40Z (main): scope: briefed; replacement confirmed but additive-versus-explicit-replacement contract remains unresolved; waits on tasks-e9af16; brief: docs/notes/2026-10-02-dependency-editing-brief.md
