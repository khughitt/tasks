---
id: tasks-8203e6
title: Project colours from familiar where a project has one
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-06T13:01:34Z
updated: 2026-10-06T16:49:59Z
depends: [ops-b31634]
parent: tasks-1f05a7
tags: [pretty]
agent: claude-code/claude-opus-5-5
---

Why: no registered project sets color in tasks/.config.toml, so project colours never show. The user wants each project's colour taken from familiar (fam) where fam knows the project, then used for project prefixes, ids and titles in pretty output. fam pins projects to one of twelve slots (~/.config/familiar/identities.yaml; slot hue/sat frozen in familiar src/protocol/slot-hues.js; ramp() in src/theme/ramp.js gives scheme-dependent stops), and familiar whoami --json reports slot and hue but no RGB. Decide: how tasks gets fam's colour (a fam-published colour per project, a sync that writes tasks' color key, or a read at render time), precedence against an explicit color key, scheme (dark/light) handling, and slot sharing (12 slots, ~27 projects). Overlaps ops-b31634 (shared visual identity contract): reuse its finding.
