---
id: tasks-8efda8
title: "tasks edit --depends replaces the whole dependency list silently, unlike --tag which appends on edit; an agent adding one dependency dropped five"
status: done
priority: 2
size: s
complexity: mid
process: direct
owner: design/edit-depends
created: 2026-09-30T22:07:40Z
updated: 2026-10-02T15:56:56Z
started: 2026-10-02T15:56:53Z
completed: 2026-10-02T15:56:53Z
depends: []
parent: tasks-671956
tags: [feedback, friction, "from:sci"]
model: claude-opus-5-5
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
- 2026-10-02T15:56:53Z (design/edit-depends): started
  provenance: {"harness_session":"claude-code:e9d6021e-11dc-417c-a01a-7061cb2cb618","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-02T15:56:53Z (design/edit-depends): review: impl round 1 — verdict: accept; findings: Minor 3; reviewer: claude-code/claude-opus-5-5
- 2026-10-02T15:56:53Z (design/edit-depends): review fix: the already-present warning said 'nothing changed' and named dep's flags even when edit saved other fields; regraded Important, reworded to 'that dependency is unchanged' / 'tasks dep --rm' (tests RED→GREEN)
- 2026-10-02T15:56:53Z (design/edit-depends): done
  provenance: {"harness_session":"claude-code:e9d6021e-11dc-417c-a01a-7061cb2cb618","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-02T15:56:53Z (design/edit-depends): edit --depends appends (alias-aware dedup with warning, spellings kept, final graph validated) and --no-depends clears or replaces, via dep::add_dependencies shared with dep --on and add; docs and cli.toml (ops 2f5f4b7) updated
  provenance: {"harness_session":"claude-code:e9d6021e-11dc-417c-a01a-7061cb2cb618","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
