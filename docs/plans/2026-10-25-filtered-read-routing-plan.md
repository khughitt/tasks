# Filtered Read Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Route ID-valued read filters like positional IDs while preserving explicit scopes, local worktrees, literal sources and typed failures.

**Architecture:** A shared filtered-read opener checks canonical filter-prefix agreement, then delegates to `open_id_read_ctx`. The existing positional resolver remains responsible for checkout discovery and destination selection. Shared read completion uses the existing `local_or_foreign` helper.

**Tech Stack:** Existing Rust CLI, clap, registry and integration fixtures; no new dependencies.

**Spec:** `docs/specs/2026-10-25-filtered-read-routing-design.md` at `a92a91accc1a698d5606f8ded28bbe51a8e01cdd`. Keith approved `f03f0b4` for planning; `a92a91a` applies the two accepted wording nits and records that approval without changing policy.

## Global Constraints

- "It adds no command, flag, dependency, stored field or JSON shape."
- "It does not change mutation routing."
- "ID filters narrow that scope; they never enlarge or replace it."
- "A bare suffix outside every project is `invalid_id`, including with explicit scope: explicit scope does not define shorthand's local prefix."
- "`--source` does not select a project, even when it looks exactly like a full task ID or contains an unregistered prefix."
- Preserve malformed-local-config, stale-prefix, Git-marker, registry and record-pairing errors; no discovery-error suppression.
- Run tests through `just test-one`, `just test-fast` and the documented baseline `just test`; never call the runner directly.
- Build fixture child environments key by key with temporary HOME and isolated Git configuration. Existing `TestEnv` and `common::git_fixture` provide this. Do not use `common::git`, which currently inherits its caller's environment; new Git-backed cases belong with the isolated helpers in `tests/linked.rs`.

## Review Focus

1. Both local config and registry corrupt: an ID-filter call reports the local config error first, matching positional IDs (Task 1, lookup-order test).
2. Old and live spellings of the local prefix: canonical agreement must retain the local worktree (Task 1, alias and in-tree-worktree tests).
3. A local bare suffix combined with a foreign full ID: shorthand must not change meaning after routing (Task 1, ID-normalization test).
4. Explicit scope with a corrupt cwd config: full-ID filtering must still skip cwd discovery; explicit suffixes still use the original cwd (Task 1, explicit-scope test).
5. Parked tasks and worktree-only descendants: route before scanning so records and park rows come from the intended checkout (Task 1, routing and worktree tests; Task 2 checks matching completion).

## Files and responsibilities

| File | Change |
| --- | --- |
| `src/commands/mod.rs` | One filtered-read opener and four dispatch call sites. |
| `src/error.rs` | A typed no-project error carrying next-step text, retaining `no_project`. |
| `src/cli.rs` | Explain parent/under routing in existing flag help, including next. |
| `src/complete.rs` | Reuse local-or-foreign candidates in shared scoped completion. |
| `tests/cli.rs` | Routing, normalization, explicit scopes, diagnostics and completion cases. |
| `tests/linked.rs` | Actual in-tree worktree and linked-record routing with isolated Git fixtures. |
| `README.md`, `skills/tasks/SKILL.md` | Current routing contract, conflicts, explicit source-search example and completion behavior. |

Keep the current modules and fixtures; no file split, new fixture framework or parser-surface change. Leave `tools/cli.toml` alone.

## Execution Setup

- [x] Obtain user approval of this frozen plan, then resume the existing `.worktrees/tasks-284d9b` on `design/tasks-284d9b`. Read parent/child tasks and both documents; start the implementation child before editing product code and run `trial-arm` as instructed. Use the worktree skill to verify existing isolation, not create another tree.
- [x] Recheck tasks-e147b9 from its authoritative record. Its last inspected state was an open investigation, with no reproduced finding. If a new finding changes this spec's discovery assumptions, note the conflict and revise the design before implementation; do not reproduce or patch its attachment incident here.
- [x] Verify hydration remains present under `target/hydrated`; if missing, use the README's confined route: `CARGO_TARGET_DIR="$PWD/target" cargo install --locked --path . --root "$PWD/target/hydrated"`. Keep the host launcher unchanged during worktree checks.
- [x] Run `just test` before any product/test edits. Record command, nonzero executed test counts, failures and ignored counts on the task. Resolve a failed baseline before attributing failures to this change. This is the planned-work baseline, deferred during design.

## Lookup order fixed by this plan

Within the new opener:

1. Explicit scope delegates immediately to `open_id_read_ctx`; no extra registry load, filter parsing or local lookup precedes it.
2. With neither ID filter, delegate directly with None; preserve current discovery-before-registry order and decorate only a resulting `NoProject`.
3. With ID filters and no explicit scope, locate the current project first, propagating any error other than `NoProject`. Then load the registry, parse/canonicalize every provided filter using shorthand anchored to the original cwd, and compare canonical prefixes.
4. After agreement, share the existing positional resolver's local matching checkout or foreign registered-project selection. A small extraction may carry the already discovered context so discovery is not repeated.

Thus dual corruption with full-ID filters reports the local config path first, exactly as positional IDs do. With no ID filter, an outside cwd and corrupt registry still yield `no_project` with scope guidance. Explicit calls retain existing error precedence. Syntax errors are detected during step 3, before destination opening. These order changes adopt the accepted plan review's Minor 1.

### Task 1: Route filtered reads through the shared ID resolver

**Files:** Modify `src/commands/mod.rs`, `src/error.rs`, `src/cli.rs`, `README.md`, `skills/tasks/SKILL.md`; test `tests/cli.rs` and `tests/linked.rs`.

**Interfaces:**
- Consumes `parse_id(&Registry, &Shorthand, &str) -> Result<TaskId>`, `open_id_read_ctx(Option<&Path>, &ScopeArgs, Option<&str>) -> Result<ReadCtx>`, existing `TestEnv`, `list_ids`, `error_of`, `alias_registry`, and isolated linked fixtures.
- Produces `fn open_filtered_read_ctx(dir: Option<&Path>, scope: &ScopeArgs, parent: Option<&str>, under: Option<&str>) -> Result<ReadCtx>` in `src/commands/mod.rs`.
- Adds `Error::NoProjectNext { dir: PathBuf, next: String }`, displayed as `no tasks/.config.toml found above {dir}; {next}`. `kind()` returns `no_project`; `with_suffix` appends to next. Existing `NoProject(PathBuf)` and its discovery consumers remain unchanged.

- [x] **Step 1: Write routing and explicit-scope regression tests.** Add `filtered_read_routes_by_full_id` and `filtered_read_conflicts_and_explicit_scopes` in `tests/cli.rs`. Create one goal and one todo leaf per project (`sci`, `fam`); run from a plain outside directory and from the other project. Search uses a query matching the leaf. For each parent/under flag on list/search/ready and under on next, assert the leaf ID, with no unrelated local task. Park the leaf through `as_agent` and assert the routed `--parked` list contains it.

Core assertions (fixture names are illustrative):

```rust
assert_eq!(list_ids(&env.json(outside, &["list", "--parent", &fam_goal])), [fam_leaf.clone()]);
assert_eq!(env.json(&sci, &["next", "--under", &fam_goal])["next"]["task"]["id"], fam_leaf);
assert_eq!(env.fail(&sci, &["list", "--parent", &sci_goal, "--under", &fam_goal]), "validation");
assert_eq!(list_ids(&env.json(outside, &["list", "--all-projects", "--parent", &sci_goal, "--under", &fam_goal])), Vec::<String>::new());
```

Also assert conflict details name both flags and canonical prefixes, inside and outside. Test same-project combined predicates, explicit project including/excluding a target, groups including/excluding it, all-projects membership, and unknown explicit project (`config`). With both cross-project targets in a wide scope the result is empty; a missing target stays `task_not_found`. With corrupt local config, full-ID `--project fam` still succeeds.

- [x] **Step 2: Add normalization, source and diagnostic tests.** Name these `filtered_read_normalizes_ids`, `filtered_read_source_stays_literal`, and `filtered_read_lookup_order_and_errors`. Use existing fixtures/helpers; no inherited subprocess environments.

| Input/condition | Assertion |
| --- | --- |
| Retired and live same-prefix IDs, trailing periods/commas | Same canonical route and selected leaf; no false prefix conflict. |
| Outside suffix, with or without explicit scope | `invalid_id`, detail asks for full ID. |
| Local suffix plus foreign full ID, without explicit scope | `validation`; suffix expands to the original local prefix. |
| Local suffix with foreign `--project` | `task_not_found`; explicit destination does not rebind shorthand. |
| Unknown full prefix | `unresolvable_id`; missing known-project task is `task_not_found`. |
| ID-shaped, retired-prefix, unregistered-prefix and punctuated sources | Byte-for-byte equality under explicit scope or an ID filter; never canonicalize or resolve source. |
| Outside source-only or plain list, no ID | `no_project`; detail includes `--all-projects` and `--project <prefix>`; JSON error still contains only kind and detail. |
| Local malformed config, healthy registry, full-ID filter | `config`; detail identifies local `tasks/.config.toml`. |
| Registry and local config both malformed, full-ID filter | `config`; detail identifies the local config path, pinning positional-ID order. |
| Outside cwd, corrupt registry, no ID filter | `no_project` and guidance, preserving `local_read_prefers_no_project_to_a_malformed_registry`. |
| Registered retired target, inferred single-project read | Preserve the target's retirement warning; compare with the same filtered read in its root. |

For source equality, file matching tasks in both projects and verify `--all-projects --source` finds both; routing by an unrelated parent still filters the raw source string in the parent's project. Include a same-prefix alias pair with both filters supplied, so canonical agreement is exercised before route selection. Restore temporary registry contents before subsequent fixture operations. Keep the existing stale-prefix refusal control.

- [x] **Step 3: Add safe worktree/linked controls.** In `tests/linked.rs`, add `filtered_read_keeps_in_tree_worktree` using the module's isolated `git`/`code_repo`, and `filtered_read_routes_linked_records` using `layout`. For an in-tree project, commit a goal, branch a real worktree, then use `tasks add` with record auto-commits disabled to add a leaf only there. No Git writes occur in the linked worktree. Enroll both new cases in `linked_git_fixtures_ignore_hostile_environment`. Assert matching full ID, local suffix, alias, and `-C <worktree>` retain that leaf; explicit `--project` reads the registered main copy and returns no children. For the alias case, add an alias to this test's temporary registry with `toml::Value`, following `alias_registry`'s existing pattern; that CLI-test-private helper is not available here. Foreign full IDs route out of the worktree. For linked records, query parent/under from code, code worktree and outside; assert overlay records are read and code-tree status is unchanged. Extend `a_broken_ancestor_dot_git_is_named_for_inspection_never_removal` with a filtered-read case preserving its existing `io` diagnostic. Do not add the empty-marker attachment experiment owned by tasks-e147b9.

- [x] **Step 4: Prove RED.** Run `just test-one --test cli filtered_read_` and `just test-one --test linked filtered_read_`, then the existing marker test by name. New routing/guidance cases must fail on current code for their intended assertions; no compile errors or zero-test runs count. The existing boundary controls may already pass. Record what failed before implementing.

- [x] **Step 5: Implement the opener and typed diagnostic.** Use the exact interface and lookup order above. Parse all supplied ID filters before selecting one; compare only canonical prefixes, not whole IDs. On conflict return existing `Validation` mentioning both flags/prefixes and advising one project's IDs or explicit scope. Delegate resolution and warning generation to `open_id_read_ctx`. At the opener's outer return, convert only `NoProject(dir)` to `NoProjectNext` with next text `choose --all-projects or --project <prefix> to set the read scope`. No error conversion belongs in `Project::locate`, `open_ctx` or positional routing.

Wire list/search/ready with their `FilterArgs.parent/under`; next supplies None and its under. Keep search query validation before opening context. Do not change command-specific scan/filter/claim logic or the flags' stored values.

- [x] **Step 6: Update current-behavior documentation and help.** Parent/under help, including next, says the ID selects its project unless explicit scope is set. README routing and the shipped tasks skill explain matching local/worktree precedence, canonical prefix conflicts, original-cwd suffix meaning and shared positional/filter routing. Include the exact recovery example `tasks list --all-projects --source ops-2fe949 --status todo`; source remains literal. Keep completion prose for Task 2.

- [x] **Step 7: Prove GREEN and commit.** Rerun both new-test commands and the marker control. Run `just test-one --test cli bare_suffixes_`, `just test-one --test cli project_scope_`, and `just test-one --test cli id_commands_route_from_outside_every_project`. Then `just test-fast`, `tasks check`, and `git diff --check`; all must exit 0 with nonzero test counts. The pre-commit hook supplies hygiene, formatting and Clippy. Commit the changed files and task record with `feat: route filtered reads by task id`.

- [x] **Step 8: Verify the installed worktree CLI and close the child.** Rebuild with the confined install command from Setup. Use `target/hydrated/bin/tasks` explicitly for read-only local list/search checks and, with `-C` naming a temporary directory outside every project, `list --under tasks-284d9b`, the original `list --source ops-2fe949 --status todo`, and its `--all-projects` recovery. Outside routing reads registered main, so compare descendants with a main-checkout read, not the branch-only children; the source-only call must fail with `no_project` and scope guidance. Use a disposable registry containing only the registered tasks root for the smoke when unrelated projects are held or broken. Remove the temporary cwd/registry afterward. Do not install or repoint a host launcher at the worktree. Record results and complete this child before Task 2; the parent remains open and owns the retained worktree.

### Task 2: Align shared filter and tree completion

**Files:** Modify `src/complete.rs`, `tests/cli.rs`, `README.md` and `skills/tasks/SKILL.md`.

**Interfaces:**
- Consumes Task 1's routing policy; no new call to its opener from completion.
- Keeps `pub fn scoped(current: &OsStr) -> Vec<CompletionCandidate>` and `fn local_or_foreign(registry: &Registry, base: Option<Project>, current: &str) -> Vec<Task>` unchanged in signature.
- Produces foreign-prefix candidates for unscoped filters/tree while retaining existing explicit scopes, local suffixes and silent failure behavior.

- [x] **Step 1: Write completion regressions.** Add `filtered_completion_routes_filters_and_tree` and `filtered_completion_keeps_local_and_explicit_scopes` in `tests/cli.rs`, using `TestEnv::complete`/`complete_values`. Table over list parent/under, search parent/under, ready parent/under, next under and positional tree. With a typed `fam-` fragment, assert `[foreign_id]` both from `sci` and outside. Empty outside fragment and outside bare-suffix fragments return no candidates. Empty local fragment and local suffix return only local IDs.

```rust
assert_eq!(env.complete(&sci, "bash", 3, &["tasks", "list", "--parent", "fam-"]), [foreign.as_str()]);
assert_eq!(env.complete(outside, "bash", 2, &["tasks", "tree", "fam-"]), [foreign.as_str()]);
assert!(env.complete(outside, "bash", 3, &["tasks", "next", "--under", ""]).is_empty());
```

Use the existing second-root fixture pattern to assert matching-prefix and `-C` completion retain the current checkout over the registered copy. Explicit `--project sci` with fragment `fam-` returns none; `--project fam`, a group containing fam and `--all-projects` offer the fam candidate. A group excluding fam returns none. Unknown/unreachable typed prefixes and malformed registries offer nothing without stderr. Update `completion_scopes_ids_to_what_each_argument_accepts`' old assertion that tree's foreign completion is empty: it must now offer the foreign ID. Leave destination/write completion assertions unchanged.

- [x] **Step 2: Prove RED.** Run `just test-one --test cli filtered_completion_` and `just test-one --test cli completion_scopes_ids_to_what_each_argument_accepts`. Expect the new foreign-candidate assertions to fail with empty candidates before the implementation change.

- [x] **Step 3: Implement shared completion.** In `complete::scoped`, keep all-projects/group and explicit-project arms as they are. In the unscoped arm, feed `open_local(&line)` and the fragment to `local_or_foreign`, then use existing `candidates` and `local_prefix`. Do not split tree from filters or introduce error-reporting on the completion path. Update the function comment and reference prose to describe typed foreign prefixes, local/worktree precedence, and no outside enumeration for empty fragments.

- [x] **Step 4: Prove GREEN and commit.** Rerun both focused commands, then `just test-one --test cli completion_` to cover unchanged explicit/destination completion behavior. Run `just test-fast`, `tasks check` and `git diff --check`; all must pass with nonzero test counts. Commit with `fix: align read completion with id routing` and complete the implementation child with recorded results.

## Final verification and integration

- [ ] Self-check spec coverage, then obtain the executing-plans skill's final whole-branch review, read-only against frozen commits. Persist its full returned report and record the review round on the parent before acting. Apply the repository's corrective-round and ruling rules; no unfinished check or reviewer may be left running at handoff.
- [ ] After the last product change, refresh `target/hydrated/bin/tasks` via confined install and run read-only smoke using that explicit path. Run further tests only for new changes, failures or unresolved concerns. With hooks installed, the pre-push hook owns the full suite after the Setup baseline; do not push as part of this plan.
- [ ] Complete the parent only after both children and final review are settled, and after accounting for retained files/worktree. Commit completion before integration. Integrate locally into main under the personal profile after checking main is clean and preserving others' work; do not push or open a PR. Follow the repository's reinstall instruction from merged main, then verify the installed CLI.
- [ ] Before worktree removal, run `tt-report`, inspect ignored entries individually, preserve review ledgers/reports through task attachments, and check no host pointer resolves into it. Follow unlock/removal and retained-item rules; unfinished work stays owned by an open task. Report actual tests/counts, accepted/deferred findings, integration and installation state.

## Plan self-review

Before freezing: check each spec section against these two tasks, test matrices,
lookup order, signatures and documentation destinations. Confirm no production
code was changed during planning and no accepted spec finding was silently
reopened. Recommend inline execution: the change has two small, sequential units
sharing one routing policy, with focused integration tests and a final independent
review. User approval of the plan is required before execution.
