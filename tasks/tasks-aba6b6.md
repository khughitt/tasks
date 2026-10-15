---
id: tasks-aba6b6
title: Repoint relay fixtures README at relay's committed registry-workers.json
status: todo
priority: 3
size: xs
complexity: low
process: direct
created: 2026-10-15T12:38:28Z
updated: 2026-10-15T12:38:28Z
depends: []
tags: [relay]
agent: claude-code/claude-opus-5-5
---

tests/fixtures/relay/README.md sources registry-workers.json from relay plan revision 74ce0d8. Once relay's c7074c8 (relay-6825f7 branch, byte-identical per tasks-af3aac's final review) is on relay main, cmp the bytes and replace the source with that relay revision of test/fixtures/registry-workers.json. A byte difference is a contract change to settle with relay.
