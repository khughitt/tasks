---
id: tasks-8203e6
title: Project colours from familiar where a project has one
status: doing
priority: 2
size: m
complexity: high
process: planned
owner: main
created: 2026-10-06T13:01:34Z
updated: 2026-10-15T15:51:56Z
started: 2026-10-15T15:51:56Z
depends: [ops-b31634]
parent: tasks-1f05a7
tags: [pretty]
agent: claude-code/claude-opus-5-5
---

Why: no registered project sets color in tasks/.config.toml, so project colours never show. The user wants each project's colour taken from familiar (fam) where fam knows the project, then used for project prefixes, ids and titles in pretty output. fam pins projects to one of twelve slots (~/.config/familiar/identities.yaml; slot hue/sat frozen in familiar src/protocol/slot-hues.js; ramp() in src/theme/ramp.js gives scheme-dependent stops), and familiar whoami --json reports slot and hue but no RGB. Decide: how tasks gets fam's colour (a fam-published colour per project, a sync that writes tasks' color key, or a read at render time), precedence against an explicit color key, scheme (dark/light) handling, and slot sharing (12 slots, ~27 projects). Overlaps ops-b31634 (shared visual identity contract): reuse its finding.

## Notes

- 2026-10-15T13:33:46Z (main): consumer goal: ops-30b965 (every project has one colour, used wherever a project is named); the user re-requested this 2026-10-15 and noted the earlier request seemed lost
- 2026-10-15T14:00:05Z (main): finding (ops-b31634, 2026-10-15): no overlap between project and thought identity needs standardizing — thought colour is corpus- and scheme-relative (placement slots, seed noise, material), familiar's is a shared slot of 12 ramped per tone; project colour is one authored #rrggbb in tasks' color key (tasks/.config.toml), 256-colour stand-in derived; brief: docs/notes/2026-10-04-shared-appearance-brief.md; this task's premise (colour from familiar) no longer holds: familiar slots are shared families (20 pins over 12 slots) and publish no hex; re-scope as precedence/no-op once ops-40f2a6 sets every project's color key
- 2026-10-15T14:43:41Z (main): ops-40f2a6: every live project now sets color in its tasks/.config.toml (user accepted the swatch sheet 2026-10-15), so pretty output already paints prefixes; what remains of this task is whether ids/titles get the colour and any precedence question — familiar is no longer the source
- 2026-10-15T15:51:56Z (main): started
  provenance: {"harness_session":"claude-code:fc40c151-204a-41a6-a0d1-3d155d2ff9b2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
