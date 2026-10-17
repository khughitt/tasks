---
id: tasks-f3910b
title: "scope queue (--needs-review) on the main checkout lists an idea that an unmerged branch has already promoted to a todo goal with children; the scope skill has no rule for this, and a main-side audit note would conflict with the branch's record of the task"
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-10-15T18:15:26Z
updated: 2026-10-17T18:10:42Z
depends: []
tags: [feedback, gap, "from:material"]
agent: claude-code
---

Why: a scope pass on the main checkout can pick an idea that an unmerged branch has already promoted (for example to a todo goal with children). On main it looks unreviewed, so the pass spends its evidence reads on it, and its verdict either fails at the write or, with no worktree left, lands on main and conflicts with the branch's record at merge. Reported from material sessions; duplicates tasks-16de1c and tasks-2b2b49 are dropped. Brief: docs/notes/2026-10-16-scope-queue-other-branches-brief.md.

Decision (scope pass 2026-10-17; the user may override before start): the brief's lean, alternatives 2 + 1. Rejected: alternative 3 (the queue filters or flags rows), because it changes what `--needs-review` means and adds a sibling read per row to a listing, while the warning reaches the pass at its first `show`, before any reads.

Established (reproduced 2026-10-16 in a disposable project, re-checked against 8e1ff08):
- Worktree present: from main, `list --needs-review` lists the idea, and `show` reports `status: idea`, no claim and `warnings: []`, so scope §6 revalidation passes. Only the write fails, with `stale_copy` and a "Run it there: tasks -C <wt> note ..." retry, which would put a scope note on the branch's todo.
- Worktree removed, branch kept: the same `note --reviewed` exits 0 and writes main's copy.
- Writes check through `refuse_stale_copy` (`src/commands/mod.rs` ~1114, edit, release, feedback, move preflight) on `stale::compare` (`src/stale.rs`), which reads `Project::sibling_task_copies` (`git worktree list`). `show` does not call it. Record-home spec §6.1 keeps reads local, so a warning, not a redirect, is the allowed change; `show`'s `warnings` array already exists, so the JSON shape is unchanged.
- Scope skill §6 (`skills/scope/SKILL.md`) compares claim, status and updated, and has no `stale_copy` rule.

Done:
- `show` calls `stale::compare` for the loaded copy and, when a sibling is newer, warns `<id> is newer in <worktree> (<status> there); this copy is behind`. A same-second fork gets its own wording. A failure to list worktrees surfaces as the warning `compare` already returns.
- Scope §6: a member whose `show` carries that warning, or whose first write fails with `stale_copy`, is skipped as "changed in another checkout", naming the checkout. The printed retry is never run.
- Integration test in `tests/cli.rs` from the brief's reproduction: with the worktree present, `show` from main warns and `note --reviewed` still refuses.
- Left as recorded gaps (brief Q2, Q3): a branch with no worktree (record-home §8 accepts this class) and the retry wording on a status mismatch.

Where: `src/commands/show.rs`, `src/stale.rs`, `skills/scope/SKILL.md` §6, `docs/specs/2026-09-30-record-home-design.md` §3 and §6.1.

## Notes

- 2026-10-17T02:28:10Z (main): scope: briefed; reproduced in a scratch project: with the promoting worktree present, main's queue and show look clean and only the write refuses stale_copy (its retry points into the branch); with the worktree removed and branch kept, the note writes silently. Lean: show warns on a newer sibling copy + scope §6 skips on it or on stale_copy; tasks-16de1c and tasks-2b2b49 proposed as duplicates; brief: docs/notes/2026-10-16-scope-queue-other-branches-brief.md
- 2026-10-17T18:10:42Z (main): scope: scoped; stranded briefed idea with no wait; premise unchanged at 8e1ff08 (show never calls stale::compare; scope §6 has no stale_copy rule); took the brief's lean 2+1 (show warns on a newer sibling copy, scope §6 skips on it or on stale_copy), rejecting queue-side filtering; no-worktree branch and retry wording stay recorded gaps; direct, s/mid
