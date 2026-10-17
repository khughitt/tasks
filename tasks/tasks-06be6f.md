---
id: tasks-06be6f
title: Retire the wait-backfill tool and its state once every host runs the wait-aware binary
status: todo
priority: 3
size: xs
complexity: low
process: direct
defer: 2026-10-31
created: 2026-10-17T17:45:14Z
updated: 2026-10-17T17:45:30Z
depends: []
tags: []
source: tasks-aeeef1
agent: claude-code/claude-opus-5-5
---

The wait-aware rollout ran on titan only (2026-10-17); every other host that reads synced records must run cargo install --path . from its synced main checkout before it next runs tasks. Once confirmed: remove tools/wait-backfill and tests/wait_backfill.rs (one-time tool, as review-backfill before it — check whether that one can go too), and delete ~/.local/state/tasks/wait-backfill (snapshots, judge-review.md, judge-proposals.json, judge-decisions.json) after nothing needs them.

## Notes

- 2026-10-17T17:45:30Z (main): concerns: tasks-aeeef1 extension — retire the one-time backfill tool and state after every host upgrades
