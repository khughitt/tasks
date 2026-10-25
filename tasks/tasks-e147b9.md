---
id: tasks-e147b9
title: Establish safe full-ID attachment routing from scratch
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-24T11:32:20Z
updated: 2026-10-25T16:42:45Z
depends: []
parent: tasks-b34076
tags: []
source: "tasks-4b68c8:routing-investigation"
agent: codex
---

Question: What causes a full-ID attach from scratch beneath an empty ancestor .git to fail, and what is the smallest safe remedy given that show and note share its routing?

Where to start: [ref:brief] records code evidence at 89e0371 and the original report on tasks-4b68c8. The later show-only report tasks-a7a8ce fits the same matrix; routing code was rechecked at 220bb14 without a live reproduction. Trace src/commands/mod.rs::open_ctx/open_id_ctx_unwarned/open_id_write_ctx, src/repo.rs::Project::locate, src/link.rs::GitTree::of/obstruction, src/commands/attach.rs::attach, tests/attachments.rs::attach_routes_to_the_project_the_id_names, and the outside-project routing cases in tests/cli.rs. Compare 8e2f390 (outside-project IDs) and 328790d (marker diagnostic).

Bound: one disposable registered project and one scratch cwd; compare show, note and attach using the same explicit binary, first without a marker and then beneath a confirmed empty .git. Add one genuine broken-metadata control and one local/worktree precedence control. Record binary identity, cwd, relevant fixture environment, exit status and typed errors, and check absolute versus relative attachment source paths. Construct the subprocess environment key by key with temporary HOME and XDG config/state. Keep git off host config: GIT_CONFIG_GLOBAL to a temporary file or /dev/null, GIT_CONFIG_NOSYSTEM=1, fixture author/committer identity, and no inherited GIT selectors. Before a fixture git write, assert both its top level and absolute git dir are inside the fixture. Do not alter or remove any host marker. No production fix or generic resolver redesign.

Expected result: record the matrix and root-cause finding on this task and in the brief. Recommend existing checkout-based guidance, a targeted diagnostic correction, or a full-ID routing change; explain how real corrupt repositories and local/worktree preference remain protected, and whether reviewed design is needed before implementation. If the original show/attach contrast cannot be reproduced, say so without calling the source unavailable. Existing attachment-filing research tasks-4216a2 and park-input design tasks-57425c do not answer this question.

Ideas it wakes: On completion, run `tasks note tasks-4b68c8 --no-reviewed "<finding>"` and `tasks note tasks-a7a8ce --no-reviewed "<finding>"` before this result's branch merges.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-24-scratch-evidence-brief.md — evidence, close-time scope and routing questions
