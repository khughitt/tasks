---
id: tasks-8efda8
title: "tasks edit --depends replaces the whole dependency list silently, unlike --tag which appends on edit; an agent adding one dependency dropped five"
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-09-30T22:07:40Z
updated: 2026-10-02T15:46:49Z
depends: []
parent: tasks-671956
tags: [feedback, friction, "from:sci"]
agent: claude-code/claude-opus-5-5
plan: docs/plans/2026-10-02-edit-depends.md
---

Why: edit --depends replaces the entire dependency list in apply_fields, while edit --tag appends. The report describes five existing edges being lost when adding one. The replacement path is confirmed in code; the original incident has no attached transcript.

Decision to settle: Prefer additive edit --depends with an explicit clear/replace flag following the existing --no-tags pattern, or retain replacement and make it unmistakable in help and documentation. Check existing replacement use cases before changing the public command contract. No implementation is authorized by this idea yet.

Where to look: src/commands/mod.rs::apply_fields, src/commands/edit.rs::run, src/cli.rs::TaskFields, tests/cli.rs editor/dependency/tag checks, README.md, skills/tasks/SKILL.md, and docs/notes/2026-10-02-dependency-editing-brief.md.

Original report: tasks edit --depends replaces the whole dependency list silently, unlike --tag which appends on edit; an agent adding one dependency dropped five

## Notes

- 2026-10-02T14:22:40Z (main): scope: briefed; replacement confirmed but additive-versus-explicit-replacement contract remains unresolved; waits on tasks-e9af16; brief: docs/notes/2026-10-02-dependency-editing-brief.md
- 2026-10-02T15:46:48Z (design/edit-depends): finding (tasks-e9af16, approved): edit --depends appends with canonical dedup and the dep --on warning; --no-depends clears without a graph walk and with --depends replaces; add and editor unchanged; spec docs/specs/2026-10-02-edit-depends-design.md, plan docs/plans/2026-10-02-edit-depends.md
