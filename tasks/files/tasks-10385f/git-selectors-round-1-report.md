What this change does: Frozen revision `92b1c96afffef481a635e25bb06c3a5a7e6e3d77` makes path-targeted Git operations ignore inherited repository and index selectors. It preserves the calling hook’s index for the migration guard. Review complete; the controller can proceed with integration.

- **Strengths:** `.worktrees/tasks-10385f/src/git.rs:5` provides one small constructor that clears all four selectors. Every production path-targeted Git call uses it.
- **Strengths:** `.worktrees/tasks-10385f/src/link.rs:409` retains the deliberate migration exception. The alternate-index test proves both refusal before removal and acceptance afterward.
- **Strengths:** `.worktrees/tasks-10385f/tests/detached.rs:89` verifies the records commit and unchanged foreign code HEAD/index. Other regressions cover discovery, dirty records and sibling stale-copy protection. New subprocesses use the isolated test environment.
- **Scope and minimality:** No dependency, CLI, JSON or schema change. Existing commit locking, cleanup and error handling remain intact. The change adds negligible work for the expected local CLI workload.
- **Critical:** None.
- **Important:** None.
- **Minor:** None.

Fresh verification: four hook tests passed; the stale-copy regression passed separately; `git diff --check` passed. I also inspected the recorded failing regressions and final suite logs.

Behaviors considered but declined to judge:

- Git overrides beyond the four named selectors, including object-store and configuration overrides: broader environment hardening is outside this task.
- Environment inheritance in older linked and record-commit fixture helpers: unchanged by this implementation; the task already identifies separate record-commit fixture work as `tasks-79a907`.

**Verdict: Ship.**

Reviewer identity: `codex/gpt-6.1-sol` (fresh command, exit 0). No tracked files changed, no push, worktree clean, and `host-load` reported nothing left running.

Not checked: I did not rerun the full suite or lint; five focused tests were independently rerun. No additional in-scope risk found.
