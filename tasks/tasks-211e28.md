---
id: tasks-211e28
title: "Detached records: three locations per project, [records] registry table, init --records, rename, and the typed refusals"
status: todo
priority: 1
size: l
complexity: high
process: planned
created: 2026-10-22T20:54:24Z
updated: 2026-10-22T20:54:24Z
depends: []
tags: []
source: tasks-dc16eb
agent: claude-code/claude-fable-5-1
spec: docs/specs/2026-10-22-records-home-design.md
---

Implement the tasks CLI part of the approved design (tasks:docs/specs/2026-10-22-records-home-design.md §3, §5.1–§5.4, §12 cases 1–6): Project carries root, checkout, records and kind (in-tree|detached|linked); the records config declares [checkout] path and [checkout] kind = "detached"; the registry gains [records] <prefix>; locate opens a detached project from its code checkout and worktrees through the registry and refuses an in-tree config whose prefix is paired elsewhere (records_moved), a registered root with no config and no [records] row (records_unpaired), and init --records while the config is in the working tree or index (records_in_tree; a staged deletion is allowed). init --records takes the records root and derives <records>/<prefix>; rename moves that directory and the [records] key. projects, where and resolve rows gain records and kind, checkout is always present, resolve gains via records. Path references resolve records then root. The no-write test gains the detached case. CLI rows land on ops cli.toml first. Release order: the ops linked.py adapter piece must be installed on both hosts before this lands, since it rejects the new where shape.
