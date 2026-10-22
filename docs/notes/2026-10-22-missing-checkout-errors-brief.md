# Distinguishing absent checkouts from read failures

Scoping handoff, 2026-10-22. Idea: tasks-d6bd82. Lane: tasks-a7460c.
Not an approved design.

## Problem

A read-only consumer needs to distinguish a removed checkout from a real read
failure without a separate filesystem check racing the tasks command. The report
asks about both a missing directory and a surviving directory with lost Git metadata.

## Current behaviour and evidence

Checked the main checkout at 2ce5663. src/repo.rs::Project::locate begins with
canonicalize()?; src/error.rs converts std::io::Error into a string-backed Io
variant whose JSON kind is io. The original error category is therefore lost.
src/link.rs::GitTree::of distinguishes ordinary non-repository discovery from
an obstructing .git marker, and otherwise retains Git failures as io.
Commits 328790d and e4c9100 added marker diagnostics and bounded discovery.
The existing docs/notes/2026-10-08-project-discovery-movement-brief.md explicitly
preserves corrupt-metadata errors.

The report observed io for show/list on an absent checkout and a Git exit 128 for
a half-removed worktree. This pass traced code but did not reproduce those states.
A directory with no .git is not by itself evidence of a removed worktree.

## Constraints

Preserve corrupt-repository, permission and other genuine I/O failures.
Keep ordinary no-project and healthy unborn-repository behavior intact.
JSON error kinds are a consumer contract; do not silently make every Git exit 128
mean absence. Active detached-record work (tasks-211e28) changes discovery, so
the investigator must recheck its integration state before choosing a remedy.
tasks-7cae19 addresses GIT_TRACE and unborn repositories, a separate question.

## Alternatives

1. Type only proven absence at the shared path-resolution boundary and retain
   ambiguous or corrupt metadata as an error.
2. Extend absence classification to removed worktrees using authoritative registry
   or Git worktree evidence, if available and reliable at lookup time.
3. Keep generic io and require caller-side filesystem classification.

Lean toward option 1 as the minimum contract, while testing whether option 2 can
safely cover the report's second ask. Reject treating all Git failures as absence.
Option 3 preserves behavior but leaves the reported race and duplicated logic.

## Unanswered questions

- Which shared boundary covers show, list and relevant discovery callers, and
  which underlying error categories survive there? The research owner traces it.
- Can a half-removed worktree be distinguished from corrupt metadata without
  swallowing errors? A disposable fixture matrix must answer, or retain it as unknown.
- What exact JSON kinds and compatibility treatment should consumers receive?
  Research recommends the smallest supported contract with negative controls.

## Proposed decomposition

tasks-7b1b22 (P2/s/mid/direct) under tasks-a7460c establishes the classification matrix
and proposed error contract; tasks-d6bd82 waits on it. It must address both reported
states explicitly. No implementation or additional design task is filed yet.
