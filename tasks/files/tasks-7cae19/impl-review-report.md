What this change does: Both Git closures in `.worktrees/tasks-7cae19-git-trace/src/link.rs` remove every inherited `GIT_TRACE*` key. Unborn repositories can answer `holds_records: false` under tracing, while the empty-stderr corruption guard remains intact.

Reviewed frozen range `c83093be..2ddcc8e`, the task requirements, connected callers, Git command setup, and test helpers. Assumed the documented local CLI workload.

- Critical findings: none.
- Important findings: none.
- Minor findings: none.
- Declined-to-judge behaviors: none within scope.
- The regression covers four trace variables and traced corrupt loose refs. Existing coverage retains corrupt packed-ref errors.
- New fixture commands use allowlisted environments and isolated Git config. Both Git roots are asserted before subsequent fixture writes.
- No CLI, JSON, dependency, or interface changes.

Verification evidence reviewed: expected RED regression; GREEN focused regression and corruption test; full baseline 1663 passed; final fast suite 1660 passed, 4 ignored; precommit checks passed. My independent `git diff --check` passed.

Reviewer: `codex/gpt-6.1-sol`.

Verdict: Ship. Ready to merge.

Not checked: I did not rerun tests; the supplied verification covers this narrow change. No remaining risk identified.
No writes or pushes performed; worktree unchanged, and no processes left running.
