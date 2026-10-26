# Plan round 3 review

Reviewer identity supplied with the review: claude-code/claude-opus-5-5.
Frozen revision: tasks repository, cd80149:docs/plans/2026-10-25-blocker-ranking.md.
The following is the full review received from the user.

I'd send the plan back for revision. The structure, interfaces and test coverage match the approved spec, and every code and ops seam it names exists. But two gaps would stall or confuse execution. I reviewed the frozen revision cd80149 (the one the task's park names; the worktree copy is identical). I wrote nothing. Approval is still yours; after your ruling, the task's owner records the round and revises.

For the review note: `review: plan round 3 — verdict: revise; findings: Important 2, Minor 3; reviewer: claude-code/claude-opus-5-5`.

## What checks out

- **Code seams:** `open_read_ctx`, `Painter`, `table::Layout`, `needs_theme`/`shows_priority`, `ClaimSnapshot::held`, `ClaimInfo.unverified`, the `dependency_resolution_prefers_the_captured_snapshot` test and `tests/review.rs` all exist. New modules are declared in `main.rs`, since there is no `lib.rs`.
- **Test fixtures:** `tests/common/git_fixture::{command, assert_repo}` clears the environment and checks both repository boundaries. `TestEnv` also builds its environment key by key.
- **Ops side:**
  - The proposed `cli.toml` row matches the shape of the existing scoped `list` row (`shared = "project"`, `role = "select"`, `--group`, `shared = "limit"`).
  - `bin/vendored adopt --at <commit>` exists.
  - `ops tests/test_cli.py::CliTable` exists.
- **Task 1's mapping for `ready`:** Satisfied → `Some(true)`, Open → `Some(false)`, Unresolved → `None`. That is exactly what `resolve_dependency(..).map(|t| !t.status.is_open())` does today. The review view also keeps its Open/Unresolved → Open mapping.
- **Step records:** tasks-ceee0d, tasks-5d0d99, tasks-a7db30 and ops-b36208 exist as described, with the dependency edges the plan states.
- **Spec carry-overs:**
  - The fix for my round-5 Minor is in: X is now listed as a boundary target in the bb-scope case.
  - Park and claim evidence loads for every ranked prefix before truncation.
  - The read scans the scope once.

## Important

1. **Every per-task `just test-fast` will fail in the normal environment, and the plan gives no rule for judging it.**
   - tasks-2518ad is still an unscoped idea. The design session saw `just test-fast` exit 101 with 3 move tests failing; it only passed with Git config isolated.
   - The plan handles this only for the Setup baseline. Each of Tasks 1–3 says "then `just test-fast`" with no expected result, so the implementer meets a red suite three times. The likely outcomes are stopping, or "fixing" fixtures outside scope.
   - Fix: state the per-task gate. Run `GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 just test-fast` and require it green. Also record that the normal-environment run fails on exactly the known move tests, and treat any other failure as a real regression.

2. **Task 2 is blocked on tasks-e8d471 but uses nothing it produces.**
   - The plan says Task 2 reuses e8d471's work "through the stable `Scope::scan()` seam". But `Scope::scan` already exists (`src/scope.rs:241`, and `moving::inbound` uses it).
   - `build` takes a captured scan and builds its own reverse adjacency.
   - e8d471 itself only adds incoming dependents to `show` (todo, direct, unstarted).
   - So the edge tasks-5d0d99 → tasks-e8d471 delays Task 2 without connecting it to any interface. The spec does require that sequencing (spec lines 53–54 and the handoff), so dropping it is a spec-level ruling.
   - My recommendation: drop the hard edge. Record a ruling that the spec's "no second project walker" intent is met by `build` taking a captured scan. Leave a coordination note so e8d471 reuses the scan rather than adding a walker. The alternative is to keep the edge and name the concrete e8d471 artifact Task 2 must consume.

## Minor

- **One Task 1 matrix row can't run as specified.** "Resolver keeps its own-prefix project local; foreign prefixes use registered roots" is `Resolver` behaviour that needs a registry on disk, so the in-memory unit test can't exercise it. Move it to the Resolver-backed integration assertions the next step adds.
- **`build`'s `coverage: Coverage` parameter is ambiguous.** It should say the input carries mode, requested, scanned and unreachable projects, and that `build` fills in `boundary_targets`, `unfollowed_targets` and `complete`.
- **Two spec outputs have no named test:**
  - the JSON `warnings` entry saying history metrics are unavailable (spec "Initial output contract"), as opposed to the pretty footer;
  - the complete/partial label in the pretty heading.
  
  Add both to `blockers_pretty_contract_and_existing_reads`.

Nothing was pushed or merged. `.worktrees/tasks-e637a1` is unchanged. tasks-e637a1 is still parked waiting on your plan review, and its three step records plus ops-b36208 are still blocked.
