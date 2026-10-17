# Brief: the scope queue and records promoted on another branch

Scope pass, 2026-10-16. Ideas: tasks-f3910b (kept), tasks-16de1c and tasks-2b2b49
(proposed drops as duplicates of tasks-f3910b). All three came from material sessions.

## Problem

A scope pass on the main checkout can pick an idea that an unmerged branch has already
promoted, for example to a todo goal with children. On main the idea looks unreviewed, and
the pass spends its evidence reads on it. Its verdict then either fails at the write or,
worse, lands on main and conflicts with the branch's record at merge. The person running the
pass wants the queue and revalidation to say that this idea is being handled elsewhere,
before any work is spent on it.

## Current behaviour and evidence

Reproduced in a disposable in-tree project (isolated `XDG_CONFIG_HOME`/`XDG_STATE_HOME`):
an idea committed on main, a worktree on branch `feat`, and `edit --status todo` committed
there.

- **Worktree present.** From main, `list --needs-review` lists the idea with `*`. `show`
  reports `status: idea`, no claim and `warnings: []`, so §6 revalidation passes. Only the
  write fails: `note --reviewed` exits 1 with `stale_copy` ("… in <wt> is newer than this
  copy …; nothing was written. Run it there: tasks -C <wt> note …"). Following that printed
  retry would put a scope note on the branch's todo, which is wrong for a scope pass.
- **Worktree removed, branch kept unmerged.** The same `note --reviewed` exits 0 and
  writes to main's copy. Nothing compares it with the branch's copy.

Code: the write-side check is `refuse_stale_copy` (`src/commands/mod.rs`, `src/stale.rs`),
built on `Project::sibling_task_copies` (`src/repo.rs`), which reads
`git worktree list`. Reads do not call it.

## Constraints

- Record-home design (`docs/specs/2026-09-30-record-home-design.md`): §6.1 says a local
  record is always read locally, "even when another checkout's copy is newer", because the
  next write refuses under §3. §6.2 keeps plain `list` local (JSON contract). §8 accepts
  worktree-only tasks with no claim or park as invisible elsewhere.
- JSON output is the contract. `show` already carries a `warnings` array, so a new warning
  is not a shape change. A row field on `list` would be.
- Scope §6 (`skills/scope/SKILL.md`) compares `claim.live`, `status` and `updated` and says
  to propagate CLI race failures. It has no rule for `stale_copy`.
- A branch with no worktree is outside every check today (`sibling_task_copies` lists only
  worktrees).

## Alternatives

1. **Skill rule only.** Scope §6 treats `stale_copy` on a member's first write as a skip
   ("changed in another checkout"), names the checkout, and never runs the printed retry.
   This is cheap and needs no CLI change. The evidence reads are still spent, and the
   no-worktree branch case is still missed.
2. **`show` warns on a newer sibling copy, and the skill skips on that warning.** `show`
   calls the existing sibling lookup and adds `"<id> is newer in <worktree> (<status>
   there); this copy is behind"` to `warnings`. Scope's first show and its revalidation
   both see it, so the member is dropped from the batch before any reads. Reading stays
   local, as §6.1 requires, and there is no JSON shape change. The no-worktree branch case
   is still missed.
3. **The queue itself filters or flags.** `list --needs-review` runs the sibling lookup
   once per pass and leaves out, or warns about, ideas whose newer sibling copy is no
   longer an unreviewed idea. This is the most direct fix for the reports. It changes what
   the queue means, and it adds a per-row sibling read to a listing.

Lean: 2 plus 1. The warning is the earliest signal a pass already reads, and it costs
nothing in the contract. The skill rule covers a write that races the warning. The branch
without a worktree stays a recorded gap, as §8 already does for worktree-only tasks, unless
the answer to Q2 shows it is the common case.

## Unanswered questions

1. Is 2 + 1 the right scope, or should the queue itself filter (alternative 3)? — the user.
2. In the material incidents, did the promoting branch still have a worktree, or only the
   branch? This decides whether the no-worktree case is worth covering (for example by
   reading `git for-each-ref refs/heads` copies). — material's session logs, or the user.
3. Should a `stale_copy` detail drop its "Run it there" retry when the sibling's status
   differs, since the retry is then rarely the writer's intent? — design call, with the
   record-home §3.2 rules.

## Proposed decomposition

No follow-up is filed yet. Once Q1 is answered, tasks-f3910b becomes a direct task:
scope it to the chosen alternative, with the reproduction above as its integration test.
That means `show` from main reports the warning with the worktree present, and the scope
skill's §6 names the skip. tasks-16de1c and tasks-2b2b49 wait on nothing, because they
are proposed drops.

Resolved 2026-10-17 (scope pass): Q1 taken as the lean, alternatives 2 + 1, and
tasks-f3910b is scoped to it as a direct task. Q2 and Q3 stay recorded gaps in its body;
the user may override before it starts.
