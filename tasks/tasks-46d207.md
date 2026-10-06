---
id: tasks-46d207
title: Work discovery and resumption
status: todo
priority: 3
size: l
complexity: high
process: planned
lane: true
created: 2026-09-30T10:04:29Z
updated: 2026-10-06T16:54:21Z
depends: []
tags: []
source: docs/notes/2026-09-30-work-selection-brief.md
agent: codex
---

Find useful work across projects and notice when waiting work can resume while respecting ownership, dependencies, resource limits, and human review. First milestone: tasks-325e79 establishes safe draft-plan linking and step eligibility. Dated resumption under tasks-157d05 and cross-project scoping under tasks-ffdbaa proceed independently using the existing picker and registry-wide read contracts.

Preserve the existing selection/needs children and tasks-ffdbaa's child tree. Add tasks-76d269's list limits and tasks-b2e718's plan-review eligibility report. Calendar dates must not authorize user-held work, and linked draft plans must not make unreviewed implementation executable by accident. Before proposing new review state, establish the warning behavior and safest existing plan-linking workflow.

Groups and lanes are delivered context, not prerequisites to redesign. Harden the existing holds and picker tests, retain current JSON and filter contracts, and coordinate multi-project handoffs with record publication and root ownership. Done when retained children have completed or received explicit dispositions; preserve each child's design and plan review requirements. Shelved work requires an explicit revisit or disposition before closeout.

## Notes

- 2026-10-06T16:49:56Z (main): organization: adopted as a lane from the user's approved 2026-10-06 backlog organization; existing child records and their review gates remain intact.
