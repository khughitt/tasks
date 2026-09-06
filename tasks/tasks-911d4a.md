---
id: tasks-911d4a
title: "Sort projects by prefix, size, or last activity"
status: done
priority: 2
size: s
owner: projects-table
created: 2026-09-06T10:13:57Z
updated: 2026-09-06T10:48:41Z
depends: [tasks-a52e0f]
parent: tasks-fe4716
tags: [cli]
---

Add `--sort` and `--reverse` to `projects`. Command-level, not pretty-only: it reorders the JSON array too.

Keys: `prefix` (default, the current alphabetical order), `size` (the new `total` field, largest first), `activity` (most recent `last_activity` first). Ties break on prefix so the order is total and runs are reproducible.

Do not extend `query::SortKey` for this. That enum is task-shaped - priority, updated, created - and its `date_column()` method answers a question projects do not ask. A separate small enum in the projects command, with its own `complete::project_sorts` candidate function, reuses the `--sort`/`--reverse` flag shape and the completion pattern without welding two unrelated domains together.

Unreachable projects have no counts and no activity, so they sort last under every key regardless of direction - they are absent data, not zeroes, and burying them under `--reverse` would hide the thing most worth noticing.

## Notes

- 2026-09-06T10:48:41Z (projects-table): projects gains --sort prefix|size|activity and --reverse, command-level so JSON reorders too; rows lacking the key sink in prefix order under every direction
