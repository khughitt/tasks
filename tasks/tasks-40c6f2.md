---
id: tasks-40c6f2
title: "Claim release: deferred review minors"
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-10-15T10:17:32Z
updated: 2026-10-15T10:17:32Z
depends: []
parent: tasks-2fa8c6
tags: []
agent: claude-code/claude-opus-5-5
---

Deferred Minor findings from the tasks-f9fc01 final review and its re-reviews (spec docs/specs/2026-10-15-claim-release-design.md; ledger attached to tasks-f9fc01):
- release::open: when old < prefix the old lock is taken first; if the checkout's prefix changes during lock_and_revalidate, sorted order can break (deadlock against rename/move, or a self-wait when the new prefix equals old). Take no old lock before lock_and_revalidate, or drop and retry on a prefix change.
- require_reason runs after open's locks (spec §1 says before anything is read); move it into dispatch before release::open.
- occupants' release skip matches the session string only; two sandboxes sharing one worktree can both be sid:1. Skip only non-sid sessions, or say so in spec §5.
- move preflight: `commands::ownership(claim, &me)?` can surface a relay config error instead of `claimed`.
- rename --adopt hint: `TaskId::parse(id)?` turns a malformed old-store key into a parse error instead of `claimed`; fall back to no hint.
- Hints in ready/retire/rename can point at release for the caller's own unverifiable claim (release then refuses "yours").
- Under --adopting, the not-found guidance looks for tasks/<new>-<hex>.md in a worktree that predates the rename, so it offers only "the claim stays"; elsewhere() compares with project.root rather than resume_root().
- show.rs:71 suffixes Error::TaskNotFound so the detail reads "task X (claimed in …) not found"; use TaskNotFoundNext.
- Tests: the early-lock branch (old < prefix) never runs; releasing the gone session's second claim B is not asserted; rename/retire never assert no release hint for a live claim.
- README command-list line says "drop a claim" (another verb's word) and omits the Stale case.
