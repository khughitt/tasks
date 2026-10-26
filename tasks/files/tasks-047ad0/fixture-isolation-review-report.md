What this change does: It isolates the shared integration-test Git helper from inherited repository selectors and host configuration. It validates fixture ownership after initialization and before linked-worktree writes. Review of tasks-047ad0 is complete; the controller can record acceptance and proceed with closeout.

**Verdict: Ship. Findings: Critical 0, Important 0, Minor 0.**

Reviewer: `codex/gpt-6.1-sol`  
Frozen range: `f1d45f2..d082ffee729a780303f3a22bcc6c12128dc9a6e1`

- `.worktrees/tasks-047ad0/tests/common/mod.rs` reuses the isolated command constructor with a per-call temporary HOME. The child environment is built key by key, with fake identities and global/system Git configuration disabled.
- `.worktrees/tasks-047ad0/tests/common/git_fixture.rs` preserves raw path bytes and checks the intended top level plus ownership of both Git directories. Linked worktrees correctly validate against their owned main fixture.
- The hostile-parent regression exercises initialization, staging, commits, worktree creation, and commits through the linked worktree. It verifies distinct intended contents and unchanged foreign HEAD, index bytes, and fake global configuration.
- Existing behavioral assertions remain intact. Connected callers in `cli.rs` and `refs.rs` need no API changes; the existing linked-worktree writer receives the required ownership assertion.
- The diff stays within the bounded fixture-isolation scope. I assessed the existing parallel local integration-test workload.

Verification: I independently ran the four hostile-environment regressions, the existing linked-worktree writer test, and the affected refs test: **6 passed, 0 failed**. `git diff --check` passed. I inspected the recorded **1744-test full baseline**, expected RED failure from initialization misrouting, GREEN regression, **1741-test fast suite with 4 ignored**, and successful `just check`.

Declined to judge:

- Production Git isolation and CLI behavior: unchanged and explicitly excluded.
- Independent Git helpers elsewhere: outside this task and the prior review’s accepted boundary.

No pushes, tracked-file edits, task-note changes, or branch changes were made. The reviewed worktree remains clean at the frozen commit; no processes were left running. The controller retains closeout and integration ownership.

Not independently rerun: the full suite and lint checks; their supplied logs were inspected. No additional in-scope risk identified.
