---
id: tasks-4105af
title: Sync attach and detach rows into ops/cli.toml
status: todo
priority: 2
size: xs
complexity: low
process: direct
created: 2026-09-28T14:13:02Z
updated: 2026-09-28T14:13:02Z
depends: []
tags: [design]
agent: claude-code/claude-opus-5-5
---

tools/cli.toml here gained [[cli.tasks.commands]] rows for attach and detach (c7d640e) so the parser-surface conformance test passes. The authority is ops/cli.toml, vendored byte-identical by ops's just vendor-cli; until the same rows land there, the next re-vendor drops them and surface::tests::parser_surface_equals_table fails. Copy the two rows into ops/cli.toml, re-vendor, and confirm this repo's copy is byte-identical.
