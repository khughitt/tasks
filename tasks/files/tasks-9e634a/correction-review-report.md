What this change does: Commit `2400a8085ab8b38c46b5a90b9e5b6c428c1d1167` removes `--editor` from the setter/clearer conflict test in `.worktrees/tasks-9e634a/tests/cli.rs:5964`. Each case now exercises its intended conflict without the editor flag masking it.

Looks good. The connected parser independently declares all three conflicts: `spec`/`no_spec`, `plan`/`no_plan`, and `step`/`no_step`. All arguments are valid otherwise. Separate tests retain explicit-editor conflict coverage. No production code changes.

- Findings: Critical 0, Important 0, Minor 0.
- Verification: inspected the frozen diff, surrounding test, parser declarations, and existing editor-conflict test. Read the correction log confirming 1 focused test passed; controller reports 1,665 fast tests passed, 4 ignored, and `just check` passed.
- Reviewer identity: `codex/gpt-6.1-sol`, verified by the fresh identity command.
- Declined behavior: none. No files, HEAD, index, or task notes changed.

Verdict: Ship.

Not checked: I did not rerun tests or repeat the full-branch review. No additional risk identified within this correction’s scope.
