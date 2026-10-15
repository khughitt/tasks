---
id: tasks-6360ac
title: Design overlay-side document roots in a linked project's code checkout
status: todo
priority: 2
size: m
complexity: high
process: planned
created: 2026-10-15T17:32:22Z
updated: 2026-10-15T17:32:22Z
depends: [tasks-82bd08]
parent: tasks-a73dce
tags: []
agent: claude-code
---

Why: a linked project's records (the overlay) may depend on its code repository, but nothing of tasks may reach the code repository (user, 2026-10-15). Today --spec/--plan resolve only under the overlay root (ops docs/specs/2026-10-01-linked-checkouts-design.md §168), so a task cannot link the team document it was built against. nexc-ad7d4e's services/nexml/docs/specs/workflow-feedback-design.md and tasks-3eda29's .worktrees/nex-342/docs/superpowers/specs/2026-10-14-hap-union-render-design.md survive only as note prose.

Done: a reviewed design and plan for read-only document roots inside the code checkout, declared only in the overlay's tasks/.config.toml (illustrative: spec_dirs = ["docs/specs", "checkout:services/nexml/docs/specs"]). It must settle: the stored link form, which code worktree a link resolves in (the one the command runs in, then the code main checkout), what check reports for a team document that exists only on an unmerged code branch (take tasks-82bd08's finding), behaviour when the checkout is missing, and the amendment to ops's linked-checkouts design. Invariant: nothing is written, configured or committed in the code repository.

Where: docs/notes/2026-10-15-branch-only-document-links-brief.md (alternative 3); src/resolve.rs (Resolver, resolve_doc, locate); src/repo.rs (Project.checkout, spec_dirs, plan_dirs); src/commands/check.rs (doc_missing); src/link.rs; the external document references spec, as the rejected-or-reused alternative.
