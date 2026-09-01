# Material and Prism Tasks migration — design

**Status:** approved on 2026-08-31, corrected after review, and implementation planned
on 2026-09-01; migration has not started.

## 1. Purpose and baseline

Extend the completed six-project Tasks migration to Material and Prism. Reuse its
auditing, task-synthesis, registry, review, and verification rules without reopening or
rewriting that historical migration.

This extension is forward-only:

- audit every tracked project document and correct evidence-backed drift;
- turn only real unfinished outcomes into tasks;
- preserve completed and abandoned history in Git and the documentation archive;
- create and mutate task records only through the `tasks` CLI;
- leave live compositor state, deployments, and unrelated worktrees untouched.

Non-goals:

- migrating `niri-experiments`, `niri-glass`, or dotfiles;
- moving historical documents merely to satisfy Tasks path conventions;
- backfilling closed tasks for completed work;
- rerunning historical visual, burn-in, DRM, or nested-Winit acceptance;
- adding CI enforcement before `tasks` has a pinned install source.

## 2. Glossary and project map

| Term | Meaning |
|------|---------|
| Material | The `niri-material` repository and its Material compositor work |
| Prism | The Prism shell repository that consumes Material's exposed interface |
| Stable checkout | The existing checkout and branch into which reviewed migration commits are fast-forwarded |
| Migration worktree | A fresh isolated worktree created from the stable branch for migration writes |
| Portfolio registry | A temporary Tasks registry containing exactly the eight migrated projects |
| Ledger | The per-project migration evidence record under `docs/plans/` |

| Order | Project | Checkout | Stable branch | Prefix | Tracked docs at approval |
|------:|---------|----------|---------------|--------|-------------------------:|
| 1 | Material | `~/d/niri-material` | `materials-26.04` | `material` | 111 |
| 2 | Prism | `~/d/prism` | `main` | `prism` | 11 |

The counts are an inventory snapshot, not continuing invariants. `material` is eight
characters and `prism` is five; both satisfy the Tasks prefix rule.

Material goes first because Prism consumes its interface. Only one repository is
migrated or reconciled at a time, and each integration gate must pass before work starts
on the next repository.

## 3. Verified starting state

At approval:

- Material's stable checkout is clean at
  `048814893b9bc4924927468ce1f539b7764d942b`, one commit ahead of
  `origin/materials-26.04`;
- its linked `debug/overview-drag-frost` worktree starts at the same commit and has
  uncommitted changes only in `src/render_helpers/effect_buffer.rs` and
  `src/render_helpers/material.rs`;
- Prism's stable checkout is clean at
  `d20111c2182adcbbb2bd3b76356d6e1557cb1e12` on `main`;
- neither repository has a Tasks project.

These are preflight guards, not permission to clean or alter either checkout. Create the
Material migration worktree from `materials-26.04` and preserve the active debug
worktree byte-for-byte. Stop if branch tips move, stable checkouts become dirty, the
debug worktree's dirty-file set changes unexpectedly, or any required worktree cannot be
preserved.

## 4. Repository-gated workflow

Each project follows the completed migration's three-integration-commit workflow:

```text
preflight and baseline gates
  -> exact document inventory and tiered audit
  -> evidence-backed drift corrections
  -> remaining-outcome synthesis
  -> Tasks initialization and task creation
  -> independent review and fixes
  -> initial fast-forward, stable gates, and canonical registration
  -> ledger finalization and independent review
  -> final fast-forward and cleanup
```

All writes occur in the fresh migration worktree. Existing stable and linked worktrees
are read-only until reviewed commits are fast-forwarded into the stable checkout. Do not
repurpose, clean, stash, or modify unrelated work.

Each repository adds a ledger:

- Material: `docs/plans/2026-08-31-material-tasks-migration.md`
- Prism: `docs/plans/2026-08-31-prism-tasks-migration.md`

The ledger records the inspected HEAD and worktrees, complete document classification,
drift findings and outward greps, candidate outcomes and dispositions, created task IDs,
dependency decisions, commands and results, review fixes, integrated commits, and
canonical registration evidence. It classifies itself and is finalized in the third
commit, or in the reconciliation commit when a deferred link is pending.

## 5. Tiered documentation audit

Use `git ls-files -z docs` as the exact tracked-document denominator, then include the
new ledger in its own classification coverage. Every document receives one
classification:

- **Authority/current:** verify semantics against code, tests, configuration, and current
  Git state.
- **Active delivery:** verify status, checkboxes, dependencies, ownership, and remaining
  work; deeply verify technical claims governing unfinished work.
- **Historical/superseded:** preserve historical rationale and snippets; correct only
  misleading status, broken authority links, or claims repeated by current user-facing
  documentation.

Evidence precedence is current code/tests/configuration, commit ancestry, active
branches/worktrees and their uncommitted state, then documentation claims. Status
headers and checkboxes are claims about the past, not proof.

### Material audit scope

Deep-audit `docs/materials/**`, Material-specific plans, and relevant root claims. Cover
the 83 inherited upstream `docs/wiki/**` documents with one explicit bulk
`historical/upstream, unchanged` rule and list per-file exceptions; do not spend one
ledger row repeating the same classification for every file. The NUL-delimited coverage
gate must still prove that the bulk rule plus exceptions account for every tracked path,
including names Git would otherwise quote. Change an upstream document only when a
Material-specific current claim depends on it. Do not rewrite upstream history for
stylistic consistency.

Known candidates to settle from evidence include:

- the active overview-drag frost defect;
- the outstanding default-preserves-v1 DRM acceptance remainder;
- any noise/saturation follow-up, which becomes a task only if current evidence makes it
  actionable rather than speculative;
- outward status drift between current Material design, rollout, and README claims.

### Prism audit scope

Deep-audit all tracked docs and relevant README claims. Verify the current
`glass.backdropBlur` interface and every implemented/deployed status claim against code
and tests.

After correcting any claim, grep the README, agent guidance, active plans, guides, and
other current docs for the same claim and reconcile propagated drift in the same commit.

## 6. Remaining-outcome and task rules

Before task creation, each ledger records every candidate outcome with its evidence,
source documents, active branch/worktree state, size, proposed status, blockers, final
disposition, and eventual task ID.

Allowed dispositions are:

- evidence-backed unfinished outcome: create `todo`;
- demonstrably active outcome with verified owner: create `todo`, then `start` it;
- unresolved design or unproven status: create `idea` and state the evidence gap;
- actual non-task obstruction: create `todo`, then `block` it with a one-line note;
- already represented by a valid task: preserve it;
- completed, abandoned, speculative, or externally owned history: no task.

The Material overview-drag frost outcome is created as `doing` with owner
`debug/overview-drag-frost`, provided the preflight still proves that active worktree and
its expected dirty state. Migration-authored notes use owner `migration`. Set
`TASKS_OWNER` explicitly for every command that records an owner or note; never allow a
migration branch name or inherited shell value to manufacture ownership.

Tasks receive the `migration` tag plus only tags that support a real query. Dependencies
represent genuine delivery blockers, not advisory order or thematic relationships. Task
bodies state the outcome, acceptance evidence, source documents, and uncertainty. Notes
are single-line summaries.

Structured `spec` and `plan` fields may reference only compatible files under
`docs/specs/**` and `docs/plans/**`. Existing sources elsewhere remain in place and are
cited in the body. The canonical directories created by `tasks init` intentionally
coexist with older layouts.

## 7. Registry and dependency boundary

Seed every temporary registry only through idempotent `tasks init` calls. A repository
migration registry contains the completed six-project baseline, each already-integrated
extension project, and the current migration worktree: seven entries for Material and
eight for Prism. Reconciliation and final portfolio registries contain exactly these
eight stable checkouts:

| Prefix | Project |
|--------|---------|
| `fam` | Familiar |
| `atoms` | Atoms |
| `beliefs` | Beliefs |
| `nodes` | Nodes |
| `mind3` | Mindful v3 |
| `mind6` | Mindful v6 |
| `material` | Material |
| `prism` | Prism |

Persist each temporary root through a guarded control file so separate shell steps do
not depend on shell state. Pin creation to `/tmp`, require non-empty expansions, set
`TASKS_FORMAT=json`, and assert that no relative `tasks/projects.toml` was created before
staging repository task files.

The normal machine registry also contains Autonomy as `aut`, with no task records or
dependency role in this migration. Deliberately exclude it from the exact-eight
temporary registry and leave its normal mapping untouched. `niri-experiments`,
`niri-glass`, and dotfiles are evidence-only boundaries. Cite them by repository,
branch, commit, or document as appropriate; do not invent task IDs or create dangling
dependencies to unmigrated projects.

Material may depend on verified tasks from the completed six-project portfolio. Prism
may also depend on Material after the relevant Material task exists and resolves. Add
only genuine blockers. Because Material migrates first, the only link that can require
deferral is Material to a not-yet-created Prism task; Prism-to-Material links resolve
during the Prism migration. After both initial migrations, reconcile each deferred
Material-to-Prism link, record the merged dependency and zero-warning evidence in the
Material ledger, and classify that ledger complete and historical. Reconcile one
repository at a time and merge it before beginning another, so cycle checks always see
preceding edges.

Every migration and portfolio `tasks check` requires empty errors and warnings. CLI exit
status alone is insufficient because unreachable foreign dependencies and unverifiable
cycles are warnings.

## 8. Verification

Run baseline gates before edits and repeat them in the migration worktree and stable
checkout at the integration points.

Material's noninteractive gate is:

```sh
cargo test --all --exclude niri-visual-tests -- --nocapture
cargo clippy --all --all-targets
cargo fmt --all -- --check
(cd docs && uv sync --locked --all-extras --dev && uv run mkdocs build)
```

This matches the current CI test selection, preserves doctests, deliberately excludes
the GPU-client visual-test package, and includes the strict MkDocs build that validates
the audited wiki. Compare it with current CI during the audit and stop if a relevant job
changed or fails. Source-neutral platform, feature-matrix, visual-client, and deployment
jobs are not rerun locally. Do not redeploy, mutate a live compositor, or rerun
historical DRM, nested-Winit, burn-in, or subjective visual evidence.

Prism's gate is:

```sh
command -v lua
npm ci
npm test
/usr/lib/qt6/bin/qmllint integrations/debug-backdrop/shell.qml
```

Lua is a required development prerequisite because `npm test` invokes the direct plugin
test. The QML lint command must exit zero; only the documented unresolved `qs.*`
warnings are accepted. Record its output disposition in the ledger.

For each repository also require:

- exact document coverage and candidate disposition coverage;
- `tasks check` with no errors or warnings;
- expected `tasks prime` prefix and successful `tasks ready`;
- independent review of the first two commits and their fixes;
- successful stable gates after the initial fast-forward;
- two successful canonical `tasks init --prefix <prefix>` calls from the stable checkout;
- independent review of the ledger-finalization diff;
- a clean stable checkout before worktree and branch cleanup.

The final gate uses a fresh eight-project registry and requires warning-free `tasks
check`, correct `prime`, and inspected `ready` across all eight stable checkouts. It runs
`TASKS_FORMAT=json tasks list --all-projects`, requires command success and an empty
`warnings` array, runs the Tasks repository test suite, and proves that every deferred
dependency is merged and cycle-free.

## 9. Commits and review

Each repository receives these commits:

1. `docs: reconcile project status for tasks migration`
2. `chore(tasks): initialize project task tracking`
3. `docs: record tasks migration integration`

The first commit contains the initial ledger and evidence-backed documentation fixes.
The second contains CLI-created task state, task IDs and verification evidence in the
ledger, and concise agent guidance requiring `tasks prime` at session start and `tasks
check` before completion. Extend Material's existing `.agents/AGENTS.md`; create
Prism's root `AGENTS.md`. The third records completed stable verification and canonical
registration, then marks the ledger complete and historical when no reconciliation is
pending.

Independently review the first two commits before the initial fast-forward and the third
before the final fast-forward. If Material has a deferred Prism link, its third commit
keeps the ledger active with reconciliation pending. After Prism lands, one additional
`chore(tasks): reconcile cross-project dependencies` commit applies the edge, records
its integration and zero-warning evidence, and closes the Material ledger as
`historical/superseded`.

## 10. Failure and recovery

Stop the current migration on:

- prefix-to-path conflict or a project config with the wrong prefix;
- moved branch tips, unexpected dirty files, or failure to preserve the active Material
  debug worktree byte-for-byte;
- unresolved authority contradictions or ownership;
- malformed docs or task records, incomplete coverage, unreachable dependencies, or a
  cycle;
- a repository gate failure not explained by a documented environmental baseline
  exception.

Do not proceed to the next repository around a failed gate. Before the first
fast-forward, recovery remains isolated to the migration worktree and temporary
registry. After it, the stable checkout and normal registry are authoritative while the
migration worktree remains available for ledger finalization.

Registry paths are canonical and machine-local. If checkout relocation produces a burst
of unreachable dependency warnings, explicitly repair the affected normal-registry
mappings among these eight projects with `tasks init` from the stable checkouts and
repeat the eight-project gate. Leave unrelated mappings such as `aut` untouched; do not
alter valid dependency edges to silence an environmental warning.

## 11. Alternatives rejected

### Pair-only registry

A registry containing only Material and Prism is smaller, but it cannot resolve or prove
cycles through dependencies on the completed six-project portfolio. Use the exact
eight-project registry.

### Combined migration branch

One branch spanning two repositories is not reviewable or independently recoverable.
Keep the serial repository gates and commits.

### Reopening the completed portfolio documents

Editing the implemented six-project design and plan would make historical claims less
precise. This extension references that baseline and records only its new scope.

## 12. Completion criteria

The extension is complete when:

- Material and Prism have committed `tasks/.config.toml` files with prefixes `material`
  and `prism`, and their stable roots are canonically registered;
- both ledgers classify every tracked document and themselves, account for every
  candidate outcome, and truthfully record integrated evidence;
- every evidence-backed unfinished outcome has exactly one task, no completed history is
  backfilled, and the active Material debug work is represented without modifying its
  worktree;
- all dependency edits are committed, resolve across the exact eight-project registry,
  and form no cycle;
- both repository gates and all eight portfolio Tasks checks pass with zero Tasks errors
  or warnings;
- no live compositor, deployment, unrelated checkout, or evidence-only repository was
  changed;
- temporary registries, migration worktrees, and merged migration branches are removed;
- this design and its implementation plan truthfully state the completed result on their
  integration branch.
