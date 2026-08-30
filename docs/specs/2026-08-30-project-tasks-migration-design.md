# Existing-project Tasks migration — design

**Status:** design approved for implementation planning (2026-08-30); not implemented.

## 1. Purpose

Migrate Familiar, Science, Atoms, Nodes, Mindful v3, and Mindful v6 to the Tasks
workflow without turning their historical design archives into synthetic task history.
Each migration audits documentation against the repository, corrects drift, groups real
remaining work into independently shippable outcomes, and creates checked-in task files.

The migration is forward-only:

- completed and abandoned work remains in Git and the documentation archive;
- only actionable unfinished work becomes a task;
- uncertain work becomes an `idea` with its evidence gap recorded;
- task files are created and changed only through the `tasks` CLI.

Non-goals:

- moving every historical design into `docs/specs/`;
- converting every unchecked plan step into a task;
- creating closed tasks to reproduce project history;
- standardizing unrelated documentation structure or prose;
- adding CI integration before `tasks` has a pinned install source.

## 2. Projects, prefixes, and order

Prefixes are permanent cross-project identifiers. The compact names retain semantic cues:

| Order | Project | Prefix | Initial authority anchors | Documents at inventory |
|------:|---------|--------|---------------------------|-----------------------:|
| 1 | Familiar | `fam` | `README.md`, `docs/surfaces.md`, current specs/plans | 17 |
| 2 | Atoms | `atoms` | authority design named by `AGENTS.md`, obligation ledger, README | 35 |
| 3 | Science | `sci` | README, current roadmap/adoption ledgers, guide | 85 |
| 4 | Nodes | `nodes` | `docs/STANDARD.md`, code/tests, README | 44 |
| 5 | Mindful v3 | `mind3` | README, `AGENTS.md`, current roadmap/active plans | 107 |
| 6 | Mindful v6 | `mind6` | `docs/ARCHITECTURE.md`, `docs/FORMATS.md`, README | 124 |

The counts are the 2026-08-30 inventory snapshot, not continuing invariants.

Familiar is the pilot. Atoms precedes its Science consumer. Nodes and Mindful v3
precede Mindful v6, which consumes Nodes and carries an explicit v3 import boundary.
Only one repository is migrated at a time. A repository must pass its integration gate
before the next migration starts.

## 3. Repository-gated workflow

Before the Familiar pilot, install one reviewed `tasks` binary and the Tasks agent skill
at user level. Record the Tasks source commit used for the migration and use that binary
for all six repositories. Do not pin six project-local skill copies.

Each project uses a fresh migration worktree and the same pipeline:

```text
preflight and Git inventory
  -> document classification
  -> evidence-backed drift corrections
  -> remaining-outcome synthesis
  -> Tasks initialization and task creation
  -> validation and independent review
  -> integration and canonical registration
```

Existing branches, linked worktrees, and uncommitted changes are inspected read-only.
All migration writes occur in the dedicated worktree. No current worktree is repurposed,
cleaned, or modified. Run the repository's required gates before editing; a failing
baseline is reported and resolved or explicitly deferred before migration work begins.

Never let a migration worktree's branch name become task ownership evidence. Set
`TASKS_OWNER` explicitly on every migration command that records an owner or note. Use
`migration` for migration-authored notes and the verified existing owner for `start`. If
the audit cannot identify a valid owner, do not mark the task `doing`; record the branch
evidence in its body and leave it `todo` or `idea`. Owner tokens must match
`[A-Za-z0-9._/@+-]+`; put human display names or other punctuation in the body.

Each project adds one concise ledger:

```text
docs/plans/YYYY-MM-DD-<project>-tasks-migration.md
```

The ledger is the durable evidence record for that repository, not an implementation
plan that every task must cite. It contains:

1. stable checkout HEAD and the local branches/worktrees inspected;
2. authority/current, active-delivery, and historical document classifications;
3. drift findings, evidence, corrections, and outward-grep results;
4. candidate outcomes and their disposition;
5. the local prefix plus any foreign prefixes used;
6. created task IDs and deferred cross-project links;
7. verification commands and results.

## 4. Tiered documentation audit

Every project document receives one classification:

- **Authority/current:** semantically verify it against code, tests, schemas,
  configuration, and current Git state.
- **Active delivery:** verify status, checkboxes, dependencies, and remaining-work
  claims; deeply verify the technical sections that govern unfinished work.
- **Historical/superseded:** preserve its rationale and old snippets. Correct only
  misleading status, broken authority links, or claims repeated by current user-facing
  documentation.

Evidence order is:

1. current code, tests, schemas, and configuration;
2. commit ancestry and merged history;
3. active local branches/worktrees and their uncommitted state;
4. documentation claims.

A status header, checked box, unchecked box, branch name, or migration ledger entry is a
claim, not proof. When the evidence cannot settle a claim, the ledger records the
ambiguity instead of guessing.

Drift corrections land before tasks are created. Corrections cover stale status and
checkboxes, obsolete module/path names, broken authority links, supersession labels,
current roadmaps, and README or agent-guidance summaries. After changing a claim, grep
README files, agent guidance, roadmaps, guides, and active plans for the same claim and
correct any propagated drift in the same change.

Historical documents are not rewritten to describe today's architecture. If a current
document depends on obsolete history, correct the current document or add a concise
supersession note to the historical source.

## 5. Remaining-outcome synthesis

Before task creation, the ledger builds a candidate-outcomes table with:

| Field | Meaning |
|-------|---------|
| Outcome | Independently shippable result, not an implementation step |
| Evidence | Why the outcome is known to remain unfinished |
| Sources | Governing or explanatory project documents |
| Active state | Relevant branch/worktree and owner evidence, if any |
| Size | `xs`, `s`, `m`, `l`, or `xl` |
| Proposed status | `idea`, `todo`, `doing`, or `blocked` |
| Blockers | Only work that truly prevents delivery |
| Task ID | Filled after CLI creation |

Conversion rules:

- verified unfinished outcome -> `todo`;
- demonstrably active outcome -> `doing`;
- unproven status -> `idea`, with the missing evidence in the body;
- external obstruction that is not another task -> `blocked`, with a note;
- completed or abandoned history -> no task.

`tasks add` accepts only `idea` and `todo`. Apply the table with these command sequences:

- `idea`: `tasks add <title> --status idea ...`;
- `todo`: `tasks add <title> --status todo ...`;
- `doing`: add it as `todo`, then run
  `TASKS_OWNER=<verified-owner> tasks start <id>`;
- `blocked`: add it as `todo`, then run
  `TASKS_OWNER=migration tasks block <id> "<reason>"`.

An open dependency is represented by `depends`, not by also marking the task `blocked`.
Only verified delivery blockers become dependencies. Advisory sequencing and thematic
relationships stay in task bodies.

Tasks receive the `migration` tag. Add another tag only when it supports a real project
query, and pass the complete initial tag set as repeated `--tag` arguments in the single
`add` command. `edit --tag` replaces the entire tag list; any later tag edit must repeat
the complete desired set, including `migration`. Task bodies state the outcome,
acceptance evidence, source documents, and any audit uncertainty. A body cannot contain
a bare line equal to `## Notes`; quote that historical heading as `> ## Notes` or mention
it inline. Note and close/block messages are single-line summaries with no newline or
carriage return; multi-line evidence belongs in the body. Existing compatible
`docs/plans/` files and exact headings use the structured `plan` and `step` fields.
Existing `docs/designs/` files are referenced in the body rather than moved merely to
satisfy the Tasks path convention. `tasks init` still creates `docs/specs/` and
`docs/plans/`; those canonical directories intentionally coexist with historical layouts.
New designs and plans use the canonical directories.

## 6. Cross-project dependencies

The migration order makes the common blocker direction resolvable: Atoms before Science,
and Nodes plus Mindful v3 before Mindful v6. A foreign dependency is added only after its
task exists and resolves through the staging registry.

If a verified blocker points to a project that has not yet been migrated, record the
relationship in the current ledger without creating a dangling dependency. After all
initial migrations, run a conditional reconciliation phase only for ledgers with deferred
links:

1. create a temporary portfolio registry containing exactly the six stable checkouts;
2. create a fresh reconciliation worktree from the affected repository's stable branch;
3. run `tasks dep <id> --on <foreign-id>` in that worktree;
4. update the ledger so no deferred link remains;
5. run the repository gates and zero-warning `tasks check` against the portfolio registry;
6. commit as `chore(tasks): reconcile cross-project dependencies`, independently review,
   merge, and remove the reconciliation worktree.

No stable checkout is edited directly, and every dependency edit is committed. If no
ledger has a deferred link, the reconciliation phase is a no-op. A cycle in a fully
reachable graph is an error. If any foreign project is unreachable, cycle verification
degrades to a warning; the migration's zero-warning gate treats that as unresolved rather
than accepting the command's exit code.

## 7. Worktree-safe registry handling

`tasks init` registers the canonicalized directory where it runs. Registering a temporary
worktree in the normal machine registry would leave a stale path after cleanup. Migration
commands therefore use a temporary configuration root created with `mktemp -d`:

```sh
tasks_migration_config=$(mktemp -d)
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C <stable-prior-repo> init --prefix <prior-prefix>
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C <migration-worktree> init --prefix <prefix>
```

Seed the temporary registry with already-migrated stable checkouts needed for foreign
resolution. Use the same temporary root for all commands in the current migration.
Every command that records ownership or a note receives both overrides explicitly:

```sh
XDG_CONFIG_HOME="$tasks_migration_config" \
TASKS_OWNER=<verified-owner-or-migration> \
tasks -C <migration-worktree> <mutation>
```

After review and merge:

1. run `tasks -C <stable-checkout> init --prefix <prefix>` without the temporary override;
2. verify the normal registry resolves the stable checkout;
3. remove the migration worktree;
4. remove the temporary configuration directory.

A prefix already registered to another path is a hard stop for explicit reconciliation.
Never overwrite or silently ignore it.

## 8. Commits and review

Each repository migration has two logical commits:

1. `docs: reconcile project status for tasks migration`
   - add/update the ledger;
   - correct documentation drift and propagated claims.
2. `chore(tasks): initialize project task tracking`
   - initialize `tasks/`;
   - create tasks and dependencies through the CLI;
   - fill task IDs and verification evidence into the ledger;
   - update agent guidance with the Tasks session/completion gates.

More commits are allowed only when a repository's existing review or test workflow needs
them; do not split mechanical file-by-file changes. Independently review the complete
repository migration before integration. Fix load-bearing findings, rerun affected
checks, and preserve unrelated user changes. The conditional reconciliation phase in §6
adds one later commit only to repositories with deferred foreign links.

## 9. Verification and enforcement

For each repository:

1. run repository-specific documentation/status checks;
2. run the repository's existing required test, type, lint, and formatting gates;
3. run `tasks check` and require both `errors` and `warnings` to be empty; exit status 0
   alone is insufficient because warnings do not change it;
4. smoke-test `tasks prime` and `tasks ready`;
5. after canonical registration, rerun `tasks init --prefix <prefix>` and `tasks prime`
   from the stable checkout; successful idempotent init proves the normal registry entry
   matches that root, and prime must report the expected prefix;
6. verify a clean diff/status and review the complete initial migration range.

After all migrations and final dependency reconciliation:

1. create a fresh temporary portfolio registry containing exactly the six stable
   checkouts, independent of unrelated entries in the normal machine registry;
   use that registry for every command in items 2–5;
2. against that registry, run `tasks check` from all six stable checkouts and require
   zero errors and warnings;
3. run `tasks prime` in every stable checkout and confirm its expected prefix;
4. run `tasks list --all-projects`, require command success, and resolve every warning;
5. inspect `tasks ready` in every project;
6. confirm no task backfills completed history;
7. confirm every unresolved audit item is represented by an `idea` and every deferred
   dependency has a merged reconciliation commit.

Agent guidance in each project requires `tasks prime` at session start and `tasks check`
before completion. Do not add `command -v tasks && tasks check` or another silent CI
fallback. CI enforcement waits until `tasks` has a pinned install source; until then the
documented local gate fails explicitly when the binary is absent.

Foreign dependency resolution and cycle proof are machine-local because the registry is
machine-local. Task files remain portable, but another machine must register every
referenced project before its zero-warning check is meaningful. Portfolio completion is
therefore proven on the migration machine; equivalent CI enforcement also requires a
pinned Tasks install and deterministic registry bootstrap.

## 10. Failure and recovery

Stop the current migration when:

- authority documents contradict code/tests and the discrepancy cannot be resolved;
- active work ownership or intended landing state is unclear;
- unrelated dirty changes overlap a required correction;
- a plan step, dependency, registry entry, project gate, or `tasks check` fails;
- task dependencies form a cycle.

Do not proceed to the next repository around a failed gate. Earlier repositories remain
usable; current writes remain isolated in the migration worktree and temporary registry.
If the branch is abandoned before merge, discard its temporary registry without changing
the normal registry. After merge, the stable checkout and normal registry are the source
of truth.

## 11. Alternatives rejected

### Portfolio-first audit

Auditing all projects before creating any tasks would maximize batch consistency but delay
feedback and hold a large set of drift corrections open. The Familiar pilot provides the
same learning sooner.

### Task-first capture

Creating candidate tasks during the audit would preserve discoveries quickly, but tasks
would temporarily cite stale documents and require promotion/rewriting churn. The ledger
captures candidates until the evidence and documents are ready.

### Historical task backfill

Closed tasks for hundreds of completed plans would duplicate Git history, obscure the
actionable list, and invent ownership/timestamps that Tasks cannot prove. Historical docs
remain the record.

## 12. Completion criteria

The portfolio migration is complete when:

- every repository has its approved permanent prefix and a committed `tasks/.config.toml`;
- all six stable checkouts are present in the normal registry;
- each project has a completed migration ledger and corrected current documentation;
- every evidence-backed remaining outcome has one task and no completed history was
  backfilled;
- every deferred foreign dependency edit is committed and all foreign dependencies
  resolve, with no cycles;
- all six repositories pass their existing gates and `tasks check` with zero errors and
  zero warnings;
- `tasks prime` reports `fam`, `atoms`, `sci`, `nodes`, `mind3`, and `mind6` from their
  stable checkouts, and global listing against the six-project portfolio registry
  succeeds with no warning;
- every unresolved claim is visible as an `idea`, not silently treated as done or ready.
