---
id: tasks-4e8cef
title: "The scope skill fixes one project root per pass, so a cluster whose ideas live in different projects cannot be scoped together; the pass had to put one brief in the hub project and point the other members' audit notes at it by project-prefixed path"
status: idea
priority: 2
size: m
complexity: high
process: planned
created: 2026-09-30T14:28:44Z
updated: 2026-10-02T14:29:37Z
depends: []
parent: tasks-ffdbaa
tags: [feedback, gap, "from:mind6"]
agent: claude-code/claude-opus-5-5
---

Why: The scope skill rejects foreign IDs in an unscoped batch and fixes one root for evidence and writes. The reported workaround used one hub brief and project-qualified audit references, which is outside the current same-root contract.

Decision to settle: Extend the contract deliberately to a fixed root per participating project, with one handoff destination and safe cross-project associations. Preserve each existing parent and source; foreign parents are invalid, so hub coordination should use supported cross-project dependencies and member notes rather than foreign parent links. Specify stale/live-claimed member skips, partial failures, destination selection, and explicit reruns. This is the write-safety half of tasks-c591da, not a separate implementation or a reason to weaken local-root safety in current passes.

Where to look: skills/scope/SKILL.md steps 1, 4, and 6; docs/specs/2026-09-12-scope-pass-design.md; docs/specs/2026-09-04-multi-project-design.md; docs/notes/2026-09-13-scope-skill-validation.md; docs/notes/2026-10-02-cross-project-review-brief.md.

Original report: The scope skill fixes one project root per pass, so a cluster whose ideas live in different projects cannot be scoped together; the pass had to put one brief in the hub project and point the other members' audit notes at it by project-prefixed path

## Notes

- 2026-10-02T14:29:35Z (main): scope: briefed; mixed-project handoffs must preserve root ownership, existing parents, and rerun reuse; shares design tasks-91c622 with tasks-c591da; brief: docs/notes/2026-10-02-cross-project-review-brief.md
