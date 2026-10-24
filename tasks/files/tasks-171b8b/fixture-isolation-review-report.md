What this change does: It isolates Git subprocesses in the linked-project and record-commit integration fixtures. It also checks repository boundaries after initialization and adds hostile-environment regressions that protect an owned foreign repository and fake global config.

Review complete. **Verdict: Ship.** The controller can record acceptance and proceed with closeout.

Reviewer: `codex/gpt-6.1-sol`  
Frozen range: `a0f1d8e..99ad63da279fd1830fd0023bca93758e1def161f`  
Findings: **Critical 0, Important 0, Minor 0.**

- `.worktrees/tasks-171b8b/tests/common/git_fixture.rs` clears the inherited environment and reuses the existing key-built environment, with temporary HOME, fixed identity, and disabled global/system Git configuration.
- Both module helpers check the fixture top level and contained Git directory immediately after initialization, before subsequent mutations. The direct Git calls also use the isolated constructor.
- The regressions spawn children with selectors targeting only owned temporary state. They require successful fixture operations and unchanged foreign HEAD, index bytes, and fake global config.
- Existing assertions remain intact. Reusing representative tests also covers linked worktrees and non-UTF-8 paths without duplicating their fixture logic. The raw-byte boundary check preserves those paths correctly.
- The change stays within the task’s scope. No production constructor, CLI contract, or wider fixture rewrite was introduced. I assessed it for the existing parallel local integration-test workload.

Verification: I reran `just test-one --test linked --test record_commit`: **65 passed, 0 failed, 0 ignored**. `git diff --check` passed. I inspected the recorded full baseline (**1664 passed**), fast suite (**1662 passed, 4 ignored**), check output, and both attached RED logs showing the old helpers changed the owned foreign HEAD.

Declined to judge:

- Production Git isolation: explicitly excluded; production code is unchanged.
- Git fixtures in other integration modules: explicitly excluded from this bounded fix.

No pushes, tracked-file changes, task-note changes, or branch changes were made by this review. The worktree remains clean; the session process check found nothing left running.

Not checked independently: the full suite and lint checks were inspected from the supplied logs rather than rerun. No additional in-scope risk identified.
