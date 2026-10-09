---
id: tasks-96b215
title: Compare task records with the nodes on-disk standard
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-09-16T21:40:16Z
updated: 2026-10-08T10:58:21Z
depends: []
tags: [question]
agent: "claude-code/claude-opus-5[1m]"
---

Why: determine whether the science stack can index task records without a second task system or a tasks dependency on nodes. This is a compatibility investigation, not approval to change either format.

Question: Can the current nodes on-disk standard and reader consume an unchanged task record, including identity, frontmatter, typed relations and notes?

Where to start: the nodes project's docs/STANDARD.md and conformance entry point, resolved from its current registered checkout at execution; this checkout's src/frontmatter.rs, src/format.rs::parse_task/serialize_task, src/model.rs::Task/TaskId, and the MINIMAL/FULL fixtures in src/format.rs. The originating design reference is beliefs docs/specs/2026-08-29-user-and-autonomy (sections 4.1 and 4.4; verify its actual path/version). The external standard and beliefs design were not inspected in this local scope pass; do not assume their current contract.

Bound: read-only comparison of syntax and semantics, plus at most one representative fixture through the existing nodes conformance oracle. Cover prefix-plus-hex identity; depends/parent as task IDs versus spec/plan as document paths; required/optional and unknown keys; markdown body and reserved Notes section; provenance continuations. No task-format changes, library dependencies, adapters, sync layer or schema migration. If a required source is unavailable, report the exact missing source and stop short of a compatibility verdict.

Expected result: a field-by-field compatibility table citing the inspected revisions/sections, any oracle command and output, and a recommendation to consume the format unchanged, propose a separately scoped format-only change, or leave it independent. Distinguish successful markdown parsing from preserved relation semantics. Record the result on this task; any broader implementation requires separate scoping.

Ideas it wakes: on completion, resolve and run tasks note on beliefs-ff2529 with the finding and evidence pointer, paired with the completion result. Verify that waiting record's current status and authority first; never silently omit failed cross-project delivery. This task itself is the investigation and needs no duplicate research child.

## Original capture

Question: can the nodes STANDARD (nodes/docs/STANDARD.md) read a tasks record — markdown body, frontmatter, typed relations depends/parent/spec/plan, notes — as a node without tasks depending on the nodes library? Where to start: STANDARD.md's frontmatter and relation sections against src/frontmatter.rs and src/model.rs. Bound: a read-only comparison and, at most, a fixture of one task record run through the nodes conformance oracles; no format change. Expected result: a list of the differences (field names, relation encoding, identity) and a recommendation: conform by format only, or leave it. Why: the science stack's design (beliefs 2026-08-29 user-and-autonomy §4.1, §4.4) treats task/decision/note as coordination records; if tasks records are readable as nodes, science can index them without a second task system and without tasks learning the science stack exists. Ideas it wakes: none yet; the connection decision is a conversation in ai on 2026-09-16.

## Notes

- 2026-09-16T21:45:25Z (main): Waiting ideas: beliefs-ff2529 (the coordination seam) — note it with the finding when this resolves.
- 2026-10-08T10:58:20Z (main): scope: scoped; promoted the existing bounded read-only compatibility investigation to P2/s/mid/direct; added format/notes/provenance evidence points and a checkable comparison result; preserve original capture and wake beliefs-ff2529; external standard remains unverified until execution
