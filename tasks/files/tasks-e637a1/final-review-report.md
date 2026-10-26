# Final whole-branch review, round 1

Reviewer: fresh general-purpose subagent, claude-code/claude-opus-5-5 (self-reported, model pinned at dispatch).
Range: 882a2bc..b9f8123 (package review-882a2bc..b9f8123.diff). Verdict: With fixes (Important 2, Minor 5).

## Strengths
- Task 1 refactor preserves behaviour at every migrated site (list::resolve_dependency, known_closed, satisfied, open_deps, review::Deps, closeout/prime, parked, lanes): same canonicalize-then-follow and lookup authority; review::Deps keeps the terminal closed timestamp; compact dependency.rs with a matrix covering authority, recurring/dropped/shelved, move loops, alias hops and error propagation.
- blockers::build is pure, derives boundary/unfollowed/complete itself (callers cannot supply graph completeness), examines boundary rows' own dependencies, memoizes per canonical id, uses iterative Tarjan (no stack overflow), deterministic tie-breaks proven by reversed-iteration test.
- JSON contract pinned: exact top-level and row key sets, optional-field omission, five-key status maps, goal null/lane rule; defer in existing YYYY-MM-DD form.
- Read-only orchestration: one scan, claim stores for every ranked prefix loaded before truncation (malformed store fails at -n 0); byte snapshot of task dirs + HOME proves no writes.
- Fixture isolation correct: git_fixture::command + assert_repo before first write; GIT_CONFIG_GLOBAL=/dev/null, GIT_CONFIG_NOSYSTEM=1.
- Adopted tools/cli.toml byte-identical to ops f90085b:cli.toml; ops 32b2f2e adds exactly the planned row.
- Runtime spot checks in a temporary worktree: pretty at 120/90/60 columns renders user/host/needs/cycle/defer correctly; boundary with no deps keeps complete true; read-only `--all-projects -n 3` against the real registry: 35 projects, 460 rows, complete, 2.35 s debug (vs 9.98 s for ready --all-projects).

## Review Focus verdicts
1. Met (unit test asserts complete false + diagnostic; note any boundary with stored deps is already unfollowed, so the second-level lookup adds counts/diagnostic but never alone flips complete).
2. Code correct, test coverage partial (Important 2).
3. Met.
4. Partly: 120 fits, unverifiable labelled; 60 titles do not wrap (Ruling T3-R4); label lines not asserted (Important 1).
5. Met.

## Issues
### Critical
None.

### Important
1. tests/blockers.rs blockers_pretty_contract_and_existing_reads never asserts the attention/resource/cycle detail labels. The plan requires "all attention/claim/resource labels"; the spec requires distinct pretty labels for task needs vs park host requirement, park reason/next step under user-attention rows, and "investigate cycle" under cyclic rows. In that fixture the user-held task is a consumer, not a ranked row, so user:/park:/host:/needs:/cycle:/defer: are never rendered. Behaviour verified correct at runtime; regression guard missing. Fix: make a ranked row a user/approval park with --needs headless and a task need, add a hand-seeded cycle and a deferred blocker, assert each label line.
2. Review Focus 2 only partly tested (src/blockers.rs blockers_identity_and_lifecycle alias case asserts only direct_work_count and relief). Untested: own unmet_dependency_count dedup when a row names the same open target under two spellings; two spellings of one missing reference -> unresolved count 1 and one diagnostic; status maps and direct_examples counting an alias+move dependent once. A probe confirmed the code is correct (own 1/1, one diagnostic, idea counted once, examples deduped). Fix: add those assertions.

### Minor
1. blockers --help does not mention unavailable history metrics (src/cli.rs:894); spec says "stated in warnings and help". Add long_about after checking parser_surface_equals_table compares only the summary.
2. Unresolved diagnostics keep only the first spelling per canonical id per dependent (src/blockers.rs:162-166); spec says "retain original reference IDs". Counting by canonical is right; diagnostics could list every stored spelling.
3. Lookup policy duplicated (src/commands/blockers.rs:46-51 vs list::resolve_dependency; Ruling T3-R2); both linear find per unique id. A shared list::scan_lookup(ctx, all), ideally map-backed, would keep them in step.
4. Test helper duplication: isolated_worktree in tests/cli.rs and tests/blockers.rs; write_claim/own_pid_view copy tests/cli.rs. Consider tests/common.
5. Weak footer assertion: "unknown" is satisfied by "attention unknown"; assert "history: blocking since".

## Executor rulings
Agrees with all eleven. T1-R1 cost partly retired by Task 3's worktree case; T2-R4 agreed; T3-R2 accept (see Minor 3); T3-R4 accept as an explicit departure from "wrapped titles at narrow widths", consistent with shared table behaviour.

## Declined to judge
- ops-b36208 local merge into ops main (outside range/repo).
- Unrelated obs cleanup row carried by adoption (ops content; byte-identical adoption).
- Goal misclassification via cross-project parent links under narrow scopes (spec's structural-only limitation).
- Records stored under an aliased prefix missing the scan-first lookup (same as existing resolve_dependency).
- trans/direct columns visually adjacent without --headers at 90 columns (existing table convention).
- Always-on history warning duplicating the footer (spec-mandated).
- Performance at larger corpora (spec: profile first; measured run fine).

## Recommendations
- One test-only corrective round for both Important items; no product code change needed.
- Spec note: the permanent history warning adds a warnings block to every pretty run; reconsider when tasks-71c74d lands.

## Process disclosure (reviewer)
Ran `git stash` in its temporary worktree to revert a probe; that wrote one entry to the shared refs/stash, which it verified held only its probe and dropped (stash list empty again; executor re-verified: 0 entries). Temporary worktree, its target dir and scratch fixtures removed (executor verified /tmp/review-b9f8123 absent and no review worktree registered).
