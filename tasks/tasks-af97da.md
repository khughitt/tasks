---
id: tasks-af97da
title: No text search over task titles and bodies across registered projects; quick-add's near-duplicate check had to grep tasks/*.md files in each project directly
status: done
priority: 1
size: m
complexity: mid
process: direct
owner: feat/task-search
created: 2026-09-30T14:13:18Z
updated: 2026-10-06T09:27:42Z
started: 2026-10-06T09:17:32Z
completed: 2026-10-06T09:27:42Z
depends: [ops-c13647]
parent: tasks-ffdbaa
tags: [feedback, gap, "from:mind6"]
model: claude-opus-5-5
agent: claude-code/claude-opus-5-5
---

Why: Cross-project duplicate checks currently grep task files because the CLI has no text query. The existing list pipeline already supplies scoped scans, record filters, ordering, claims, warnings, and JSON rows.

Done: Add tasks search <query> with the same selection options as tasks list: status, all FilterArgs fields, local/-C/--project/--all-projects scope, sort/reverse, and parked/periodic/deferred modes with the same conflicts and default pools. Apply an additional literal substring predicate to each task's title or body, using Rust lowercase normalization on both query and text. Quoted multiword queries are one phrase; punctuation is literal. Reject empty or whitespace-only queries before scanning with a typed validation error. Search excludes generated frontmatter, notes, and attachment contents; source/tag/owner remain metadata filters. Return the existing list or parked JSON container and rows, including warnings; no matches is an empty tasks array with success. Retain pretty rendering and project colors.

Approach: Reuse ScopeArgs, FilterArgs, TaskFilter, open_read_ctx, and the list pipeline. Apply text matching while the full Task is still available, before mapping to summaries; do not grep serialized markdown or build an index. For --parked, match the title/body of the same registered-or-recorded copy that existing parked resolution chooses. A worktree-only parked task must remain searchable through scan_recorded/open_recorded; unresolved park-only entries cannot match text, and their existing availability warnings remain visible. Do not silently switch to a different checkout for body matching or change list behavior when no query is present.

Where to look: src/cli.rs::Command::List, src/commands/mod.rs command dispatch and ReadCtx, src/commands/list.rs::list and list_parked, src/commands/parked.rs::rows_preferring and open_recorded, src/filter.rs, src/scope.rs, src/output.rs::ListOut and ParkedOut, tests/cli.rs list/filter/parked/cross-project checks. Update README.md, skills/tasks/SKILL.md, the command reference in docs/specs/2026-08-29-tasks-design.md, and required CLI-surface declarations following the existing command conventions.

Verification: Focused CLI coverage must distinguish title-only, body-only, and note-only matches; confirm mixed case and literal punctuation/phrases, empty-query rejection and empty results; intersect text with representative shared filters and their repeat semantics; verify local/worktree scope and two registered projects under --all-projects. Exercise periodic/deferred/parked pools and a recorded-worktree-only body match, preserve unavailable-project warnings and parked resolution precedence, and prove pre-existing list calls are unchanged. Reuse the test front door: just test-one --test cli <filter>, then just test-fast. Rebuild and reinstall after implementation as required by the repository guide.

Original report: No text search over task titles and bodies across registered projects; quick-add's near-duplicate check had to grep tasks/*.md files in each project directly

## Notes

- 2026-09-30T21:48:54Z (main): Follow-up from dotfiles shell JSON search: implement tasks search over task content; accept the same filter options as tasks list (including --all-projects), then apply case-insensitive text search and return structured JSON.
- 2026-10-02T14:29:35Z (main): scope: scoped; literal case-insensitive title/body search reuses list selection, scopes, modes, warnings, and JSON; P2/m/mid/direct; brief: docs/notes/2026-10-02-cross-project-review-brief.md
- 2026-10-05T00:22:26Z (main): Raised to P1 on 2026-10-04: a quick-add pass filed a duplicate (mind6-c2cdb6 of mind6-b81107) because its near-duplicate check is keyword grep across projects; text search over titles and bodies is the fix.
- 2026-10-05T01:18:40Z (main): Review 2026-10-05: literal substring search helps discovery but will not reliably catch paraphrased duplicates (the mind6-c2cdb6/b81107 pair shared few words); keep the P1 for discovery and treat paraphrase detection as a separate question.
- 2026-10-06T09:17:32Z (main): started
  provenance: {"harness_session":"claude-code:5ea724da-a9ed-49f2-b579-b29c13ab5efd","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T09:17:41Z (feat/task-search): resumed
  provenance: {"harness_session":"claude-code:5ea724da-a9ed-49f2-b579-b29c13ab5efd","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T09:27:06Z (feat/task-search): review: impl round 1 — verdict: accept; findings: Minor 8; reviewer: claude-code/claude-opus-5-5
- 2026-10-06T09:27:42Z (feat/task-search): done
  provenance: {"harness_session":"claude-code:5ea724da-a9ed-49f2-b579-b29c13ab5efd","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-06T09:27:42Z (feat/task-search): tasks search <query>: literal case-insensitive title/body match over the full list pipeline (filters, scopes, modes, parked copy resolution), with docs, skill, and cli.toml row (ops-c13647)
  provenance: {"harness_session":"claude-code:5ea724da-a9ed-49f2-b579-b29c13ab5efd","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
