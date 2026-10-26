# Ranked Downstream Blockers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship an explainable `tasks blockers` read ranked by observed downstream
impact, with honest coverage and no changes to ready/next policy.

**Architecture:** Extract the existing dependency rule with a caller-supplied
lookup, build a report from one captured scope scan, then add CLI serialization,
pretty output and guidance. Reuse existing scope, registry, tombstone, claim, table
and color facilities. Keep analysis independent of host-local attention evidence.

**Tech Stack:** Existing Rust crate, clap, serde, standard collections and the
existing table renderer; no new dependencies.

**Spec:** [Ranked downstream blockers](../specs/2026-10-25-blocker-ranking-design.md),
approved frozen revision d2940d0 in repository tasks. User supplied round-5
acceptance and delegated proceeding to planning; the controller accepted it.
The spec remains frozen. Its nonblocking bb-scope fixture omission is corrected
here: boundary targets are B, L **and X**; only L is unfollowed.

## Global Constraints

- Default scope is local; wide scopes use registered checkouts. Existing records,
  priorities, claims, parks, needs, picker ordering and other JSON shapes stay intact.
- Ranking key: transitive work descending, direct work descending, declared priority
  ascending, creation time ascending, full canonical ID ascending. No urgency policy.
- Work counts include idea, todo, doing, blocked and shelved equally; goals are
  separate. No implicit parent/child/lane dependency edges. No history inference.
- Limit defaults to 20, accepts 0, applies after full analysis/ranking; collections
  and zero counts are explicit as defined by the spec. Diagnostics are not limited.
- Scoped paths stop at foreign targets; nonempty dependencies on an open boundary
  target mark coverage partial even if its own dependencies resolve closed.
- Dependency relief never means ready or authorized. Missing park/claim evidence
  means unknown; task_needs and park.needs retain distinct meanings.
- New CLI rows land in ops before adoption here; never edit tools/cli.toml by hand.
- Process remains planned. These draft children are blocked for review; approval
  releases the review gate, not implementation dependencies or permission gates.

## Review Focus

1. A missing dependency of a foreign ranked target must make coverage partial even
   when the target itself resolved; Task 2 tests this second-level lookup.
2. Alias and move spellings can converge on the same target; Task 2 tests that own
   counts, examples and status maps deduplicate with their respective identity rules.
3. Local/worktree and registered-root copies can disagree; Task 1 tests each lookup
   source and closed timestamp, Task 3 tests command scope authority end to end.
4. Long colored Unicode titles can obscure counts or imply an unverified holder is
   definitely live; Task 3 tests layout and uncertainty at width 120 and narrow width.
5. Host Git configuration or routing can reach real repositories from fixtures;
   Tasks 1 and 3 use the existing isolated fixture helpers and assert both boundaries.

## Setup, dependencies and release order

The worktree is `.worktrees/tasks-e637a1`, branch `design/tasks-e637a1`. It was
hydrated with the README's locked cargo install, confined to target/local-install
and target; no host pointer was changed. At execution Setup, re-enter the worktree
skill and run the deferred full baseline `just test` before any product-code edits.
Record the actual test count and exit status on tasks-e637a1. The known host-config
fixture failure is tracked as tasks-2518ad; a diagnostic run with
`GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 just test` isolates Git config
without changing hooks. Do not claim the normal environment is fixed. Any new test
must use an explicitly constructed subprocess environment from its first RED run.

The blocked implementation records are tasks-ceee0d (Task 1), tasks-5d0d99
(Task 2), tasks-a7db30 (Task 3), plus ops-b36208 for vocabulary. After plan approval,
the controller unblocks these four records; existing dependency edges remain.
Task 1 can proceed then. Task 2 also waits on tasks-e8d471
landing in the code base used for execution; a done record on an unmerged branch
does not satisfy that integration prerequisite. Reuse its captured incoming-scan
work through the stable `Scope::scan() -> Result<Vec<Task>>` seam; inspect any
landed index helper before using it, rather than guessing its future signature.
The graph engine still owns move-aware transitive analysis, not another registry
walker. tasks-e90b4f's public-contract work is not a prerequisite.

Task 3 waits for Task 2 and ops-b36208. That prerequisite is blocked for this plan's
review, with source `tasks-e637a1:blockers-vocabulary`.
Its concrete deliverable in ops is the following command row in cli.toml, with
a `tests/test_cli.py::CliTable` assertion for it:

```toml
[[cli.tasks.commands]]
path = ["blockers"]
summary = "Rank unmet prerequisites by observed downstream impact"
options = [
  { shared = "limit", value = "int", default = "20" },
  { shared = "project", role = "select", value = "string", repeatable = false },
  { shared = "all_projects", value = "none" },
  { names = ["--group"], value = "string" },
]
```

The ops owner follows that checkout's process/worktree/setup/baseline rules,
proves the vocabulary assertion RED then GREEN through
`just test-one tests.test_cli.CliTable.test_tasks_blockers_surface_is_declared`,
runs `just test-fast`, and lands the reviewed row locally before adoption here.
No host installation, push or PR is needed for that prerequisite. On this side,
adopt from the registered ops root with the repository-documented vendored command;
commit the adopted table only with Task 3's parser change. If unrelated pending
rows make parser_surface_equals_table fail, use the documented `adopt cli.toml
--at <landed-commit>` route selecting the suitable landed snapshot; never hand-copy.

Before each implementation task: `tasks start <step-id>`, trial-arm, focused RED
test, minimum change, focused GREEN, `just test-fast`, then `tasks check` and
conventional commit. Record the task's actual results. New Git fixture mutations
use `tests/common/git_fixture::{command, assert_repo}` and temporary HOME;
never call the inherited-environment `common::git` helper for a new fixture.

## File map and internal seams

- `src/dependency.rs` (new): move-aware resolution and one three-way verdict.
- `src/commands/list.rs`, `src/review.rs`, `src/commands/mod.rs`: caller policy
  adapters; preserve signatures, lookup authority and old observable behaviour.
- `src/blockers.rs` (new): pure graph construction/counts/rank/coverage/cycles.
- `src/commands/blockers.rs` (new): read orchestration and host-local annotations.
- `src/output.rs`: new serde payloads/Output variant, warnings and theme integration.
- `src/cli.rs`, `src/main.rs`, `src/commands/mod.rs`: parser, module and read dispatch.
- `src/blockers_text.rs` (new): report-specific table/detail rendering, using table.rs.
- `tests/blockers.rs` (new): isolated CLI fixtures using tests/common; source-unit
  tests in dependency.rs and blockers.rs cover the pure logic.
- `tools/cli.toml`: adopted bytes only. README.md and skills/tasks/SKILL.md describe
  the shipped read and its limits in Task 3. Existing show JSON is not redesigned.

### Task 1: Share dependency resolution and preserve caller policies

**Files:** Create src/dependency.rs; modify src/main.rs (module declaration),
src/commands/list.rs, src/commands/mod.rs, src/review.rs; test these modules and
tests/cli.rs only where existing integration assertions need extension.

**Interfaces:** Produce these crate-internal functions/types:

```rust
pub enum Verdict { Satisfied, Open, Unresolved }
pub fn resolve(
    registry: &Registry, id: &TaskId,
    lookup: impl FnMut(&TaskId) -> Result<Option<Task>>,
) -> Result<Option<Task>>;
pub fn verdict(task: Option<&Task>) -> Verdict;
```

resolve canonicalizes before **every** lookup and reuses tombstone::follow. It
returns the terminal record, preserving closed timestamps and identity. verdict
is the only satisfaction predicate; it applies the established closed/open rule.
No new public read or JSON field is introduced by this task.

- [ ] Add `dependency::tests::dependency_lookup_authority_and_verdicts` using
  in-memory record maps and this assertion matrix (Task constructors can reuse
  the module's existing fixture idiom):

```text
captured copy open / disk copy done: lookup(scan-first) => Open
same ID with lookup(Resolver authority) => Satisfied, closed timestamp retained
Resolver keeps its own-prefix project local; foreign prefixes use registered roots
done recurring, even due => Satisfied; restarted => Open
dropped => Satisfied; shelved => Open; absent/move loop => Unresolved
alias -> moved -> live => terminal canonical ID, chosen lookup used at every hop
lookup parse/I/O failure => same typed error, not Unresolved
```

- [ ] Run `just test-one dependency_lookup_authority_and_verdicts`; expect RED
  because the new shared module/API is absent. Add a small integration assertion
  to the existing `dependency_resolution_prefers_the_captured_snapshot` test and
  Resolver-backed completion/closeout tests to pin caller authority before migration.
- [ ] Implement the two helpers. Keep `list::resolve_dependency`'s return shape
  and supply a lookup that consults the captured scan first, then ReadCtx, at each
  move hop. `open_deps` supplies Resolver::resolve_task, preserving its project-local
  lookup for its own prefix and registered-root lookup for foreign prefixes,
  with original stored IDs/order in its returned open list. Do not force every
  lookup to registered roots or reuse a captured scan in the write-side caller.
- [ ] Migrate ready's known-closed mapping to verdict (Satisfied -> Some(true),
  Open -> Some(false), Unresolved -> None); review::Deps uses Satisfied with the
  terminal closed timestamp and maps Open/Unresolved to its existing Open state;
  open_deps includes everything except Satisfied. Keep existing warnings/errors
  and scan ownership. Do not alter show.resolved or review wake policy.
- [ ] Run the new test GREEN, `just test-one dependency_resolution`,
  `just test-one --test review`, and existing dependency/recurrence tests selected
  through test-one; then `just test-fast`, tasks check and the pre-commit check.
  Expect no existing JSON/eligibility/closeout changes. Commit
  `refactor: share dependency verdict with explicit lookup authority`.

### Task 2: Compute downstream impact and honest graph coverage

**Files:** Create src/blockers.rs, add its module in src/main.rs; new analysis-only
payload structs in src/output.rs where needed. Tests live in src/blockers.rs.
**Requires:** Task 1 and integrated tasks-e8d471.

**Interfaces:** Consume Task 1 and a captured scan. Produce:

```rust
pub fn build(
    all: &[Task], registry: &Registry, coverage: Coverage,
    resolve: impl FnMut(&TaskId) -> Result<Option<Task>>,
) -> Result<Analysis>;
```

Coverage's serialized fields are exactly the spec's mode, requested_projects,
scanned_projects, unreachable_projects, boundary_targets, unfollowed_targets,
complete. `Analysis` has `rows: Vec<Impact>`, coverage, unresolved diagnostics and
sorted cycle components. `Impact` holds the terminal Task, goal Option<bool>,
cycle, all seven spec counts (work/goal direct+transitive, relief, own open,
own unresolved), both five-status work maps and up to three direct examples.
It contains no park, claim or authorization verdict. Task 3 turns it into rows.
All output-facing IDs/collections use the spec's canonicalization and deterministic
order. Implementation may choose private index/SCC structs; no new dependency.

- [ ] Add `blockers::tests::blockers_diamond_counts_and_ranking` with assertions:

```text
B: direct work 4, transitive work 5, goals 1/1, relief 3, own open/? 0/0
R: direct/transitive work 1/1, relief 0, own open/? 1/0
X: direct/transitive work 1/1, relief 0, own open/? 0/0
L: direct/transitive work 1/1, relief 0, own open/? 1/0
ranked IDs: [B,R,X,L]; all work-map totals are todo in the base fixture
```

- [ ] Add `blockers_scope_cut_and_boundary_targets`. In bb scope, assert exact
  boundary set `[aa-000001, aa-000002, aa-000006]` (B,L,X), unfollowed `[L]`,
  complete false. This corrects round-5 Minor 1. In a variant with no cut target
  except X (which has no dependencies), complete stays true. For A->F->Y across
  aa/bb/aa, assert local/group F has impact 1, own 1/0, goal null, Y has no row,
  complete false and F named as unfollowed; all-projects Y has direct 1/transitive
  2. A foreign target with dependencies all closed is still conservatively partial.
- [ ] Add named table-driven cases `blockers_identity_and_lifecycle` and
  `blockers_cycles_status_and_unknowns`: closed/dropped/recurring-done stop paths;
  restart reopens; alias/move duplicates collapse; move loop/missing/unregistered
  targets are unresolved with no invented row; a missing target in a boundary
  row's own lookup makes complete false. Known-open and unresolved own counts
  remain separate; parse/I/O errors propagate. Dependency SCCs/self-loops terminate,
  exclude self, and sort components/IDs. Goal-only rows rank after work rows;
  goal organization creates no implicit child impact. A blocker affecting ten
  idea/shelved records ranks ahead of one affecting three todo/doing records,
  with exact five-key maps and sums. Reverse fixture/edge iteration to prove ties
  by counts, priority, creation time and full ID are deterministic.
- [ ] Run `just test-one blockers_`; expect RED while build is absent/incomplete.
- [ ] Implement one canonical memoized resolution view, unique reverse adjacency,
  visited-set reachability and cycle-component detection. Retain graph edges only
  from open in-scope records to resolved open targets. Fetch boundary rows' own
  dependencies for status/counts without importing their graph edges. Preserve
  original references in unresolved diagnostics. Deduplicate per the spec, derive
  exact counterfactual direct relief without recursively completing dependents,
  then sort/rank all rows. No limit, I/O loop or host evidence belongs in build.
- [ ] Run `just test-one blockers_` GREEN and `just test-fast`; check/commit
  `feat: compute ranked downstream blocker impact`. Record any landed incoming
  helper reused and verify there is one scope scan, not repeated scans per blocker.

### Task 3: Ship the scoped CLI report and user guidance

**Files:** Create src/commands/blockers.rs, src/blockers_text.rs and tests/blockers.rs;
modify src/cli.rs, src/main.rs, src/commands/mod.rs, src/output.rs, README.md,
skills/tasks/SKILL.md and adopted tools/cli.toml. Existing src/table.rs, style.rs,
claims.rs and ScopeArgs are consumed without a new interface layer.
**Requires:** Task 2 and landed ops vocabulary prerequisite.

**Interfaces:** `cli::BlockersArgs { scope: ScopeArgs, limit: usize }` with flattened
scope; `commands::blockers::run(ctx: ReadCtx, args: BlockersArgs) -> Result<Output>`;
`blockers_text::render(out: &BlockersOut, painter: &Painter, layout: &Layout) -> String`.
Register `Output::Blockers(Box<BlockersOut>)`. BlockersOut/BlockerRow serialize the
approved spec's complete field list, including task_needs, status maps, own counts,
optional ParkInfo/ClaimInfo and diagnostics; do not serialize the internal Task.

- [ ] Adopt the landed ops row through bin/vendored in this worktree before parser
  changes. Add `blockers_cli_surface_and_limits` in tests/blockers.rs: default20,
  n0 (empty rows/full totals and diagnostics), n above total, empty graph and
  invalid negative/noninteger limits. Assert no undocumented filters/sorts and
  existing scope conflict/error behaviour. Run `just test-one --test blockers`
  RED: parser rejects the absent command.
- [ ] Add `blockers_scopes_and_read_only` using TestEnv. Exercise local, project,
  group and all-projects; reachable/unreachable projects, unregistered targets,
  malformed config/task/store, and a real isolated worktree whose local target
  status differs from its registered copy. Use git_fixture::command/assert_repo
  before its seed write. Local reads use the captured worktree; explicit wide
  reads use registered roots. Snapshot owned record/claim/registry bytes before
  and after and assert no writes. Verify an out-of-scope record with no task
  children scan keeps goal null unless lane=true. Assert sorted boundary/unfollowed
  sets, diagnostics and partial-rank warning survive n0 truncation.
- [ ] Add `blockers_attention_claims_and_resources`: synthetic prose-only approval
  todo is unknown; explicit user/approval park yields user attention/reason/step,
  identical graph rank; declared task_needs alone remain unknown; absent host-local
  park is unknown; reviewed-idea awaiting_user yields user; ordinary todo writes
  cannot set it. Live/unverifiable claims render holder and uncertainty, stale
  claims omitted; task_needs and park.needs can coexist. No real foreign mutation.
- [ ] Add `blockers_pretty_contract_and_existing_reads`: long Unicode/color titles,
  headers on/off, color modes, width120 and narrow widths preserve complete IDs,
  counts, wrapped titles and all attention/claim/resource labels. Assert footer
  explains structural-only goals, no goal-child attribution, host-local evidence,
  unknown history and relief not ready. All spec fields appear in JSON with exact
  zero/omission rules, timestamps ignored only for snapshot comparison. Existing
  ready/next result order and JSON stay equal before/after calling blockers;
  relief is shown even for a shelved/user-held/claimed/deferred consumer without
  asserting it is ready. Run these tests RED through test-one.
- [ ] Wire the noun command, flattened scope and default limit20. Resolve scope
  through open_read_ctx; call Scope::scan once, capture observed_at, memoize target
  resolution with Task 1 and build Task 2's analysis. Load host-local snapshots for
  **every ranked prefix before truncation**, including boundary prefixes; malformed
  stores error instead of silently disappearing with n0. Annotate future defer,
  task needs, attention, observed park and held claim. Derive total/shown/omitted,
  truncate ranked rows only, and preserve all diagnostics/coverage. Standard typed
  error rendering and warning channels remain as in existing reads.
- [ ] Integrate output serialization, pretty renderer, project colors and exhaustive
  matches (pretty, warnings, needs_theme, shows_priority, main's read-command width
  handling). Reuse table columns/details; target120 columns, wrap titles, keep IDs
  and counts intact, put relief/goals/status breakdowns/attention on continuation
  lines as the spec defines. An unverifiable claim is explicitly uncertain.
- [ ] Update README and the tasks skill with the implemented command, default/limit,
  scope examples, full coverage caveats, interpretation and metadata recording
  requirement. Do not install guidance links or change global startup instructions.
- [ ] Run `just test-one --test blockers` GREEN and
  `just test-one parser_surface_equals_table`; run focused Task 1 regressions,
  then `just test-fast` and tasks check. Commit adopted table with parser/output/docs:
  `feat: report ranked downstream blockers`. Run the existing pre-push full gate
  if pushing is later authorized; otherwise run just test only if hooks are absent
  or affected behaviour lacks fast coverage. There is no push authorization here.

## Final verification, installation and integration

After all implementation steps, request a fresh whole-branch code review using
the repository's review skill. Save its full report and ledger in the plan
workspace, note the review before acting, and run corrective rounds as the user
rules require. Reopen any completed step before its fixes. No deferred Minor can
quietly become a merge gate without the required regrade ruling.

Rebuild CLI changes under target/local-install with the confined locked install,
and smoke-test that explicit binary with isolated fixture state. It must exercise
local/group/all-projects coverage, user-held evidence and limit0 through their
verdict, without shared launchers or live foreign records. Record real results;
this plan asserts no measured latency. A shared host install is a separate approved
rollout after integration, not an implicit consequence of approving this plan.

Complete step records on their branch before merge. Keep tasks-e637a1 open while
children or review gates remain; update the brief with shipped results only when
observed. Final integration/cleanup uses the user-owned repository policy, retains
needed raw logs/review ledgers, checks ignored files/host pointers and runs
tt-report before worktree removal. tasks-6166d6 still needs independent history
collection and reporting under tasks-71c74d; this plan does not close that goal.

Recommended execution is native/inline: three tightly connected steps share lookup,
graph and output interfaces, so one implementer avoids repeated handoffs. The plan
still requires independent whole-branch review. Plan approval is required before
execution; no implementation step has begun.
