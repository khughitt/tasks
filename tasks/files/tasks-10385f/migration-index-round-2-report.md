What this change does: Frozen corrective commit `6e1492a9a6fca5bada10006f22fbabab59d09de7` makes the migration guard inspect the code repository despite inherited Git repository selectors. It preserves only explicit `GIT_INDEX_FILE`, so a hook’s alternate index still determines whether the config is being removed. Review complete; the controller can proceed with integration.

- **Critical:** None.
- **Important:** None.
- **Minor:** None.
- **Correctness:** `.worktrees/tasks-10385f/src/link.rs:409` now uses `git::at(top)`, clearing `GIT_DIR`, `GIT_COMMON_DIR`, `GIT_WORK_TREE` and `GIT_INDEX_FILE`, then restores only the explicit index. Git failures still produce an error rather than reading as an absent config.
- **Callers:** Both `check` and `init --records` pass the discovered code-repository root. The correction preserves their index-held-config refusal and staged-deletion acceptance.
- **Tests:** The new regression places a tracked config in a foreign repository and removes the explicit index override. Its recorded RED failure is exactly the false `records_moved` error; the corrected test passes. The alternate-index regression verifies both refusal and acceptance.
- **Minimality:** The correction reuses the existing constructor, removes duplicated command setup and adds one focused regression. No dependency, interface or output change; negligible added cost for the local CLI workload.

Independent validation: four focused tests passed—new foreign-selector regression, alternate-index guard, ordinary `check` migration guard and `init --records` migration guard. `git diff-tree --check` passed. I inspected the recorded RED, 46-test detached GREEN and 1,659-pass fast-suite logs. The checkout differs from the frozen corrective revision only in its task record; reviewed source and tests match.

Behaviors declined to judge: Git overrides beyond the four named selectors are outside this task. This scoped review does not repeat the original implementation’s whole-branch review.

**Verdict: Ship.**

Reviewer identity: `codex/gpt-6.1-sol`, verified by a fresh command with exit 0. No tracked files, index, HEAD, branches or task records changed; no push; worktree clean. `host-load` reported nothing left running.

Not checked: I did not rerun the full suite or lint. No additional in-scope risk found.
