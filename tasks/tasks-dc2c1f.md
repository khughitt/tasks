---
id: tasks-dc2c1f
title: Include required reason in halt takeover hints
status: todo
priority: 2
size: xs
complexity: low
process: direct
created: 2026-09-30T16:22:28Z
updated: 2026-09-30T16:22:28Z
depends: []
tags: []
source: tasks-c543ae
agent: codex
---

Under a registered halt, ready and prime still suggest tasks start --force <id> for a stale claim, but start requires --reason. Make both takeover hints show a valid command under a halt while preserving unhalted wording. Cover both views in one focused CLI test.
