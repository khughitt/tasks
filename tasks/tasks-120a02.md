---
id: tasks-120a02
title: Completion offers ids outside a project that the command then rejects
status: idea
priority: 3
created: 2026-09-05T23:00:36Z
updated: 2026-09-09T11:05:50Z
depends: []
tags: [cli]
spec: docs/specs/2026-09-05-shell-completions-design.md
---

From outside every project, `tasks start fam-<TAB>` offers fam ids, but the command then fails with `no_project`; `show` behaves the same (reproduced 2026-09-09). Only `root` genuinely works there, since it resolves purely through the registry. The cause is that `id_directed` in src/complete.rs builds its candidates with `local_or_foreign`, which offers foreign ids whether or not a local project exists.

The completions spec (docs/specs/2026-09-05-shell-completions-design.md) sanctions this: a missing local project is never an error in any scope. It also states that offering an id the command will reject is a defect. The two rules conflict for every id-directed command except `root`.

## Open questions

- Which rule wins: give `root` its own registry-wide completion scope and make id-directed completion require a local project, or amend the spec to say why offering foreign ids everywhere is right.

Found by the final review of the completions branch, 2026-09-05.

## Notes

- 2026-09-09T11:05:50Z (design/curation): curate: decision; reproduced from outside every project on 2026-09-09, conflict stated under Open questions; proposal: pick the rule: a registry-wide completion scope for root alone with id-directed completion requiring a local project, or amend the completions spec to justify offering foreign ids everywhere
