Ready to merge: no Critical or Important findings. The frozen head `a92324edb974b190fb4e61ec97e139362b35d1e5` preserves production Git behavior and isolates unit-test Git execution without process-wide environment changes. One nonblocking test gap remains.

What this change does: It injects a Git runner into the shared commit procedure, including discovery, staging, HEAD checks, and cleanup. Tests select an environment cleared and rebuilt key by key with temporary HOME; production entrypoints retain the original runner and users’ configuration and hooks.

1. **Minor — inherited repository selectors lack regression coverage** — [.worktrees/tasks-f23cd5/src/record_commit.rs:642](.worktrees/tasks-f23cd5/src/record_commit.rs:642)
   **Cause:** The new child test supplies hostile global configuration and runs one successful commit. It supplies no hostile `GIT_DIR`, `GIT_WORK_TREE`, or `GIT_INDEX_FILE`, and never enters cleanup.
   **Consequence:** Switching discovery or cleanup back to the production runner could leave this regression green: pre-commit hooks do not run during discovery, index inspection, or reset. The current implementation routes these operations correctly.
   **Fix:** Extend the child regression with repository selectors pointing exclusively at owned temporary fixtures, and exercise a refused commit’s cleanup. Assert that the unrelated fixture’s HEAD and index stay unchanged.

Verification and scope:

- Independently ran `just test-one --bin tasks record_commit::tests`: **22 passed**, zero failures. `git diff --check` passed.
- Inspected every `commit_paths_in` caller, production `commit_paths` and `working_tree` entrypoint, and automatic/manual commit callers. Production command construction is unchanged.
- Existing tests cover local hooks, failed cleanup, unborn HEAD, linked worktrees, literal paths, and concurrent writers. The function pointer and stored runner are a small, sufficient change; no new dependency, CLI surface, or schema appears.
- Supplied logs corroborate **1,606 passing fast-suite tests** and completion of formatting and Clippy. The **1,609-test baseline** and RED result are controller-reported evidence.
- Reviewer identity: **codex/gpt-6.1-sol**, verified during this review turn.

**Declined to judge:** Pre-existing integration-helper environment isolation; changes to production handling of inherited Git selectors; hook implementations outside these fixtures. Those are outside this unit-test isolation fix.

**Ready to merge:** Yes, with the Minor finding deferred if desired.

Not checked: older Git versions or a fresh full-suite run. Residual risk is the regression-coverage gap above.
No files edited, Git state changed, or pushes made. Existing `check.log` and `test-fast.log` remain; no session processes remain running.
