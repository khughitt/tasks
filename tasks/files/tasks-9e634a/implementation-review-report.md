What this change does: A flagless edit now returns an actionable `editor` error when stdin is not a terminal, before creating the edit copy or launching `$EDITOR`. Explicit `--editor` preserves scripted and GUI editor use.

Reviewer: `codex/gpt-6.1-sol` — verified by a fresh identity command.

Reviewed frozen range in `<tasks checkout>`:

- Base: `e230cbdd6854c3e4cb5b2646eedc541aa889f395`
- Head: `9dc9d1a2eef9c15a1c5bc10a9d39d3e3203e7b24`
- Requirements: `tasks/tasks-9e634a.md` at that head; direct process, no implementation plan.

The implementation is small and preserves the existing editor’s validation, concurrency checks, failed-edit retention, and save behavior. I checked all 48 field and modifier argument IDs against the new conflict list: none are missing or extra. Retry generation preserves `--editor`, and the updated retry assertion checks this. README and shipped skill describe the changed behavior accurately.

Critical: none.

Important: none.

Minor:

1. **An editor override masks existing conflict coverage** — `.worktrees/tasks-9e634a/tests/cli.rs:5964`

   The setter/clearer conflict loop now supplies `--editor` alongside each pair. All three cases therefore return usage error 2 because `--editor` conflicts with either field, even if the original `--spec`/`--no-spec`, `--plan`/`--no-plan`, or `--step`/`--no-step` conflict is accidentally removed.

   Remove `--editor` from this loop only. These are field edits and need no override. This is a minor test regression: the production setter/clearer declarations remain correct.

Verification: I inspected the frozen diff, connected dispatch and retry paths, editor safeguards, test migrations, parser declarations, and documentation. `git diff --check` passed. I relied on the controller’s reported baseline, focused tests, final fast suite, checks, and PTY smoke; I did not rerun them.

Declined to judge:

- Detecting terminal usability beyond stdin’s terminal status: the task explicitly chooses stdin detection.
- Whether an explicitly requested editor can itself hang: `--editor` deliberately permits that launch.
- Behavior of unrelated science commands in the adopted vocabulary: their implementation is outside this change.

Verdict: **Ship, with one minor test correction recommended.** No Critical or Important findings block integration.

Not checked: independent upstream byte comparison or runtime release rebuild; the controller owns those remaining checks. Review changed no files, records, index, HEAD, or branches and left no processes running.
