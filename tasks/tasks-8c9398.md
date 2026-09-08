---
id: tasks-8c9398
title: No way to rename a project prefix
status: doing
priority: 2
size: l
owner: design/prefix-rename
created: 2026-09-06T09:37:15Z
updated: 2026-09-08T20:53:27Z
depends: []
tags: [feedback, gap, "from:tasks", cli, registry]
spec: docs/specs/2026-09-08-prefix-rename-design.md
plan: docs/plans/2026-09-08-prefix-rename.md
---

Renaming a project alias means hand-editing three layers the binary owns: the registry key in ~/.config/tasks/projects.toml, `prefix` in the project's tasks/.config.toml, and every task id (filename + `id:` frontmatter). Inbound cross-project references in OTHER registered projects (depends lists, parent, note prose, spec/plan docs) go stale, and foreign-id resolution in `check` then fails there. Done by hand on 2026-09-06 for aut -> autonomy (empty project, trivial) and dot -> dots (2 task files, 4 inbound refs across ops and prism, 3 doc mentions).

Diagnosis: natural key vs surrogate key. The prefix is a mutable human label embedded in an immutable identifier, so renaming the label rewrites identity everywhere.

Rejected alternative: opaque ids (hex only) with the prefix demoted to a display alias. Rename becomes a one-line registry edit, but cross-project refs stop resolving without a global index (today the registry is a small alias->path map and prism-5bc782 self-describes its own home), collision pressure forces longer ids, and `depends: [5bc782, b32b3d]` tells a human reading a diff nothing. Reading an id is a daily act; renaming a prefix is once per project lifetime. Do not pay the common case to cheapen the rare one.

Design sketch - keep the id scheme, make rename first-class:

1. Cross-project writes are the one narrow exception. The tool already READS across projects (check's foreign-dep resolution, show, dep); only writes are project-local, and that asymmetry is policy rather than architecture. `tasks rename <old> <new>` gets --dry-run by default listing every file in every project it would touch, refuses if any target repo is dirty, and leaves one commit per repo.

2. Rewrite structured refs, report prose. depends and parent are structured and safe to rewrite mechanically; note bodies and docs/ markdown are not - regexing arbitrary prose corrupts history. In the dot -> dots migration that split was 2 auto-fixed against 5 reported. It also surfaces a decision the tool must not make silently: ops-645643's note read "moved to dot-a00088 after registering dotfiles", true when written, and rewriting it trades historical fidelity for resolvability.

3. Retired prefixes as read-only aliases. Let the registry keep `aliases = ["dot"]` on the dots entry so stale inbound refs still resolve. That decouples renaming the alias from rewriting the world, making the migration lazy rather than atomic across N repos, with `check` warning while any stale ref remains so the alias stays a transition rather than a permanent second name.
