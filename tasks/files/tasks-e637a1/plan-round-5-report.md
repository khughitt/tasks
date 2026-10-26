# Plan round 5 review and user approval

Reviewer identity supplied with the review: claude-code/claude-opus-5-5.
Frozen revision: tasks repository, 327fe22:docs/plans/2026-10-25-blocker-ranking.md.
The following is the full review and approval received from the user.

I'd accept revision 327fe22. All five round-3 findings are resolved, and I found nothing new. I reviewed the frozen revision (the worktree copy is identical to 327fe22) and checked what it depends on in the code and the task records. I wrote nothing. Approving the plan is your decision, and approving it also approves Ruling S1, which amends the approved spec's sequencing. After that, the task's owner records the round, unblocks the four step records and starts execution Setup.

For the review note: `review: plan round 5 — verdict: accept; findings: none; reviewer: claude-code/claude-opus-5-5`. That's round 5 because the owner's self-review was recorded as round 4.

## My round-3 findings in 327fe22

- **Red fast suite (Important 1):** fixed.
  - Every commit now requires `GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 just test-fast` to pass, and focused runs use the same prefix.
  - The ordinary run is logged once per task. The only allowed exception is the exact set of three move tests, failing for the known cause. Any other failure, a changed cause or a partial set means investigating.
  - The three named tests exist in `tests/move.rs` (lines 1721, 2332 and 2370). The design session's log shows exactly those three panicking at `tests/move.rs:1717`.
- **Unneeded wait on tasks-e8d471 (Important 2):** fixed by Ruling S1, using the drop-the-edge option I recommended.
  - tasks-5d0d99 now depends only on tasks-ceee0d.
  - The coordination note is on tasks-e8d471 in the main checkout (commit 4119b99, 2026-10-26T00:25:37Z).
  - The ruling states why it's safe and what it costs, and says it amends only the spec's sequencing, not any report behaviour.
- **Resolver test row (Minor):** fixed. The in-memory matrix no longer claims to test Resolver. A new disk-backed `dependency_resolver_preserves_local_and_foreign_authority` test in `tests/cli.rs` captures how Resolver behaves today, and has to pass both before and after the refactor.
- **`Coverage` input vs. output (Minor):** fixed.
  - The input carries mode and the requested, scanned and unreachable projects.
  - `build` fills in both target sets and the final `complete` value.
  - Callers can't supply a precomputed completeness result.
- **Untested spec outputs (Minor):** fixed. Two explicit assertions are added: the JSON warning that history metrics are unavailable, and the complete/partial heading, including at `-n 0`.

## Things to know before approving

- **Ruling S1 lives in the plan, not the spec.** The spec stays frozen at d2940d0 and still says tasks-e8d471 comes first. The plan says it supersedes that, and the brief records it. Anyone reading the spec alone will see the old sequencing until the spec is rewritten as a reference document.
- **The plan's self-review round wasn't independent.** The owner's report says so. This review is the independent check on the revision.

Nothing was pushed or merged. `.worktrees/tasks-e637a1` is unchanged, tasks-e637a1 is still parked waiting on your plan review, and its step records stay blocked until you approve.

Keith: approved; after marking the plan approved, let's park the task so that execution can begin in a fresh session.
