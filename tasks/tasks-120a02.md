---
id: tasks-120a02
title: Completion offers ids outside a project that the command then rejects
status: idea
priority: 3
created: 2026-09-05T23:00:36Z
updated: 2026-09-05T23:00:36Z
depends: []
tags: [cli]
spec: docs/specs/2026-09-05-shell-completions-design.md
---

From outside every project, 'tasks start fam-<TAB>' offers fam ids, but open_id_write_ctx fails with no_project. Same for show. Only root genuinely works there, because it resolves purely through the registry.

The spec sanctions this ('a missing local project is never an error in any scope') but it contradicts the spec's own contract line that offering an id the command will reject is a defect. Decide: either give root its own registry-wide scope and make IdDirected require a local project, or amend the spec to say why the blanket rule is right. Found by the final review of the completions branch, 2026-09-05.
