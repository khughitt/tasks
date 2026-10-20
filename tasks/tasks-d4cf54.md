---
id: tasks-d4cf54
title: Pin task evidence references to the version actually reviewed
status: idea
priority: 3
complexity: high
process: planned
created: 2026-10-19T15:12:01Z
updated: 2026-10-20T01:41:14Z
depends: [tasks-c8ad5b]
parent: tasks-7102c1
tags: []
source: "tasks-3a445e:followup-pinned-refs"
agent: codex
---

Why: references currently select the current document. A report or review can change after the task closes, so an old citation no longer identifies the evidence that supported its decision. The implemented spec explicitly defers revision pinning; attaching a copy is the existing workaround. Capture this separately from document-root relocation and code locations.

Scope to settle: which concrete evidence needs stable versions; the smallest supported pin or snapshot representation; local Git revisions versus external documents and untracked artifacts; retained-byte/revision availability; and how deliberate reads, inventory and check distinguish a valid pin from unavailable historical evidence. Do not promise historical recovery merely because a path once existed. Preserve handles so prose need not change when a reference is pinned.

Done when scoped and, if pursued, a reviewed design defines version identity, retention limits and retrieval/validation behavior without expanding into a general artifact store. See docs/specs/2026-10-19-task-references-design.md section 11.

Scope handoff: [ref:brief] records the verified implementation limits, alternatives, and related work. Wait for tasks-c8ad5b to return its design or investigation finding, then re-scope this idea; no implementation approach is approved by this pass.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-19-reference-extensions-brief.md — evidence, alternatives, and the follow-up that wakes this idea

## Notes

- 2026-10-19T15:12:02Z (main): concerns: tasks-3a445e extension — retain the exact report or review version that supported a task decision
