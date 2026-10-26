---
id: tasks-6644b7
title: Establish detached record-hook validation of branch-only documents
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-26T15:19:02Z
updated: 2026-10-26T15:19:02Z
depends: []
parent: tasks-a73dce
tags: []
source: tasks-85e8cd
agent: codex
origin: agent
---

Question: Can automatic record commits validate a retained tracked document that exists or changes only in the invoking detached code worktree, and is placement/recovery guidance sufficient?
Where to start: [ref:brief] distinguishes the new-document placement error from the retained-document case; src/resolve.rs::Resolver::walk, src/repo.rs::Project::resume_root, src/record_commit.rs::commit_paths, the records repository hook route, and docs/specs/2026-10-22-records-home-design.md. Filing evidence: tasks-85e8cd, codex session 01a177f7-004b-7d00-8197-cb997c339b39 lines 135-157.
Bound: One disposable detached project with a code worktree and a separate records repository. Compare direct tasks check with the actual automatic commit hook for a new spec under records, a retained tracked spec present only on the code branch, and a tracked plan whose branch-only heading differs from main. Test missing and conflicting main/worktree documents. Isolate HOME, XDG_CONFIG_HOME, XDG_STATE_HOME and Git configuration; build subprocess environments key by key with fake credentials, unset inherited Git selectors, and assert fixture git roots before the first write. No product code, host hooks, shared pointers, sibling scanning or new CLI flags. Reuse existing fixture helpers and installed binaries by explicit path.
Expected result: Record the command/result table, hook entry context, lookup precedence, commit warnings and preserved record/index state on this task and in the brief. Recommend supported placement/publication guidance if sufficient, or identify acceptance cases for an explicit-context design under tasks-a73dce. Preserve hooks, record-only commits and historical links; do not invent a context transport before the fixture establishes the need.
Ideas it wakes: On completion, run `tasks note tasks-85e8cd --no-reviewed "<finding and recommendation>"` and update [ref:brief] before this result's branch merges.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-15-branch-only-document-links-brief.md — detached commit question and existing document-root constraints

## Notes

- 2026-10-26T15:19:02Z (add): created
  provenance: {"harness_session":"codex:01a178c5-fc4f-79b0-847a-9aa4826e274e","harness_session_source":"CODEX_THREAD_ID","origin":"agent"}
