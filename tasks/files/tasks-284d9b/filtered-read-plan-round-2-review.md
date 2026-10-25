I recommend **accept**: the plan at 3a9752f is ready to execute, with three minors and nothing Critical or Important. Approving it is still your call. Once you do, the Codex session that parked tasks-284d9b runs it inline. This was a read-only review: I edited nothing and wrote no task note. I read all 157 lines of the plan.

**What I checked against the code at 3a9752f**
- **Spec coverage:** the plan covers every rule in the approved spec (a92a91a). The a92a91a diff only applies my two round-4 wording nits and the approval status line. The routing rules, conflict check, literal `--source`, the `no_project` guidance, shared completion, documentation and the test list each map to a step.
- **References resolve:** every helper, test and function the plan names exists. That includes `common::git_fixture`, which is the module `tests/common/git_fixture.rs` (my first search looked for a function of that name and missed it). It also includes the `["next"]["task"]["id"]` JSON path the plan assumes for `next`.
- **New error variant:** `NoProjectNext` follows the existing `TaskNotFoundNext` pattern, so the JSON envelope stays kind plus detail. Nothing after dispatch matches on `NoProject`; the only consumers are discovery code.
- **No ops change needed:** `tools/cli.toml` has no help text for options, so editing the `--parent`/`--under` help needs no ops change.
- **Completion:** `local_or_foreign` already falls back to local candidates when a typed prefix doesn't resolve, so "unknown prefix offers nothing" holds once the fragment is filtered.
- **Test filters:** the RED/GREEN test-name filters (`filtered_read_`, `filtered_completion_`) don't overlap.

**Minor**
1. **Lookup order diverges from positional IDs, and a test would lock that in.** The plan loads the registry before locating the local project whenever ID filters are present. So when both the local config and the registry are corrupt, a filtered read names the registry. `show <id>` in the same directory goes through `open_ctx`, finds the project first, and names the local config. Spec rule 2 says filters select the project "exactly as a positional ID does", and Review Focus 1 would write the divergence into a test. Both errors are `config`, so the cost is only which file you're told to fix first.
   - **Recommendation:** find the project first, propagating any error other than `NoProject`. Then load the registry, parse and compare the prefixes. The plan's "small extraction" allowance covers this, and the dual-corruption assertion flips to name the local config.
2. **The installed-binary smoke never exercises the new route.** Task 1 Step 8 runs only from inside the worktree, which is the local route that already worked. Add read-only runs of `target/hydrated/bin/tasks` with `-C "$(mktemp -d)"`, i.e. from outside every project:
   - `list --under <an id on main>`. From outside, this reads the registered main checkout, so children that exist only on the branch won't appear; expect that.
   - Your original `list --source ops-2fe949 --status todo`, expecting `no_project` with the new guidance.
   - The recovery form `list --all-projects --source ops-2fe949 --status todo`.
3. **Task 1 Step 3 leaves the worktree's Git safety check unspecified.** "Commit a goal, branch a real worktree, then add a leaf only there" doesn't say how the leaf is added. If it's `tasks add` in the worktree, there's no Git write, which is what the existing worktree fixtures do (around `tests/linked.rs:92`). If a commit in the worktree is needed, `git_fixture::assert_repo` won't serve: it requires the Git directory to sit under the given directory, and a linked worktree's lives under `<main>/.git/worktrees/<name>`. That case would need its own top-level and Git-directory assertion. Adding the new Git-backed test to `linked_git_fixtures_ignore_hostile_environment` would also prove its isolation, as tasks-171b8b did for the existing fixtures.

For the note writer, I believe this is plan round 1, since the task record holds only spec rounds:

`review: plan round 1 — verdict: accept; findings: Minor 3; reviewer: claude-code/claude-opus-5-5`

Nothing was pushed and I left no processes running. The plan stays in .worktrees/tasks-284d9b at 3a9752f on `design/tasks-284d9b`, which stays unmerged. After execution and the final review, the plan has the controller merge it locally into main, as the personal profile allows.

Keith: accepted.

Reviewer: claude-code/claude-opus-5-5 (relayed by user). Recorded as round 2 because plan self-review was already round 1.
