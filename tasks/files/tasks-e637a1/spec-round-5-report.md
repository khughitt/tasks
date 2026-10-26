# Spec round 5 review

Reviewer identity supplied with the review: claude-code/claude-opus-5-5.
Frozen revision: tasks repository, d2940d0:docs/specs/2026-10-25-blocker-ranking-design.md.
The following is the full review received from the user.

I'd accept revision d2940d0. All eight findings from my round-3 review are resolved, and the new claims it makes about the code hold up against the source. One minor fixture gap is left, and it doesn't block approval. Approving the design is still your decision. Once you approve, the task's owner records the round and drafts the implementation plan for your separate review. I wrote nothing.

For the review note: `review: spec round 5 — verdict: accept; findings: Minor 1; reviewer: claude-code/claude-opus-5-5`. That's round 5 because the other session already ran its own round 4.

## My round-3 findings in d2940d0

- **Paths through another project's task (Important 1):** fixed.
  - When an in-scope path passes through a task outside the scan, the spec now lists that task in `coverage.unfollowed_targets` and sets `complete` to false.
  - It also warns that in-scope impact and rows may be missing.
  - The bb-scope case is corrected, and there's a new A→F→Y fixture that shows the missing row.
  - It chose to label these cut paths partial rather than walk through the foreign task. That was one of the two fixes I offered, and the cost is stated.
- **Whether a blocker is itself blocked (Important 2):** fixed. Each row now carries `unmet_dependency_count` and `unresolved_dependency_count`, and neither affects ranking. The fixture shows L and R each at 1/0.
- **Waiting on tasks-e90b4f (Important 3):** fixed. The plan pulls the shared dependency check out itself and moves `ready`, the review view and `open_deps` onto it unchanged. tasks-e90b4f's public contract is no longer a prerequisite.
- **All five Minor findings:** fixed.
  - `review::Deps` and `open_deps` are now listed.
  - The cost of depending on a goal is stated, and the footer explains it.
  - `awaiting_user` is limited to reviewed ideas.
  - The row field is renamed `task_needs`, so it no longer collides with `park.needs`.
  - An optional `claim` field shows the holder; stale claims are omitted.
- **Your open question on shelved and idea dependents:** answered the way I recommended. Ranking still weighs every open dependent equally, and each row adds `direct_work_by_status` and `transitive_work_by_status`. Those maps cover all five open statuses (idea, todo, doing, blocked, shelved), which matches the `Status` enum.

## New claims checked against the code

- `open_deps` returns open and unreachable IDs together (`src/commands/mod.rs:1593`).
- `ClaimSnapshot::held` returns live and unverifiable claims and drops stale ones (`src/claims.rs:797`).
- `ClaimInfo` has an `unverified` field (`src/output.rs:596`).

## Remaining Minor

- **One boundary row is missing from the bb-scope case** (around spec line 344). It lists only B and L as boundary rows, but M depends on X, so X (aa-000006) is one too. X has no dependencies of its own, so it belongs in `boundary_targets` but not in `unfollowed_targets`. Listing it would also exercise the "a boundary target with no dependencies stays complete" rule inside that case.

## A point for the plan, not a finding

`open_deps` looks dependencies up through `Resolver`, against registered roots. `resolve_dependency` checks the scan it already holds first. So the shared function has to take the lookup source as a parameter, or these callers will change behaviour. The spec's "without changing their behaviour" already requires this; the plan should make that step explicit.

Nothing was pushed or merged. `.worktrees/tasks-e637a1` is unchanged, and the task is still parked waiting on your review.
