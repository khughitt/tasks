---
id: tasks-9c18e7
title: "dep accepts an edge to the task's own ancestor, which can never close first"
status: todo
priority: 2
size: m
complexity: mid
process: planned
created: 2026-10-17T12:51:55Z
updated: 2026-10-17T18:04:03Z
depends: []
tags: []
source: tasks-aeeef1
agent: claude-code/claude-opus-5-5
---

Why: `tasks dep <child> --on <its parent or ancestor>` succeeds, but `done` refuses a goal while any descendant is open (`Error::OpenDescendants`, `src/commands/mod.rs` ~1569), so the edge can never be satisfied: the child waits forever and, under wait-aware review, a reviewed idea with such an edge never wakes. Found during the wait-backfill freeze (auto entry mind6-573a5b would have written one; overridden to leave, see tasks-aeeef1).

Approach: the deadlock is a cycle in the closes-before graph, whose edges are each task's `depends` plus parent -> child (a goal closes after its children). The direct case is `child -> ancestor -> ... -> child`. Longer ones are possible too (C depends on X, X depends on C's parent). Today's cycle checks walk `depends` alone.

Established (checked 2026-10-17 at 8e1ff08):
- Every dependency write goes through `dep::add_dependencies` (`dep --on`, `add --depends`, `edit --depends`; `src/commands/dep.rs`, `src/commands/mod.rs` ~1034) and then `dep::ensure_acyclic`; the editor (`src/commands/edit.rs` ~418) and `move` preflight (`src/moving/preflight.rs` ~73) call `ensure_acyclic` directly. None reads the parent chain.
- Parents are same-project only (`hierarchy::validate_parent`), and it walks the ancestor chain from disk. It does not look at the task's or its subtree's `depends`, so a reparent can create the same deadlock (`edit X --parent P` where X, or a descendant of X, depends on P or on an ancestor of P).
- `check` reports dependency cycles (`cycle`) and parent cycles (`parent_cycle`) separately (`src/commands/check.rs` ~641) and does not report this combination.
- No open task among the 34 registered projects (1,400 open records, 420 with edges) depends on its own ancestor today, so `check` adds no findings on existing records.

Decide in the spec (planned): whether the write-time refusal covers the direct ancestor edge only or every closes-before cycle (the latter needs a child lookup per visited goal, i.e. a project scan, at write time); how a reparent checks its subtree's edges; the error (reuse `Error::Cycle` with the parent hops marked, or a new typed error); and `check`'s finding (lean: an error, like `cycle`, since the edge can never be satisfied).

Done: the reviewed spec's write paths (dep/add/edit `--depends`, editor save, `edit`/`add --parent`, `move`) refuse the deadlocking edge with a typed error that names the path; `check` reports any already on disk; tests in `tests/cli.rs` beside `dep_add_remove_and_local_cycle` (~5532); the tasks skill and README mention the refusal.

Where: `src/commands/dep.rs`, `src/hierarchy.rs` (`validate_parent`, `descendants`), `src/query.rs` `find_cycle`, `src/commands/check.rs`.
