---
id: tasks-a73dce
title: Link a spec or plan that exists only on its task's branch
status: todo
priority: 2
created: 2026-10-15T16:55:23Z
updated: 2026-10-15T17:32:41Z
depends: []
parent: tasks-0e432f
tags: []
source: docs/notes/2026-10-15-branch-only-document-links-brief.md
agent: claude-code
---

Planned work writes its spec and plan on the task's branch, but add/edit refuse a document the writing checkout lacks: the main-checkout fallback runs worktree -> main only, and a linked project resolves documents only under its overlay. Done when tasks-cefa7d and tasks-3eda29 have evidence-backed dispositions and any agreed change has reviewed design, plan and verification. Invariant (user, 2026-10-15): a linked project's overlay may depend on its code repository, but nothing of tasks may reach the code repository: config, links and writes stay in the overlay. Research tasks-82bd08 first; design tasks-6360ac for overlay-side code-checkout roots. Coordinate the workflow with tasks-325e79's draft-plan linking. Handoff: docs/notes/2026-10-15-branch-only-document-links-brief.md.
