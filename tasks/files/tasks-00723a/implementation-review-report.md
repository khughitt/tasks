What this change does: The correction preserves the project root reported by `tasks where` for nested in-tree projects. It retains the invoking code worktree only for unscoped detached or linked invocations, preserving detached document fallback without redirecting scoped passes into another project’s worktree.

Reviewer: **codex/gpt-6.1-sol**, freshly verified with exit status 0.

All three supplied frozen hashes matched before and after review.

- **Critical:** none.
- **Important:** none. Prior re-review Important1 is resolved.
- **Minor:** none.
- **Declined to judge:** none.

The instructions agree between `.worktrees/tasks-00723a/skills/scope/SKILL.md:44` and `.worktrees/tasks-00723a/skills/curate/SKILL.md:32`. I traced their path choices against the project resolver, document resolver, and existing nested, detached, and linked tests:

| Scenario | Assessment |
|---|---|
| Unscoped nested in-tree worktree | Preserves the nested `.project.root` for task commands and `.project.checkout` for evidence. |
| Unscoped ordinary in-tree worktree | Preserves that worktree’s project locations. |
| Detached worktree with a worktree-only document | Keeps the original worktree as command context, retaining code-document fallback. |
| Linked code worktree | Keeps code evidence in that worktree and documents in overlay records. |
| Records-directory invocation | Uses `.project.root` for commands and keeps records and code locations separate. |
| `--project B` from project A’s worktree | Resolves B’s registered root and obtains B’s locations there. |
| Curate `--group` or `--all-projects` | Resolves locations separately for each sampled project. |

**Verdict: Accept.** No corrective findings remain within this review’s scope.

Not checked: I ran no tests; the supplied passing test results remain controller evidence. I made no modifications or task notes. Residual limit: this was a scoped corrective review, not another whole-branch review.
