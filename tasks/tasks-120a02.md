---
id: tasks-120a02
title: Id-directed commands work from outside every project by routing through the registry
status: todo
priority: 3
size: s
created: 2026-09-05T23:00:36Z
updated: 2026-09-09T11:36:44Z
depends: []
tags: [cli]
spec: docs/specs/2026-09-05-shell-completions-design.md
---

From outside every project, `tasks start fam-<TAB>` offers fam ids, but the command fails with `no_project`; `show` behaves the same (reproduced 2026-09-09). Only `root` works there, since it resolves purely through the registry. The completions spec (docs/specs/2026-09-05-shell-completions-design.md) says offering an id the command will reject is a defect, while the multi-project design keeps a local project mandatory for every write but `add --project`, with no stated reason for id-directed commands: their prefix already names the target, and outside every project the registry is the only candidate.

Done: `open_id_write_ctx` and the id-directed read path fall through to `open_registered` by the id's prefix when no local project exists, instead of failing; inside a project the current rule (local checkout when the prefix matches, else the registry) is unchanged. The multi-project design's "exactly one exception" sentence gains this second exception, with the reason above. `feedback` keeps its local project: its `from:` tag needs one. Tests: `show`, `note`, and `start` on a registered id from a temp dir succeed and write to the registered root; an unregistered prefix still errors; completion from the same temp dir offers only ids the command accepts.

## Notes

- 2026-09-09T11:05:50Z (design/curation): curate: decision; reproduced from outside every project on 2026-09-09, conflict stated under Open questions; proposal: pick the rule: a registry-wide completion scope for root alone with id-directed completion requiring a local project, or amend the completions spec to justify offering foreign ids everywhere
- 2026-09-09T11:36:44Z (main): decision (2026-09-09): neither option; id-directed commands run from outside every project by routing through the registry when no local project exists, as root already does. Completion then offers exactly what the command accepts.
