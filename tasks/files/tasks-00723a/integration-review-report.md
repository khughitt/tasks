What this change does: The merge preserves the approved d113068 task skill byte for byte, with main’s exact 20-line routing block inserted after the Project routing link and before `## Project locations`. Scope and curate remain identical to d113068.

Critical: none. Important: none. Minor: none.

Verified in `.worktrees/tasks-00723a/`:

- All three skill hashes match the frozen inputs; staged and working bytes agree.
- Removing the inserted block restores the approved task skill exactly.
- The staged source and tests match both MERGE_HEAD and current main. Those two revisions have identical trees.
- No unresolved merge entries remain; `git diff --cached --check` passes.

Declined to judge: main’s routing implementation and the previously approved skill behavior, because both are outside this merge-integrity assignment. I did not run tests or independently verify the controller’s running combined suite.

Verdict: accept the conflict resolution. Integration remains subject to the controller’s combined-suite result.

Reviewer: codex/gpt-6.1-sol, freshly verified with exit status 0.

Skipped broader review and tests as instructed. Remaining risk: combined-suite verification is still owned by the controller.
