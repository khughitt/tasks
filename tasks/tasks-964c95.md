---
id: tasks-964c95
title: "Centralize task filtering and add priority, size, complexity, and process filters"
status: todo
priority: 2
size: m
complexity: mid
process: planned
created: 2026-09-30T16:40:28Z
updated: 2026-09-30T16:40:28Z
depends: []
tags: [cli]
agent: claude-code/claude-opus-5-5
---

list filters only on status, tag, owner, source, and parent; ready has --size (exact) and --parallel; ready/next have --max-complexity (a ceiling on effective complexity that hides unassessed). Nothing filters on priority or process.

Filter predicates are written inline where they are used: list's retain closure over Task (src/commands/list.rs, list) and list_parked's copy over ParkedRow; ready's size/parallel retains, each run twice (ready and deferred); the default open-not-shelved visibility repeated with small differences in list, hierarchy, graph, quiet, status, and tags. Only complexity has one home (src/complexity.rs: cutoff/effective/apply).

Goal: one TaskFilter in query.rs (or its own module) with matches(&Task, &ClaimSnapshot), a flattened FilterArgs shared by list and ready (and wherever else fits), list --parked filtering through the same predicate, and new filters for priority, size, complexity, and process. Complexity filtering must reuse complexity::effective. Open design points: exact vs range semantics for priority and complexity, how to select unassessed/unset values, whether the default visibility rule joins the filter, and JSON/CLI compatibility of the existing flags.
