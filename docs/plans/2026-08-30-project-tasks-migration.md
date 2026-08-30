# Existing-project Tasks Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Audit Familiar, Atoms, Beliefs, Nodes, Mindful v3, and Mindful v6 against their repositories, correct current documentation drift, and create forward-only Tasks stores for every evidence-backed remaining outcome.

**Architecture:** Migrate one repository at a time in a fresh worktree, landing a documentation reconciliation commit before a Tasks initialization commit. Review and integrate those two commits, verify and register the stable checkout, then record that evidence in a reviewed ledger-finalization commit before cleanup. Use CLI-built temporary registries for migration and portfolio validation, integrate each repository before beginning the next, then reconcile only the cross-project dependencies that had to be deferred.

**Tech Stack:** Rust `tasks` CLI, Git worktrees, Markdown, TOML, zsh, `jq`, npm, uv, and Docker Compose.

**Spec:** `docs/specs/2026-08-30-project-tasks-migration-design.md`

## Global Constraints

- Migrate in this order: Familiar (`fam`), Atoms (`atoms`), Beliefs (`beliefs`), Nodes (`nodes`), Mindful v3 (`mind3`), Mindful v6 (`mind6`).
- Migrate and reconcile only one repository at a time; merge and verify it before creating the next worktree.
- Inspect existing branches, linked worktrees, and uncommitted state read-only. Make all migration writes in a fresh `.worktrees/` worktree.
- Keep each migration worktree through canonical registration and its reviewed post-registration ledger commit; remove it only after the second fast-forward merge.
- Do not create tasks for completed or abandoned history. Create tasks only for actionable unfinished outcomes; use `idea` when evidence is inconclusive.
- Size tasks as independently shippable outcomes with `xs`, `s`, `m`, `l`, or `xl`, never as implementation steps.
- `tasks add` creates only `idea` or `todo`. Create `doing` by adding `todo` then running `tasks start`; create `blocked` by adding `todo` then running `tasks block`.
- Set `TASKS_OWNER=migration` for migration-authored notes. Use a verified owner matching `[A-Za-z0-9._/@+-]+` for `tasks start`; otherwise leave the task `todo` or `idea`.
- Give every created task the `migration` tag. Pass the complete tag set to `tasks add`; later `edit --tag` operations replace all tags.
- Task bodies must state outcome, acceptance evidence, source documents, and uncertainty. They must not contain a bare `## Notes` line. Note and close/block messages must be single-line.
- Add dependencies only for verified delivery blockers. Record blockers targeting an unmigrated project in the ledger and defer their CLI edge until reconciliation.
- Use structured `spec`, `plan`, and `step` fields only for documents under canonical `docs/specs/` and `docs/plans/`; cite historical layouts such as `docs/designs/` in the body.
- Migration and portfolio `tasks check` gates require empty `errors` and `warnings` arrays, not merely exit status zero.
- Day-to-day guidance requires zero errors and reporting every warning. Only registration-caused `unreachable_dep` and `cycle_unverifiable` warnings are expected on partially registered machines.
- Create task files and mutate task state only through the CLI. Build every migration/portfolio registry through `tasks init`; never hand-edit `tasks/*.md` or a temporary registry's `projects.toml`.
- Do not add CI enforcement until `tasks` has a pinned install source.
- Use conventional commits with no AI attribution. Preserve unrelated user changes.
- Use portable checkout notation such as `~/d/familiar` in committed documentation; do not record machine-resolved absolute paths.
- Treat every checkbox step as a fresh shell invocation: begin success-path blocks with `set -euo pipefail`, set the working directory explicitly before relative commands, and re-derive all cross-step state from the named control file.

## Shared Migration Ledger Contract

Every repository ledger contains these exact sections:

1. `Scope and evidence` — stable HEAD, Tasks source commit, audit date, and prefix.
2. `Git state inspected` — every local branch, linked worktree, and dirty path reviewed read-only.
3. `Document classification` — one row for each tracked file under `docs/`, the migration ledger itself, and existing root `README.md`, `AGENTS.md`, and `CLAUDE.md`.
4. `Drift corrections` — evidence, correction, and outward-grep result for every changed claim.
5. `Candidate outcomes` — outcome, evidence, sources, active state, size, proposed status, blockers, disposition, and task ID.
6. `Deferred foreign dependencies` — local task, future project/outcome, evidence, and `pending` or `reconciled`; write `None` when empty.
7. `Verification` — exact command, result, and commit containing the result.

Use exactly one classification per document: `authority/current`, `active delivery`, or `historical/superseded`. Each repository task supplies its literal ledger path and exact coverage command; expected output is empty.

After stable verification and canonical registration, update the ledger in the still-open migration worktree. If `Deferred foreign dependencies` has no pending row, mark the migration complete and classify the ledger itself `historical/superseded`. If any pending row remains, mark the initial migration integrated but reconciliation pending and keep the ledger `active delivery` until Task 8 finalizes it.

## Shared Agent Guidance

Add this concise section to the repository's existing agent guidance, or create `AGENTS.md` when none exists:

```markdown
## Tasks workflow

- Run `tasks prime` at the start of a work session and `tasks ready` before choosing work.
- Run `tasks start ID` before implementation, add concise notes as evidence changes, and close the task with a one-line result in the same commit as the work.
- Never edit `tasks/*.md` directly; use the `tasks` CLI for every task mutation.
- Before completion, run `tasks check`. Require zero errors and report every warning. Registration-only `unreachable_dep` and `cycle_unverifiable` warnings are environmental on machines without all referenced projects; resolve every other warning.
```

## Shared Task-Creation Procedure

For each reviewed `Candidate outcomes` row whose disposition is `create`:

1. Run `tasks add` with the row's literal title, body, size, `idea` or `todo` status, and every tag as repeated `--tag` flags.
2. Add each already-resolvable blocker with `tasks dep ID --on BLOCKER_ID`.
3. For a verified active owner, read the emitted task ID and verified owner token into `task_id` and `verified_owner`, then run `TASKS_OWNER="$verified_owner" tasks start "$task_id"`; otherwise retain `todo` or `idea`.
4. For a non-task obstruction, read the emitted ID into `task_id`, then run `TASKS_OWNER=migration tasks block "$task_id" 'one-line reason copied from the reviewed row'`.
5. Copy the emitted ID into the same candidate row. Never infer an ID or edit the generated Markdown.
6. Run `tasks show ID` and compare every field with the ledger row before continuing.

All CLI calls in a migration worktree use the same temporary `XDG_CONFIG_HOME`. The initialization step writes that directory's path to the task-specific `/tmp/*.registry-path` file named below; every later step re-reads it, verifies the directory exists, and passes `XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"`. Pin `TASKS_FORMAT=json` for every command consumed as JSON. Every mutation that records ownership or a note also sets `TASKS_OWNER` explicitly.

## Shared Post-registration Ledger Finalization

For Tasks 2–7, independently review the documentation reconciliation and Tasks initialization commits before the first fast-forward. Then:

1. Fast-forward those two commits into the stable checkout and run its complete repository gate.
2. Against the normal registry, run `tasks init --prefix <prefix>` twice, `tasks check` with empty `errors` and `warnings`, `tasks prime` with the exact prefix, and `tasks ready` from the stable checkout.
3. Keep the migration worktree, branch, and temporary registry. Use `apply_patch` in that worktree to record the exact integrated commits and the past-tense stable gate and registration results in the ledger, and apply the ledger self-classification rule above.
4. Rerun that task's exact ledger coverage comparison and `git diff --check`, stage only the ledger, and commit with the exact subject `docs: record tasks migration integration`.
5. Independently review the ledger-finalization diff. Fix findings with `apply_patch`, rerun the affected checks, and amend the commit without changing its subject.
6. Fast-forward the reviewed finalization commit into the stable checkout. Only then remove the migration worktree and branch, temporary registry, and registry-path control file.

The initial migration therefore has three commits. The first two receive the pre-integration review; the third receives its own review before the second fast-forward and cleanup.

---

### Task 1: Pin the Tasks Tool and Preflight the Portfolio

**Files:**

- Read: `Cargo.toml`
- Read: `src/`
- Read: `skills/tasks/SKILL.md`
- Inspect: the six stable checkouts and their linked worktrees

**Interfaces:**

- Produces: one tested `tasks` binary, the user-level Tasks skill, the recorded Tasks source commit, and six known stable roots for Tasks 2–9.

- [ ] **Step 1: Verify the Tasks source checkout**

```bash
set -euo pipefail
cd ~/d/tasks
git status --short --branch
git rev-parse HEAD
cargo test
```

Expected: the worktree has no unexplained changes and the complete suite passes with no failures. Record the exact commit for every repository ledger.

- [ ] **Step 2: Install the reviewed binary once**

```bash
set -euo pipefail
cd ~/d/tasks
cargo install --locked --path . --force
command -v tasks
tasks --version
```

Expected: installation succeeds and `tasks --version` executes from the installed binary.

- [ ] **Step 3: Verify the user-level Tasks skill**

```bash
set -euo pipefail
for skill_root in "$HOME/.agents/skills" "$HOME/.claude/skills"; do
  skill_link="$skill_root/tasks"
  if test -L "$skill_link" && ! test -e "$skill_link"; then
    printf 'broken Tasks skill link: %s\n' "$skill_link" >&2
    exit 1
  fi
  if ! test -e "$skill_link"; then
    mkdir -p "$skill_root"
    ln -s "$HOME/d/tasks/skills/tasks" "$skill_link"
  fi
  test "$skill_link" -ef "$HOME/d/tasks/skills/tasks"
done
```

If either path exists but the same-file check fails, stop and reconcile it explicitly rather than overwriting it. The CLI recognizes either location; installing both also makes the skill available to Claude Code and other agent harnesses. Begin Task 2 in a fresh agent session so skill discovery observes the new links.

- [ ] **Step 4: Inventory all stable roots without modifying them**

```bash
set -euo pipefail
for repo in ~/d/familiar ~/d/atoms ~/d/beliefs ~/d/nodes ~/d/mindful/v3 ~/d/mindful/v6; do
  git -C "$repo" status --short --branch
  git -C "$repo" worktree list --porcelain
  git -C "$repo" branch --format='%(refname:short) %(objectname)'
  git -C "$repo" check-ignore -q .worktrees || printf 'NOT_IGNORED %s\n' "$repo"
done
```

Expected: every root is identified, existing dirty state is recorded but untouched, and `.worktrees/` is ignored. Stop before migration if a required correction would overlap unrelated dirty work.

- [ ] **Step 5: Inspect the normal Tasks registry read-only**

```bash
set -euo pipefail
tasks_registry_home=${XDG_CONFIG_HOME:-"$HOME/.config"}
tasks_registry_path="$tasks_registry_home/tasks/projects.toml"
test ! -e "$tasks_registry_path" || sed -n '1,200p' "$tasks_registry_path"
```

Record existing prefix mappings. Do not repair or hand-edit the registry during preflight; any conflicting prefix is a hard stop for that repository's integration step.

- [ ] **Step 6: Record the preflight result**

No tracked commit is expected. Preserve the Tasks source commit and discovered stable branch for use in each ledger.

### Task 2: Migrate Familiar as the Pilot

**Files:**

- Create: `~/d/familiar/.worktrees/tasks-migration-fam/docs/plans/2026-08-30-familiar-tasks-migration.md`
- Create through CLI: `~/d/familiar/.worktrees/tasks-migration-fam/tasks/.config.toml`
- Create through CLI: `~/d/familiar/.worktrees/tasks-migration-fam/tasks/fam-*.md`
- Modify: current drifted files found under `README.md` and `docs/`
- Modify or create: `AGENTS.md`

**Interfaces:**

- Consumes: the Tasks source commit and stable-root inventory from Task 1.
- Produces: the first integrated `fam` store, the approved ledger pattern, and a stable Familiar checkout available to later temporary registries.

- [ ] **Step 1: Create an isolated pilot worktree**

```bash
set -euo pipefail
git -C ~/d/familiar worktree add -b chore/tasks-migration-fam ~/d/familiar/.worktrees/tasks-migration-fam main
git -C ~/d/familiar/.worktrees/tasks-migration-fam status --short --branch
```

Expected: a clean branch based on the current `main` commit recorded in the ledger.

- [ ] **Step 2: Establish the unmodified baseline**

```bash
set -euo pipefail
cd ~/d/familiar/.worktrees/tasks-migration-fam
npm ci
test -z "$(git status --porcelain=v1)"
npm test
```

Expected: setup changes no tracked or untracked path and the repository's existing gate passes. Stop and report a baseline failure before changing documentation.

- [ ] **Step 3: Audit Git state and all project documents**

Read `README.md`, `docs/surfaces.md`, every tracked file under `docs/`, and existing root guidance. For every status header, checkbox, path, current-behavior claim, and remaining-work claim, verify against code/tests/configuration, commit ancestry, and the read-only branch/worktree inventory in that order. Write every file into `docs/plans/2026-08-30-familiar-tasks-migration.md` using the shared ledger contract.

- [ ] **Step 4: Correct evidence-backed drift**

Use `apply_patch` for the ledger and current documentation. Preserve historical rationale, make uncertainty explicit, and grep each corrected claim through user-facing and active-delivery documents:

```bash
set -euo pipefail
cd ~/d/familiar/.worktrees/tasks-migration-fam
audit_paths=(docs)
for f in README.md AGENTS.md CLAUDE.md; do test ! -f "$f" || audit_paths+=("$f"); done
rg -n 'Status:|to.?do|unchecked|supersed|README|surfaces|docs/(specs|plans|designs)' "${audit_paths[@]}" || { audit_rg_status=$?; test "$audit_rg_status" -eq 1; }
comm -3 \
  <({ git ls-files docs; printf '%s\n' docs/plans/2026-08-30-familiar-tasks-migration.md; for f in README.md AGENTS.md CLAUDE.md; do test ! -f "$f" || echo "$f"; done; } | sort -u) \
  <(sed -n '/^## Document classification$/,/^## /p' docs/plans/2026-08-30-familiar-tasks-migration.md | awk -F'`' '/^\| `/{print $2}' | sort -u)
git diff --check
```

Expected: the coverage comparison and `git diff --check` produce no output.

- [ ] **Step 5: Review and commit the documentation reconciliation**

Compare every ledger assertion with its cited evidence, run `npm test`, then commit only the ledger and drift corrections:

```bash
set -euo pipefail
cd ~/d/familiar/.worktrees/tasks-migration-fam
npm test
git add README.md AGENTS.md docs
test ! -e CLAUDE.md || git add CLAUDE.md
git diff --cached --check
git commit -m "docs: reconcile project status for tasks migration"
```

Pathspecs that do not exist may be omitted. Expected: one reviewable documentation commit and a clean worktree.

- [ ] **Step 6: Prove the Tasks store is absent, then initialize it in a temporary registry**

```bash
tasks -C ~/d/familiar/.worktrees/tasks-migration-fam prime
```

Expected before initialization: explicit `no_project` failure.

```bash
set -euo pipefail
test ! -e /tmp/tasks-migration-fam.registry-path
tasks_migration_config=$(TMPDIR=/tmp mktemp -d)
printf '%s\n' "$tasks_migration_config" >/tmp/tasks-migration-fam.registry-path
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
tasks -C ~/d/familiar/.worktrees/tasks-migration-fam init --prefix fam
tasks -C ~/d/familiar/.worktrees/tasks-migration-fam prime | jq -e '.prefix == "fam"'
```

Expected: `prime` reports prefix `fam`.

- [ ] **Step 7: Create and verify the reviewed Familiar outcomes**

In the shell that applies the shared task-creation procedure, first run:

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-fam.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
cd ~/d/familiar/.worktrees/tasks-migration-fam
```

Then create every ledger row marked `create`, record every generated ID, and leave completed/abandoned rows as `no task`. Add the shared agent guidance after task creation.

- [ ] **Step 8: Run the pilot's Tasks and repository gates**

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-fam.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
cd ~/d/familiar/.worktrees/tasks-migration-fam
tasks -C ~/d/familiar/.worktrees/tasks-migration-fam check >/tmp/fam-check.json
jq -e '.errors == [] and .warnings == []' /tmp/fam-check.json
tasks -C ~/d/familiar/.worktrees/tasks-migration-fam prime | jq -e '.prefix == "fam"'
tasks -C ~/d/familiar/.worktrees/tasks-migration-fam ready
npm test
git diff --check
```

Expected: all commands pass; the JSON arrays are empty.

- [ ] **Step 9: Commit and independently review the Tasks migration**

```bash
set -euo pipefail
cd ~/d/familiar/.worktrees/tasks-migration-fam
test ! -e tasks/projects.toml
git add tasks AGENTS.md docs/plans/2026-08-30-familiar-tasks-migration.md
git diff --cached --check
git commit -m "chore(tasks): initialize project task tracking"
git status --short
```

Independently review both initial migration commits against the design, ledger evidence, task files, and baseline diff before the first integration. Fix load-bearing findings through the CLI or `apply_patch`, rerun affected gates, and use conventional fixup commits before integration.

- [ ] **Step 10: Integrate, register, finalize, and clean up the pilot**

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-fam.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
git -C ~/d/familiar merge --ff-only chore/tasks-migration-fam
cd ~/d/familiar
npm test
TASKS_FORMAT=json tasks init --prefix fam
TASKS_FORMAT=json tasks init --prefix fam
TASKS_FORMAT=json tasks check >/tmp/fam-stable-check.json
jq -e '.errors == [] and .warnings == []' /tmp/fam-stable-check.json
TASKS_FORMAT=json tasks prime | jq -e '.prefix == "fam"'
TASKS_FORMAT=json tasks ready
git status --short --branch
```

Keep the migration worktree and temporary registry. Use `apply_patch` there to record the
two exact integrated commits plus the past-tense stable test, Tasks gate, normal-registry,
prime, and ready results. Familiar has no deferred foreign dependency, so mark the
migration complete and classify the ledger row for itself `historical/superseded`.
Rerun Step 4's exact coverage comparison and `git diff --check`, then:

```bash
set -euo pipefail
cd ~/d/familiar/.worktrees/tasks-migration-fam
git add docs/plans/2026-08-30-familiar-tasks-migration.md
git diff --cached --check
git commit -m "docs: record tasks migration integration"
git status --short
```

Independently review this ledger-only diff and correct any finding before the second
fast-forward. Then:

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-fam.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
git -C ~/d/familiar merge --ff-only chore/tasks-migration-fam
git -C ~/d/familiar status --short --branch
git worktree remove ~/d/familiar/.worktrees/tasks-migration-fam
git branch -d chore/tasks-migration-fam
case "$tasks_migration_config" in /tmp/*) rm -r -- "$tasks_migration_config" ;; *) exit 1 ;; esac
rm -- /tmp/tasks-migration-fam.registry-path
```

Expected: `main` contains all three initial-migration commits, the normal registry maps
`fam` to the stable checkout, the ledger is historical, and the migration worktree is
gone. If the pilot exposes a contradiction in the design or ledger contract, stop, amend
the design and this plan, review those changes, and only then begin Atoms.

### Task 3: Migrate Atoms

**Files:**

- Create: `~/d/atoms/.worktrees/tasks-migration-atoms/docs/plans/2026-08-30-atoms-tasks-migration.md`
- Create through CLI: `~/d/atoms/.worktrees/tasks-migration-atoms/tasks/.config.toml`
- Create through CLI: `~/d/atoms/.worktrees/tasks-migration-atoms/tasks/atoms-*.md`
- Modify: evidence-backed drift under `README.md` and `docs/`
- Modify: `AGENTS.md`

**Interfaces:**

- Consumes: integrated Familiar and the approved pilot procedure.
- Produces: integrated `atoms` tasks and a stable producer for Beliefs dependencies.

- [ ] **Step 1: Create the Atoms worktree and run its baseline**

```bash
set -euo pipefail
git -C ~/d/atoms worktree add -b chore/tasks-migration-atoms ~/d/atoms/.worktrees/tasks-migration-atoms main
cd ~/d/atoms/.worktrees/tasks-migration-atoms/python
uv run pytest
uv run ruff check .
uv run pyright
```

Expected: all existing gates pass before edits.

- [ ] **Step 2: Audit Atoms and write its ledger**

From the worktree root, read `AGENTS.md` first and treat its named authority design and obligation ledger as current anchors. Classify every document, verify A1–A9 and certification/adoption claims against code/tests/history, and distinguish Atoms delivery from Beliefs-consumer work. Write `docs/plans/2026-08-30-atoms-tasks-migration.md` with the shared contract.

- [ ] **Step 3: Correct drift, prove coverage, and commit documentation**

```bash
set -euo pipefail
cd ~/d/atoms/.worktrees/tasks-migration-atoms
audit_paths=(docs)
for f in README.md AGENTS.md CLAUDE.md; do test ! -f "$f" || audit_paths+=("$f"); done
rg -n 'Status:|A[1-9]|obligation|certif|adopt|Beliefs|supersed' "${audit_paths[@]}" || { audit_rg_status=$?; test "$audit_rg_status" -eq 1; }
comm -3 \
  <({ git ls-files docs; printf '%s\n' docs/plans/2026-08-30-atoms-tasks-migration.md; for f in README.md AGENTS.md CLAUDE.md; do test ! -f "$f" || echo "$f"; done; } | sort -u) \
  <(sed -n '/^## Document classification$/,/^## /p' docs/plans/2026-08-30-atoms-tasks-migration.md | awk -F'`' '/^\| `/{print $2}' | sort -u)
git diff --check
cd python && uv run pytest && uv run ruff check . && uv run pyright
cd ..
git add README.md AGENTS.md docs
test ! -e CLAUDE.md || git add CLAUDE.md
git diff --cached --check
git commit -m "docs: reconcile project status for tasks migration"
```

Expected: coverage is exact, gates pass, and the first commit contains only evidence and drift reconciliation.

- [ ] **Step 4: Initialize Atoms with Familiar resolvable**

```bash
tasks -C ~/d/atoms/.worktrees/tasks-migration-atoms prime
```

Expected: explicit `no_project` failure.

```bash
set -euo pipefail
test ! -e /tmp/tasks-migration-atoms.registry-path
tasks_migration_config=$(TMPDIR=/tmp mktemp -d)
printf '%s\n' "$tasks_migration_config" >/tmp/tasks-migration-atoms.registry-path
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
tasks -C ~/d/familiar init --prefix fam
tasks -C ~/d/atoms/.worktrees/tasks-migration-atoms init --prefix atoms
```

- [ ] **Step 5: Create Atoms tasks and record future Beliefs blockers**

Re-derive the registry before applying the shared task-creation procedure:

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-atoms.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
cd ~/d/atoms/.worktrees/tasks-migration-atoms
```

Create an A9-related task only when the repository evidence proves remaining delivery work. Add resolvable dependencies now; record any verified blocker pointing to not-yet-migrated Beliefs as `pending` in `Deferred foreign dependencies` without creating a dangling edge. Add the shared agent guidance.

- [ ] **Step 6: Verify, commit, and review Atoms**

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-atoms.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
tasks -C ~/d/atoms/.worktrees/tasks-migration-atoms check >/tmp/atoms-check.json
jq -e '.errors == [] and .warnings == []' /tmp/atoms-check.json
tasks -C ~/d/atoms/.worktrees/tasks-migration-atoms prime | jq -e '.prefix == "atoms"'
tasks -C ~/d/atoms/.worktrees/tasks-migration-atoms ready
cd ~/d/atoms/.worktrees/tasks-migration-atoms/python
uv run pytest
uv run ruff check .
uv run pyright
cd ..
test ! -e tasks/projects.toml
git add tasks AGENTS.md docs/plans/2026-08-30-atoms-tasks-migration.md
git diff --cached --check
git commit -m "chore(tasks): initialize project task tracking"
git status --short
```

Independently review both initial migration commits and correct findings before the first integration.

- [ ] **Step 7: Integrate, register, finalize, and clean up Atoms**

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-atoms.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
git -C ~/d/atoms merge --ff-only chore/tasks-migration-atoms
cd ~/d/atoms/python && uv run pytest && uv run ruff check . && uv run pyright
cd ..
TASKS_FORMAT=json tasks init --prefix atoms
TASKS_FORMAT=json tasks init --prefix atoms
TASKS_FORMAT=json tasks check >/tmp/atoms-stable-check.json
jq -e '.errors == [] and .warnings == []' /tmp/atoms-stable-check.json
TASKS_FORMAT=json tasks prime | jq -e '.prefix == "atoms"'
TASKS_FORMAT=json tasks ready
git status --short --branch
```

Apply the Shared Post-registration Ledger Finalization to
`docs/plans/2026-08-30-atoms-tasks-migration.md`, rerunning Step 3's exact coverage
comparison. Base the ledger status and self-classification on whether any deferred row is
still pending. After its ledger-only commit passes independent review, run the second
fast-forward and clean up:

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-atoms.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
git -C ~/d/atoms merge --ff-only chore/tasks-migration-atoms
git -C ~/d/atoms status --short --branch
git worktree remove ~/d/atoms/.worktrees/tasks-migration-atoms
git branch -d chore/tasks-migration-atoms
case "$tasks_migration_config" in /tmp/*) rm -r -- "$tasks_migration_config" ;; *) exit 1 ;; esac
rm -- /tmp/tasks-migration-atoms.registry-path
```

### Task 4: Migrate Beliefs

**Files:**

- Create: `~/d/beliefs/.worktrees/tasks-migration-beliefs/docs/plans/2026-08-30-beliefs-tasks-migration.md`
- Audit through CLI: `~/d/beliefs/.worktrees/tasks-migration-beliefs/tasks/.config.toml`
- Preserve through CLI: `~/d/beliefs/.worktrees/tasks-migration-beliefs/tasks/beliefs-c88566.md`
- Create through CLI: only additional evidence-backed `~/d/beliefs/.worktrees/tasks-migration-beliefs/tasks/beliefs-*.md`
- Modify: evidence-backed drift under `README.md` and `docs/`
- Create: `AGENTS.md` if still absent; otherwise modify it

**Interfaces:**

- Consumes: integrated Familiar and Atoms, including resolvable `fam-*` and `atoms-*` IDs.
- Produces: integrated `beliefs` tasks and explicit repository guidance.

- [ ] **Step 1: Create the Beliefs worktree and establish the kernel-sensitive baseline**

Preflight prerequisite: before creating the migration worktree, integrate the independently
reviewed one-line `python/tests/test_holdings_boundary.py` package-rename correction on Beliefs
stable `main` (`src/science/...` to `src/beliefs/...`). This is a preflight fix, not a migration
commit; the migration's prescribed two initial commits remain documentation and Tasks only.

```bash
set -euo pipefail
test -f ~/d/beliefs/python/tests/test_holdings_boundary.py
rg -q 'src/beliefs/' ~/d/beliefs/python/tests/test_holdings_boundary.py
if rg -q 'src/science/' ~/d/beliefs/python/tests/test_holdings_boundary.py; then exit 1; fi
git -C ~/d/beliefs worktree add -b chore/tasks-migration-beliefs ~/d/beliefs/.worktrees/tasks-migration-beliefs main
beliefs_worktree=~/d/beliefs/.worktrees/tasks-migration-beliefs
beliefs_python_gate=/tmp/tasks-migration-beliefs-python-gate
beliefs_failed_nodes=/tmp/tasks-migration-beliefs-python-failed-nodes
test ! -e "$beliefs_python_gate"
test ! -e "$beliefs_failed_nodes"
cat >"$beliefs_python_gate" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
mode=${1:?mode required}
worktree=${2:?worktree required}
failed_nodes=${3:?failed-node file required}
output=$(mktemp)
trap 'rm -f -- "$output" "$output.nodes"' EXIT
set +e
(
  cd "$worktree/python"
  uv run --frozen pytest --tb=short
) >"$output" 2>&1
pytest_status=$?
set -e
test "$pytest_status" -eq 1
test "$(awk '/^FAILED / { count += 1 } END { print count + 0 }' "$output")" -eq 144
test "$(awk '/^E   / { count += 1 } END { print count + 0 }' "$output")" -eq 144
test "$(awk '/^E   atoms\.core\.errors\.CapabilityUnavailable: volume configuration is not on the supplied durability allowlist: ext4 / { count += 1 } END { print count + 0 }' "$output")" -eq 144
rg -q '^144 failed, 2580 passed in .+$' "$output"
sed -n 's/^FAILED \([^ ]*\).*/\1/p' "$output" | LC_ALL=C sort -u >"$output.nodes"
test "$(wc -l <"$output.nodes")" -eq 144
case "$mode" in
  record) cp -- "$output.nodes" "$failed_nodes" ;;
  check) cmp -- "$failed_nodes" "$output.nodes" ;;
  *) exit 2 ;;
esac
EOF
chmod +x "$beliefs_python_gate"
"$beliefs_python_gate" record "$beliefs_worktree" "$beliefs_failed_nodes"
cd "$beliefs_worktree/python"
uv run --frozen ruff check .
uv run --frozen pyright
cd ../ts
npm ci
npm test
npm run typecheck
npm run check
```

Expected: on kernel `7.1.11-arch1-1`, Python records exactly `2580 passed, 144 failed`.
Every failure is `atoms.core.errors.CapabilityUnavailable`: the ext4/kernel tuple is absent
from the durability allowlist certified for kernel `7.1.10-arch1-1`. This user-authorized,
kernel-sensitive baseline is for Task 4 only. The reusable gate records its 144 failed-node
set here; every later Task 4 Python run must use `check` and match that set, the identical
root-cause signature, and `2580 passed`. Ruff, Pyright, and all three TypeScript gates remain
strict.

- [ ] **Step 2: Audit Beliefs and write its ledger**

Read the README, current roadmap and adoption ledgers, guide, active plans, code, and tests. Classify every document; verify adoption state, cut-12 claims, and unfinished work against all local branches/worktrees. Separate Beliefs outcomes from already-owned Atoms outcomes. Write `docs/plans/2026-08-30-beliefs-tasks-migration.md` with the shared contract. Its `Candidate outcomes` ledger includes a literal row for existing task `beliefs-c88566`: size, proposed status, and blockers are `n/a`; disposition is `preserve (no task mutation)`; task ID is `beliefs-c88566`. Audit its evidence and every field, retain its task ID, and neither duplicate it nor silently omit it. Create only additional evidence-backed outcomes.

- [ ] **Step 3: Correct drift, add guidance, and commit documentation**

Create `AGENTS.md` if absent with the repository's authority and gate summary. Add the shared Tasks guidance only after initialization in Step 5. Then run:

```bash
set -euo pipefail
cd ~/d/beliefs/.worktrees/tasks-migration-beliefs
audit_paths=(docs)
for f in README.md AGENTS.md CLAUDE.md; do test ! -f "$f" || audit_paths+=("$f"); done
rg -n 'Status:|roadmap|adopt|cut.?12|guide|Atoms|supersed' "${audit_paths[@]}" || { audit_rg_status=$?; test "$audit_rg_status" -eq 1; }
comm -3 \
  <({ git ls-files docs; printf '%s\n' docs/plans/2026-08-30-beliefs-tasks-migration.md; for f in README.md AGENTS.md CLAUDE.md; do test ! -f "$f" || echo "$f"; done; } | sort -u) \
  <(sed -n '/^## Document classification$/,/^## /p' docs/plans/2026-08-30-beliefs-tasks-migration.md | awk -F'`' '/^\| `/{print $2}' | sort -u)
git diff --check
beliefs_python_gate=/tmp/tasks-migration-beliefs-python-gate
beliefs_failed_nodes=/tmp/tasks-migration-beliefs-python-failed-nodes
test -x "$beliefs_python_gate"
test -f "$beliefs_failed_nodes"
"$beliefs_python_gate" check "$PWD" "$beliefs_failed_nodes"
cd python && uv run --frozen ruff check . && uv run --frozen pyright
cd ../ts && npm test && npm run typecheck && npm run check
cd ..
git add README.md AGENTS.md docs
test ! -e CLAUDE.md || git add CLAUDE.md
git diff --cached --check
git commit -m "docs: reconcile project status for tasks migration"
```

- [ ] **Step 4: Audit and idempotently seed the existing Beliefs store**

```bash
set -euo pipefail
test ! -e /tmp/tasks-migration-beliefs.registry-path
test ! -e /tmp/tasks-migration-beliefs-existing-task.json
tasks_migration_config=$(TMPDIR=/tmp mktemp -d)
printf '%s\n' "$tasks_migration_config" >/tmp/tasks-migration-beliefs.registry-path
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
tasks -C ~/d/familiar init --prefix fam
tasks -C ~/d/atoms init --prefix atoms
tasks -C ~/d/beliefs/.worktrees/tasks-migration-beliefs init --prefix beliefs
tasks -C ~/d/beliefs/.worktrees/tasks-migration-beliefs init --prefix beliefs
tasks -C ~/d/beliefs/.worktrees/tasks-migration-beliefs check >/tmp/beliefs-existing-check.json
jq -e '.errors == [] and .warnings == []' /tmp/beliefs-existing-check.json
tasks -C ~/d/beliefs/.worktrees/tasks-migration-beliefs prime | jq -e '.prefix == "beliefs"'
tasks -C ~/d/beliefs/.worktrees/tasks-migration-beliefs list --status idea --status todo --status doing --status blocked --status done --status dropped | jq -e '.tasks | length == 1 and .[0].id == "beliefs-c88566"'
tasks -C ~/d/beliefs/.worktrees/tasks-migration-beliefs show beliefs-c88566 | jq -S . >/tmp/tasks-migration-beliefs-existing-task.json
```

Expected: the CLI-created store is valid with prefix `beliefs`, zero Tasks errors and warnings,
and exactly the preserved task ID `beliefs-c88566`. The second `init` is the idempotent seed into
the temporary registry; it does not create a store or change the existing task.

- [ ] **Step 5: Create Beliefs tasks and dependencies**

Re-derive the registry before applying the shared task-creation procedure:

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-beliefs.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
cd ~/d/beliefs/.worktrees/tasks-migration-beliefs
```

Add only ledger rows marked `create` and blockers proven by the Beliefs evidence; use existing Atoms IDs where delivery truly depends on them. Do not mutate or recreate `beliefs-c88566`; after all additions, compare its complete CLI JSON record with `/tmp/tasks-migration-beliefs-existing-task.json`. Record blockers targeting Nodes or either Mindful repository as pending rather than dangling. Ensure `AGENTS.md` contains the shared guidance.

- [ ] **Step 6: Verify, commit, and independently review Beliefs**

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-beliefs.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
tasks -C ~/d/beliefs/.worktrees/tasks-migration-beliefs check >/tmp/beliefs-check.json
jq -e '.errors == [] and .warnings == []' /tmp/beliefs-check.json
tasks -C ~/d/beliefs/.worktrees/tasks-migration-beliefs prime | jq -e '.prefix == "beliefs"'
tasks -C ~/d/beliefs/.worktrees/tasks-migration-beliefs ready
tasks -C ~/d/beliefs/.worktrees/tasks-migration-beliefs show beliefs-c88566 | jq -S . | cmp -- /tmp/tasks-migration-beliefs-existing-task.json -
beliefs_python_gate=/tmp/tasks-migration-beliefs-python-gate
beliefs_failed_nodes=/tmp/tasks-migration-beliefs-python-failed-nodes
test -x "$beliefs_python_gate"
test -f "$beliefs_failed_nodes"
"$beliefs_python_gate" check ~/d/beliefs/.worktrees/tasks-migration-beliefs "$beliefs_failed_nodes"
cd ~/d/beliefs/.worktrees/tasks-migration-beliefs/python
uv run --frozen ruff check .
uv run --frozen pyright
cd ../ts
npm test
npm run typecheck
npm run check
cd ..
test ! -e tasks/projects.toml
git add tasks AGENTS.md docs/plans/2026-08-30-beliefs-tasks-migration.md
git diff --cached --check
git commit -m "chore(tasks): initialize project task tracking"
git status --short
```

Independently review the first two migration commits and fix findings before the first integration.

- [ ] **Step 7: Integrate, register, finalize, and clean up Beliefs**

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-beliefs.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
git -C ~/d/beliefs merge --ff-only chore/tasks-migration-beliefs
beliefs_python_gate=/tmp/tasks-migration-beliefs-python-gate
beliefs_failed_nodes=/tmp/tasks-migration-beliefs-python-failed-nodes
test -x "$beliefs_python_gate"
test -f "$beliefs_failed_nodes"
"$beliefs_python_gate" check ~/d/beliefs "$beliefs_failed_nodes"
cd ~/d/beliefs/python && uv run --frozen ruff check . && uv run --frozen pyright
cd ../ts && npm test && npm run typecheck && npm run check
cd ..
TASKS_FORMAT=json tasks init --prefix beliefs
TASKS_FORMAT=json tasks init --prefix beliefs
TASKS_FORMAT=json tasks check >/tmp/beliefs-stable-check.json
jq -e '.errors == [] and .warnings == []' /tmp/beliefs-stable-check.json
TASKS_FORMAT=json tasks prime | jq -e '.prefix == "beliefs"'
TASKS_FORMAT=json tasks ready
git status --short --branch
```

Apply the Shared Post-registration Ledger Finalization to
`docs/plans/2026-08-30-beliefs-tasks-migration.md`, rerunning Step 3's exact coverage
comparison. Base the ledger status and self-classification on whether any deferred row is
still pending. After its ledger-only commit passes independent review, run the second
fast-forward and clean up:

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-beliefs.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
git -C ~/d/beliefs merge --ff-only chore/tasks-migration-beliefs
git -C ~/d/beliefs status --short --branch
git worktree remove ~/d/beliefs/.worktrees/tasks-migration-beliefs
git branch -d chore/tasks-migration-beliefs
case "$tasks_migration_config" in /tmp/*) rm -r -- "$tasks_migration_config" ;; *) exit 1 ;; esac
rm -- /tmp/tasks-migration-beliefs.registry-path
rm -- /tmp/beliefs-existing-check.json /tmp/beliefs-check.json /tmp/beliefs-stable-check.json /tmp/tasks-migration-beliefs-existing-task.json /tmp/tasks-migration-beliefs-python-failed-nodes /tmp/tasks-migration-beliefs-python-gate
```

### Task 5: Migrate Nodes

**Files:**

- Create: `~/d/nodes/.worktrees/tasks-migration-nodes/docs/plans/2026-08-30-nodes-tasks-migration.md`
- Create through CLI: `~/d/nodes/.worktrees/tasks-migration-nodes/tasks/.config.toml`
- Create through CLI: `~/d/nodes/.worktrees/tasks-migration-nodes/tasks/nodes-*.md`
- Modify: evidence-backed drift under `README.md` and `docs/`
- Modify: `AGENTS.md`

**Interfaces:**

- Consumes: integrated Familiar, Atoms, and Beliefs.
- Produces: integrated `nodes` tasks and stable blocker IDs for both Mindful repositories.

- [ ] **Step 1: Create the Nodes worktree and run both baselines**

```bash
set -euo pipefail
git -C ~/d/nodes worktree add -b chore/tasks-migration-nodes ~/d/nodes/.worktrees/tasks-migration-nodes main
cd ~/d/nodes/.worktrees/tasks-migration-nodes/python
uv run --frozen pytest -q
uv run --frozen ruff check .
uv run --frozen pyright src
cd ../ts
npm ci
npm test
npm run typecheck
npm run check
```

Expected: all six gates pass before edits.

- [ ] **Step 2: Audit Nodes and write its ledger**

Read `AGENTS.md` first and treat `docs/STANDARD.md` as the authority it names. Verify Python/TypeScript parity claims, current APIs, tests, active plans, and README statements. Classify every document and express parity as one outcome when it ships as one result rather than splitting it by language. Write `docs/plans/2026-08-30-nodes-tasks-migration.md`.

- [ ] **Step 3: Correct drift, prove coverage, and commit documentation**

```bash
set -euo pipefail
cd ~/d/nodes/.worktrees/tasks-migration-nodes
audit_paths=(docs)
for f in README.md AGENTS.md CLAUDE.md; do test ! -f "$f" || audit_paths+=("$f"); done
rg -n 'Status:|STANDARD|parity|Python|TypeScript|Mindful|supersed' "${audit_paths[@]}" || { audit_rg_status=$?; test "$audit_rg_status" -eq 1; }
comm -3 \
  <({ git ls-files docs; printf '%s\n' docs/plans/2026-08-30-nodes-tasks-migration.md; for f in README.md AGENTS.md CLAUDE.md; do test ! -f "$f" || echo "$f"; done; } | sort -u) \
  <(sed -n '/^## Document classification$/,/^## /p' docs/plans/2026-08-30-nodes-tasks-migration.md | awk -F'`' '/^\| `/{print $2}' | sort -u)
git diff --check
cd python && uv run --frozen pytest -q && uv run --frozen ruff check . && uv run --frozen pyright src
cd ../ts && npm test && npm run typecheck && npm run check
cd ..
git add README.md AGENTS.md docs
test ! -e CLAUDE.md || git add CLAUDE.md
git diff --cached --check
git commit -m "docs: reconcile project status for tasks migration"
```

- [ ] **Step 4: Initialize Nodes with earlier projects resolvable**

```bash
tasks -C ~/d/nodes/.worktrees/tasks-migration-nodes prime
```

Expected: explicit `no_project` failure.

```bash
set -euo pipefail
test ! -e /tmp/tasks-migration-nodes.registry-path
tasks_migration_config=$(TMPDIR=/tmp mktemp -d)
printf '%s\n' "$tasks_migration_config" >/tmp/tasks-migration-nodes.registry-path
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
tasks -C ~/d/familiar init --prefix fam
tasks -C ~/d/atoms init --prefix atoms
tasks -C ~/d/beliefs init --prefix beliefs
tasks -C ~/d/nodes/.worktrees/tasks-migration-nodes init --prefix nodes
```

- [ ] **Step 5: Create Nodes tasks and dependencies**

Re-derive the registry before applying the shared task-creation procedure:

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-nodes.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
cd ~/d/nodes/.worktrees/tasks-migration-nodes
```

Keep cross-language delivery together when acceptance requires parity. Record verified blockers targeting either future Mindful migration as pending; do not create dangling IDs. Add the shared agent guidance.

- [ ] **Step 6: Verify, commit, and independently review Nodes**

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-nodes.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
tasks -C ~/d/nodes/.worktrees/tasks-migration-nodes check >/tmp/nodes-check.json
jq -e '.errors == [] and .warnings == []' /tmp/nodes-check.json
tasks -C ~/d/nodes/.worktrees/tasks-migration-nodes prime | jq -e '.prefix == "nodes"'
tasks -C ~/d/nodes/.worktrees/tasks-migration-nodes ready
cd ~/d/nodes/.worktrees/tasks-migration-nodes/python
uv run --frozen pytest -q
uv run --frozen ruff check .
uv run --frozen pyright src
cd ../ts
npm test
npm run typecheck
npm run check
cd ..
test ! -e tasks/projects.toml
git add tasks AGENTS.md docs/plans/2026-08-30-nodes-tasks-migration.md
git diff --cached --check
git commit -m "chore(tasks): initialize project task tracking"
git status --short
```

Independently review the first two migration commits and fix findings before the first integration.

- [ ] **Step 7: Integrate, register, finalize, and clean up Nodes**

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-nodes.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
git -C ~/d/nodes merge --ff-only chore/tasks-migration-nodes
cd ~/d/nodes/python && uv run --frozen pytest -q && uv run --frozen ruff check . && uv run --frozen pyright src
cd ../ts && npm test && npm run typecheck && npm run check
cd ~/d/nodes
TASKS_FORMAT=json tasks init --prefix nodes
TASKS_FORMAT=json tasks init --prefix nodes
TASKS_FORMAT=json tasks check >/tmp/nodes-stable-check.json
jq -e '.errors == [] and .warnings == []' /tmp/nodes-stable-check.json
TASKS_FORMAT=json tasks prime | jq -e '.prefix == "nodes"'
TASKS_FORMAT=json tasks ready
git status --short --branch
```

Apply the Shared Post-registration Ledger Finalization to
`docs/plans/2026-08-30-nodes-tasks-migration.md`, rerunning Step 3's exact coverage
comparison. Base the ledger status and self-classification on whether any deferred row is
still pending. After its ledger-only commit passes independent review, run the second
fast-forward and clean up:

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-nodes.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
git -C ~/d/nodes merge --ff-only chore/tasks-migration-nodes
git -C ~/d/nodes status --short --branch
git worktree remove ~/d/nodes/.worktrees/tasks-migration-nodes
git branch -d chore/tasks-migration-nodes
case "$tasks_migration_config" in /tmp/*) rm -r -- "$tasks_migration_config" ;; *) exit 1 ;; esac
rm -- /tmp/tasks-migration-nodes.registry-path
```

### Task 6: Migrate Mindful v3

**Files:**

- Create: `~/d/mindful/v3/.worktrees/tasks-migration-mind3/docs/plans/2026-08-30-mindful-v3-tasks-migration.md`
- Create through CLI: `~/d/mindful/v3/.worktrees/tasks-migration-mind3/tasks/.config.toml`
- Create through CLI: `~/d/mindful/v3/.worktrees/tasks-migration-mind3/tasks/mind3-*.md`
- Modify: evidence-backed drift under `README.md` and `docs/`
- Modify: `AGENTS.md`

**Interfaces:**

- Consumes: integrated Familiar, Atoms, Beliefs, and Nodes.
- Produces: integrated `mind3` tasks and stable v3 IDs for Mindful v6 dependencies.

- [ ] **Step 1: Create the v3 worktree and record service state**

```bash
set -euo pipefail
git -C ~/d/mindful/v3 worktree add -b chore/tasks-migration-mind3 ~/d/mindful/v3/.worktrees/tasks-migration-mind3 main
cd ~/d/mindful/v3/.worktrees/tasks-migration-mind3
docker compose ps
```

Record whether the required stack was already running. Start it only if the repository's existing gate requires it and it is absent; do not recreate healthy services unnecessarily.

- [ ] **Step 2: Run the v3 baseline gates**

```bash
set -euo pipefail
cd ~/d/mindful/v3/.worktrees/tasks-migration-mind3/react
npm ci
npm run typecheck
npm run lint
npm run test
npm run build
cd ..
docker exec mindful_fastapi_v3 uv run pytest tests -v
docker exec mindful_fastapi_v3 uv run pytest tests/test_graph_producers.py -v
docker exec mindful_fastapi_v3 uv run ruff check .
```

Expected: all frontend and backend gates pass before edits.

- [ ] **Step 3: Audit Mindful v3 and write its ledger**

Read `AGENTS.md`, README, current roadmap and active plans, architecture-modernization documents, v3/v6 boundary documents, code, and tests. Classify all documents. Treat unchecked historical plans as claims, not work; create no task unless present evidence proves an unfinished outcome. Write `docs/plans/2026-08-30-mindful-v3-tasks-migration.md`.

- [ ] **Step 4: Correct drift, prove coverage, and commit documentation**

```bash
set -euo pipefail
cd ~/d/mindful/v3/.worktrees/tasks-migration-mind3
audit_paths=(docs)
for f in README.md AGENTS.md CLAUDE.md; do test ! -f "$f" || audit_paths+=("$f"); done
rg -n 'Status:|roadmap|moderni[sz]|v3|v6|import|supersed' "${audit_paths[@]}" || { audit_rg_status=$?; test "$audit_rg_status" -eq 1; }
comm -3 \
  <({ git ls-files docs; printf '%s\n' docs/plans/2026-08-30-mindful-v3-tasks-migration.md; for f in README.md AGENTS.md CLAUDE.md; do test ! -f "$f" || echo "$f"; done; } | sort -u) \
  <(sed -n '/^## Document classification$/,/^## /p' docs/plans/2026-08-30-mindful-v3-tasks-migration.md | awk -F'`' '/^\| `/{print $2}' | sort -u)
git diff --check
cd react && npm run typecheck && npm run lint && npm run test && npm run build
cd ..
docker exec mindful_fastapi_v3 uv run pytest tests -v
docker exec mindful_fastapi_v3 uv run pytest tests/test_graph_producers.py -v
docker exec mindful_fastapi_v3 uv run ruff check .
git add README.md AGENTS.md docs
test ! -e CLAUDE.md || git add CLAUDE.md
git diff --cached --check
git commit -m "docs: reconcile project status for tasks migration"
```

- [ ] **Step 5: Initialize v3 with earlier projects resolvable**

```bash
tasks -C ~/d/mindful/v3/.worktrees/tasks-migration-mind3 prime
```

Expected: explicit `no_project` failure.

```bash
set -euo pipefail
test ! -e /tmp/tasks-migration-mind3.registry-path
tasks_migration_config=$(TMPDIR=/tmp mktemp -d)
printf '%s\n' "$tasks_migration_config" >/tmp/tasks-migration-mind3.registry-path
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
tasks -C ~/d/familiar init --prefix fam
tasks -C ~/d/atoms init --prefix atoms
tasks -C ~/d/beliefs init --prefix beliefs
tasks -C ~/d/nodes init --prefix nodes
tasks -C ~/d/mindful/v3/.worktrees/tasks-migration-mind3 init --prefix mind3
```

- [ ] **Step 6: Create v3 tasks and dependencies**

Re-derive the registry before applying the shared task-creation procedure:

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-mind3.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
cd ~/d/mindful/v3/.worktrees/tasks-migration-mind3
```

Mark a task `doing` only with a verified owner token; never treat an old branch alone as ownership. Add direct Nodes blockers when proven. Record blockers targeting Mindful v6 as pending. Add the shared agent guidance.

- [ ] **Step 7: Verify, commit, and independently review v3**

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-mind3.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
tasks -C ~/d/mindful/v3/.worktrees/tasks-migration-mind3 check >/tmp/mind3-check.json
jq -e '.errors == [] and .warnings == []' /tmp/mind3-check.json
tasks -C ~/d/mindful/v3/.worktrees/tasks-migration-mind3 prime | jq -e '.prefix == "mind3"'
tasks -C ~/d/mindful/v3/.worktrees/tasks-migration-mind3 ready
cd ~/d/mindful/v3/.worktrees/tasks-migration-mind3/react
npm run typecheck
npm run lint
npm run test
npm run build
cd ..
docker exec mindful_fastapi_v3 uv run pytest tests -v
docker exec mindful_fastapi_v3 uv run pytest tests/test_graph_producers.py -v
docker exec mindful_fastapi_v3 uv run ruff check .
test ! -e tasks/projects.toml
git add tasks AGENTS.md docs/plans/2026-08-30-mindful-v3-tasks-migration.md
git diff --cached --check
git commit -m "chore(tasks): initialize project task tracking"
git status --short
```

Independently review both initial migration commits and fix findings before the first integration.

- [ ] **Step 8: Integrate, register, finalize, and clean up v3**

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-mind3.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
git -C ~/d/mindful/v3 merge --ff-only chore/tasks-migration-mind3
cd ~/d/mindful/v3/react && npm run typecheck && npm run lint && npm run test && npm run build
cd ..
docker exec mindful_fastapi_v3 uv run pytest tests -v
docker exec mindful_fastapi_v3 uv run pytest tests/test_graph_producers.py -v
docker exec mindful_fastapi_v3 uv run ruff check .
TASKS_FORMAT=json tasks init --prefix mind3
TASKS_FORMAT=json tasks init --prefix mind3
TASKS_FORMAT=json tasks check >/tmp/mind3-stable-check.json
jq -e '.errors == [] and .warnings == []' /tmp/mind3-stable-check.json
TASKS_FORMAT=json tasks prime | jq -e '.prefix == "mind3"'
TASKS_FORMAT=json tasks ready
git status --short --branch
```

Apply the Shared Post-registration Ledger Finalization to
`docs/plans/2026-08-30-mindful-v3-tasks-migration.md`, rerunning Step 4's exact coverage
comparison. Base the ledger status and self-classification on whether any deferred row is
still pending. After its ledger-only commit passes independent review, run the second
fast-forward and clean up:

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-mind3.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
git -C ~/d/mindful/v3 merge --ff-only chore/tasks-migration-mind3
git -C ~/d/mindful/v3 status --short --branch
git worktree remove ~/d/mindful/v3/.worktrees/tasks-migration-mind3
git branch -d chore/tasks-migration-mind3
case "$tasks_migration_config" in /tmp/*) rm -r -- "$tasks_migration_config" ;; *) exit 1 ;; esac
rm -- /tmp/tasks-migration-mind3.registry-path
```

If Step 1 started the stack, stop it with the repository's normal `docker compose down` command without `--volumes`. Do not stop a stack that was already running.

### Task 7: Migrate Mindful v6

**Files:**

- Create: `~/d/mindful/v6/.worktrees/tasks-migration-mind6/docs/plans/2026-08-30-mindful-v6-tasks-migration.md`
- Create through CLI: `~/d/mindful/v6/.worktrees/tasks-migration-mind6/tasks/.config.toml`
- Create through CLI: `~/d/mindful/v6/.worktrees/tasks-migration-mind6/tasks/mind6-*.md`
- Modify: evidence-backed drift under `README.md` and `docs/`
- Modify: `AGENTS.md`

**Interfaces:**

- Consumes: all five previously integrated stores, especially `nodes-*` and `mind3-*` IDs.
- Produces: the sixth integrated Tasks store and a complete set of IDs for reconciliation.

- [ ] **Step 1: Create the v6 worktree and run its baseline**

```bash
set -euo pipefail
git -C ~/d/mindful/v6 worktree add -b chore/tasks-migration-mind6 ~/d/mindful/v6/.worktrees/tasks-migration-mind6 main
cd ~/d/mindful/v6/.worktrees/tasks-migration-mind6
npm ci
npm test
npm run typecheck
npm run check
```

If a gate exits 2 solely because the Nodes core build is stale, run `npm run build` in `~/d/nodes/ts`, rerun all three v6 gates, and record that prerequisite in the ledger. Treat every other failure as a baseline failure.

- [ ] **Step 2: Audit Mindful v6 and write its ledger**

Read `AGENTS.md`, `docs/ARCHITECTURE.md`, and `docs/FORMATS.md` first as standing authority. Classify all documents; verify the v3 import boundary, Nodes integration, sprint/history claims, and all active plans against code/tests/current Git state. Write `docs/plans/2026-08-30-mindful-v6-tasks-migration.md`.

- [ ] **Step 3: Correct drift, prove coverage, and commit documentation**

```bash
set -euo pipefail
cd ~/d/mindful/v6/.worktrees/tasks-migration-mind6
audit_paths=(docs)
for f in README.md AGENTS.md CLAUDE.md; do test ! -f "$f" || audit_paths+=("$f"); done
rg -n 'Status:|ARCHITECTURE|FORMATS|v3|v6|Nodes|sprint|import|supersed' "${audit_paths[@]}" || { audit_rg_status=$?; test "$audit_rg_status" -eq 1; }
comm -3 \
  <({ git ls-files docs; printf '%s\n' docs/plans/2026-08-30-mindful-v6-tasks-migration.md; for f in README.md AGENTS.md CLAUDE.md; do test ! -f "$f" || echo "$f"; done; } | sort -u) \
  <(sed -n '/^## Document classification$/,/^## /p' docs/plans/2026-08-30-mindful-v6-tasks-migration.md | awk -F'`' '/^\| `/{print $2}' | sort -u)
git diff --check
npm test
npm run typecheck
npm run check
git add README.md AGENTS.md docs
test ! -e CLAUDE.md || git add CLAUDE.md
git diff --cached --check
git commit -m "docs: reconcile project status for tasks migration"
```

- [ ] **Step 4: Initialize v6 with all earlier projects resolvable**

```bash
tasks -C ~/d/mindful/v6/.worktrees/tasks-migration-mind6 prime
```

Expected: explicit `no_project` failure.

```bash
set -euo pipefail
test ! -e /tmp/tasks-migration-mind6.registry-path
tasks_migration_config=$(TMPDIR=/tmp mktemp -d)
printf '%s\n' "$tasks_migration_config" >/tmp/tasks-migration-mind6.registry-path
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
tasks -C ~/d/familiar init --prefix fam
tasks -C ~/d/atoms init --prefix atoms
tasks -C ~/d/beliefs init --prefix beliefs
tasks -C ~/d/nodes init --prefix nodes
tasks -C ~/d/mindful/v3 init --prefix mind3
tasks -C ~/d/mindful/v6/.worktrees/tasks-migration-mind6 init --prefix mind6
```

- [ ] **Step 5: Create v6 tasks and dependencies**

Re-derive the registry before applying the shared task-creation procedure:

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-mind6.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
cd ~/d/mindful/v6/.worktrees/tasks-migration-mind6
```

Add direct dependencies on existing Nodes and Mindful v3 tasks only when they block delivery. Any dependency targeting earlier projects is now resolvable; treat failure to resolve as a registry or evidence error rather than omitting it. Add the shared agent guidance.

- [ ] **Step 6: Verify, commit, and independently review v6**

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-mind6.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
tasks -C ~/d/mindful/v6/.worktrees/tasks-migration-mind6 check >/tmp/mind6-check.json
jq -e '.errors == [] and .warnings == []' /tmp/mind6-check.json
tasks -C ~/d/mindful/v6/.worktrees/tasks-migration-mind6 prime | jq -e '.prefix == "mind6"'
tasks -C ~/d/mindful/v6/.worktrees/tasks-migration-mind6 ready
cd ~/d/mindful/v6/.worktrees/tasks-migration-mind6
npm test
npm run typecheck
npm run check
test ! -e tasks/projects.toml
git add tasks AGENTS.md docs/plans/2026-08-30-mindful-v6-tasks-migration.md
git diff --cached --check
git commit -m "chore(tasks): initialize project task tracking"
git status --short
```

Independently review both initial migration commits and fix findings before the first integration.

- [ ] **Step 7: Integrate, register, finalize, and clean up v6**

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-mind6.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
git -C ~/d/mindful/v6 merge --ff-only chore/tasks-migration-mind6
cd ~/d/mindful/v6
npm test
npm run typecheck
npm run check
TASKS_FORMAT=json tasks init --prefix mind6
TASKS_FORMAT=json tasks init --prefix mind6
TASKS_FORMAT=json tasks check >/tmp/mind6-stable-check.json
jq -e '.errors == [] and .warnings == []' /tmp/mind6-stable-check.json
TASKS_FORMAT=json tasks prime | jq -e '.prefix == "mind6"'
TASKS_FORMAT=json tasks ready
git status --short --branch
```

Apply the Shared Post-registration Ledger Finalization to
`docs/plans/2026-08-30-mindful-v6-tasks-migration.md`, rerunning Step 3's exact coverage
comparison. Base the ledger status and self-classification on whether any deferred row is
still pending. After its ledger-only commit passes independent review, run the second
fast-forward and clean up:

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-mind6.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
git -C ~/d/mindful/v6 merge --ff-only chore/tasks-migration-mind6
git -C ~/d/mindful/v6 status --short --branch
git worktree remove ~/d/mindful/v6/.worktrees/tasks-migration-mind6
git branch -d chore/tasks-migration-mind6
case "$tasks_migration_config" in /tmp/*) rm -r -- "$tasks_migration_config" ;; *) exit 1 ;; esac
rm -- /tmp/tasks-migration-mind6.registry-path
```

### Task 8: Reconcile Deferred Cross-Project Dependencies

**Files:**

- Modify through CLI, only when pending rows exist: one or more of `tasks/fam-*.md`, `tasks/atoms-*.md`, `tasks/beliefs-*.md`, `tasks/nodes-*.md`, `tasks/mind3-*.md`, and `tasks/mind6-*.md` in the affected reconciliation worktree
- Modify, only when pending rows exist: the affected repository's `docs/plans/2026-08-30-*-tasks-migration.md`, including its final status and self-classification

**Interfaces:**

- Consumes: all six integrated task stores and every ledger's `Deferred foreign dependencies` table.
- Produces: committed dependency edges and a complete `historical/superseded` ledger for every affected repository, or a verified no-op when no row is pending.

- [ ] **Step 1: Build the exact six-project portfolio registry through the CLI**

```bash
set -euo pipefail
test ! -e /tmp/tasks-reconciliation-portfolio.registry-path
tasks_portfolio_config=$(TMPDIR=/tmp mktemp -d)
printf '%s\n' "$tasks_portfolio_config" >/tmp/tasks-reconciliation-portfolio.registry-path
export XDG_CONFIG_HOME="${tasks_portfolio_config:?portfolio registry unset}"
export TASKS_FORMAT=json
tasks -C ~/d/familiar init --prefix fam
tasks -C ~/d/atoms init --prefix atoms
tasks -C ~/d/beliefs init --prefix beliefs
tasks -C ~/d/nodes init --prefix nodes
tasks -C ~/d/mindful/v3 init --prefix mind3
tasks -C ~/d/mindful/v6 init --prefix mind6
```

Expected: six successful `init` calls. Do not write `projects.toml` directly.

- [ ] **Step 2: Enumerate pending rows exactly**

```bash
set -euo pipefail
for ledger in \
  ~/d/familiar/docs/plans/2026-08-30-familiar-tasks-migration.md \
  ~/d/atoms/docs/plans/2026-08-30-atoms-tasks-migration.md \
  ~/d/beliefs/docs/plans/2026-08-30-beliefs-tasks-migration.md \
  ~/d/nodes/docs/plans/2026-08-30-nodes-tasks-migration.md \
  ~/d/mindful/v3/docs/plans/2026-08-30-mindful-v3-tasks-migration.md \
  ~/d/mindful/v6/docs/plans/2026-08-30-mindful-v6-tasks-migration.md
do
  pending_rows=$(sed -n '/^## Deferred foreign dependencies$/,/^## /p' "$ledger" | rg '^\|.*\| pending \|$' || true)
  if test -n "$pending_rows"; then
    printf '%s\nPENDING_IN %s\n' "$pending_rows" "$ledger"
  fi
done
```

If no row prints, record reconciliation as a verified no-op and continue to Step 8. Otherwise process the printed repositories one at a time in portfolio order.

- [ ] **Step 3: Create the reusable reconciliation helper**

Use `apply_patch` to create `/tmp/tasks-reconciliation-helper.zsh` with exactly this content:

```zsh
load_reconciliation_target() {
  case "${prefix:?reconciliation prefix unset}" in
    fam)
      repo="$HOME/d/familiar"
      worktree="$repo/.worktrees/tasks-reconciliation-fam"
      ledger_rel=docs/plans/2026-08-30-familiar-tasks-migration.md
      ;;
    atoms)
      repo="$HOME/d/atoms"
      worktree="$repo/.worktrees/tasks-reconciliation-atoms"
      ledger_rel=docs/plans/2026-08-30-atoms-tasks-migration.md
      ;;
    beliefs)
      repo="$HOME/d/beliefs"
      worktree="$repo/.worktrees/tasks-reconciliation-beliefs"
      ledger_rel=docs/plans/2026-08-30-beliefs-tasks-migration.md
      ;;
    nodes)
      repo="$HOME/d/nodes"
      worktree="$repo/.worktrees/tasks-reconciliation-nodes"
      ledger_rel=docs/plans/2026-08-30-nodes-tasks-migration.md
      ;;
    mind3)
      repo="$HOME/d/mindful/v3"
      worktree="$repo/.worktrees/tasks-reconciliation-mind3"
      ledger_rel=docs/plans/2026-08-30-mindful-v3-tasks-migration.md
      ;;
    mind6)
      repo="$HOME/d/mindful/v6"
      worktree="$repo/.worktrees/tasks-reconciliation-mind6"
      ledger_rel=docs/plans/2026-08-30-mindful-v6-tasks-migration.md
      ;;
    *) return 2 ;;
  esac
}

run_reconciliation_gate() {
  case "$1" in
    fam)
      (cd "$2" && npm test)
      ;;
    atoms)
      (cd "$2/python" && uv run pytest && uv run ruff check . && uv run pyright)
      ;;
    beliefs)
      (cd "$2/python" && uv run --frozen pytest -q && uv run --frozen ruff check . && uv run --frozen pyright) &&
      (cd "$2/ts" && npm test && npm run typecheck && npm run check)
      ;;
    nodes)
      (cd "$2/python" && uv run --frozen pytest -q && uv run --frozen ruff check . && uv run --frozen pyright src) &&
      (cd "$2/ts" && npm test && npm run typecheck && npm run check)
      ;;
    mind3)
      (cd "$2/react" && npm run typecheck && npm run lint && npm run test && npm run build) &&
      docker exec mindful_fastapi_v3 uv run pytest tests -v &&
      docker exec mindful_fastapi_v3 uv run pytest tests/test_graph_producers.py -v &&
      docker exec mindful_fastapi_v3 uv run ruff check .
      ;;
    mind6)
      (cd "$2" && npm test && npm run typecheck && npm run check)
      ;;
    *) return 2 ;;
  esac
}
```

Run `zsh -n /tmp/tasks-reconciliation-helper.zsh`. Expected: syntax validation succeeds. Every later reconciliation step sources this file, re-reads the selected prefix and portfolio registry, and re-derives `repo`, `worktree`, and `ledger_rel`.

- [ ] **Step 4: Reconcile one affected repository in a fresh worktree**

Resolve the repository, prefix, ledger, and worktree from this fixed map:

| Repository | Prefix | Reconciliation worktree | Ledger |
|------------|--------|-------------------------|--------|
| `~/d/familiar` | `fam` | `~/d/familiar/.worktrees/tasks-reconciliation-fam` | `docs/plans/2026-08-30-familiar-tasks-migration.md` |
| `~/d/atoms` | `atoms` | `~/d/atoms/.worktrees/tasks-reconciliation-atoms` | `docs/plans/2026-08-30-atoms-tasks-migration.md` |
| `~/d/beliefs` | `beliefs` | `~/d/beliefs/.worktrees/tasks-reconciliation-beliefs` | `docs/plans/2026-08-30-beliefs-tasks-migration.md` |
| `~/d/nodes` | `nodes` | `~/d/nodes/.worktrees/tasks-reconciliation-nodes` | `docs/plans/2026-08-30-nodes-tasks-migration.md` |
| `~/d/mindful/v3` | `mind3` | `~/d/mindful/v3/.worktrees/tasks-reconciliation-mind3` | `docs/plans/2026-08-30-mindful-v3-tasks-migration.md` |
| `~/d/mindful/v6` | `mind6` | `~/d/mindful/v6/.worktrees/tasks-reconciliation-mind6` | `docs/plans/2026-08-30-mindful-v6-tasks-migration.md` |

Use `apply_patch` to create `/tmp/tasks-reconciliation-current.prefix` containing exactly one literal prefix from the affected map: `fam`, `atoms`, `beliefs`, `nodes`, `mind3`, or `mind6`, followed by a newline. Then execute:

```bash
set -euo pipefail
source /tmp/tasks-reconciliation-helper.zsh
prefix=$(</tmp/tasks-reconciliation-current.prefix)
load_reconciliation_target
git -C "$repo" worktree add -b "chore/tasks-reconciliation-$prefix" "$worktree" main
case "$prefix" in
  fam|mind6) (cd "$worktree" && npm ci) ;;
  beliefs|nodes) (cd "$worktree/ts" && npm ci) ;;
  mind3) (cd "$worktree/react" && npm ci) ;;
  atoms) ;;
  *) exit 2 ;;
esac
run_reconciliation_gate "$prefix" "$worktree"
```

Expected: the fresh worktree baseline passes. Do not create the next reconciliation worktree until this one is merged and removed.

- [ ] **Step 5: Add every pending edge through the CLI**

For each pending row in that ledger, use `apply_patch` to create `/tmp/tasks-reconciliation-edge.zsh` with exactly two literal assignments, `local_id=` and `foreign_id=`, copied from that row. Source the assignments in the same invocation that mutates the task:

```bash
set -euo pipefail
source /tmp/tasks-reconciliation-helper.zsh
source /tmp/tasks-reconciliation-edge.zsh
prefix=$(</tmp/tasks-reconciliation-current.prefix)
load_reconciliation_target
tasks_portfolio_config=$(</tmp/tasks-reconciliation-portfolio.registry-path)
test -d "${tasks_portfolio_config:?portfolio registry unset}"
export XDG_CONFIG_HOME="${tasks_portfolio_config:?portfolio registry unset}"
export TASKS_FORMAT=json
test -n "${local_id:?literal local task ID unset}"
test -n "${foreign_id:?literal foreign task ID unset}"
TASKS_OWNER=migration tasks -C "$worktree" dep "$local_id" --on "$foreign_id"
tasks -C "$worktree" show "$local_id"
rm -- /tmp/tasks-reconciliation-edge.zsh
```

If the edge fails resolution or acyclicity, stop; do not weaken or omit it. Use `apply_patch` to change that exact row from `pending` to `reconciled` and record the new edge. After the repository's final pending row is reconciled, also mark its migration complete, change the ledger's own classification from `active delivery` to `historical/superseded`, and record the reconciliation verification in the existing `Verification` section.

- [ ] **Step 6: Verify and commit the affected reconciliation**

```bash
set -euo pipefail
source /tmp/tasks-reconciliation-helper.zsh
prefix=$(</tmp/tasks-reconciliation-current.prefix)
load_reconciliation_target
tasks_portfolio_config=$(</tmp/tasks-reconciliation-portfolio.registry-path)
test -d "${tasks_portfolio_config:?portfolio registry unset}"
export XDG_CONFIG_HOME="${tasks_portfolio_config:?portfolio registry unset}"
export TASKS_FORMAT=json
tasks -C "$worktree" check >/tmp/reconciliation-check.json
jq -e '.errors == [] and .warnings == []' /tmp/reconciliation-check.json
run_reconciliation_gate "$prefix" "$worktree"
git -C "$worktree" diff --check
git -C "$worktree" add tasks "$ledger_rel"
git -C "$worktree" diff --cached --check
git -C "$worktree" commit -m "chore(tasks): reconcile cross-project dependencies"
git -C "$worktree" status --short
```

Expected: one commit contains the CLI-generated dependency edit, every reconciled ledger row, and the ledger's truthful complete/historical finalization.

- [ ] **Step 7: Review, integrate, and remove the affected reconciliation**

Independently review the edge evidence, full reachable graph, ledger update, and commit diff. Fix findings and rerun Step 6, then:

```bash
set -euo pipefail
source /tmp/tasks-reconciliation-helper.zsh
prefix=$(</tmp/tasks-reconciliation-current.prefix)
load_reconciliation_target
tasks_portfolio_config=$(</tmp/tasks-reconciliation-portfolio.registry-path)
test -d "${tasks_portfolio_config:?portfolio registry unset}"
export XDG_CONFIG_HOME="${tasks_portfolio_config:?portfolio registry unset}"
export TASKS_FORMAT=json
git -C "$repo" merge --ff-only "chore/tasks-reconciliation-$prefix"
run_reconciliation_gate "$prefix" "$repo"
tasks -C "$repo" check >/tmp/reconciliation-stable-check.json
jq -e '.errors == [] and .warnings == []' /tmp/reconciliation-stable-check.json
cd "$repo"
git worktree remove "$worktree"
git branch -d "chore/tasks-reconciliation-$prefix"
```

Repeat Steps 4–7 for the next affected repository only after this stable check passes.

- [ ] **Step 8: Prove reconciliation is complete**

Rerun Step 2. Expected: no pending row prints. Verify every migration ledger's own classification row is `historical/superseded` and every status says the migration is complete. Then clean up the reconciliation controls:

```bash
set -euo pipefail
tasks_portfolio_config=$(</tmp/tasks-reconciliation-portfolio.registry-path)
test -d "${tasks_portfolio_config:?portfolio registry unset}"
case "$tasks_portfolio_config" in /tmp/*) rm -r -- "$tasks_portfolio_config" ;; *) exit 1 ;; esac
rm -- /tmp/tasks-reconciliation-portfolio.registry-path
for control_path in /tmp/tasks-reconciliation-helper.zsh /tmp/tasks-reconciliation-current.prefix /tmp/tasks-reconciliation-edge.zsh; do
  test ! -e "$control_path" || rm -- "$control_path"
done
```

### Task 9: Run the Portfolio Gate and Record Completion

**Files:**

- Modify: `~/d/tasks/.worktrees/project-migration-complete/docs/specs/2026-08-30-project-tasks-migration-design.md`
- Modify: `~/d/tasks/.worktrees/project-migration-complete/docs/plans/2026-08-30-project-tasks-migration.md`
- Inspect: all six stable ledgers, task stores, agent guidance files, and repository gates

**Interfaces:**

- Consumes: six integrated repositories with no pending reconciliation row.
- Produces: the final six-project verification record and truthful completion status in the Tasks design and plan.

- [ ] **Step 1: Create a fresh exact portfolio registry**

```bash
set -euo pipefail
test ! -e /tmp/tasks-final-portfolio.registry-path
tasks_portfolio_config=$(TMPDIR=/tmp mktemp -d)
printf '%s\n' "$tasks_portfolio_config" >/tmp/tasks-final-portfolio.registry-path
export XDG_CONFIG_HOME="${tasks_portfolio_config:?portfolio registry unset}"
export TASKS_FORMAT=json
tasks -C ~/d/familiar init --prefix fam
tasks -C ~/d/atoms init --prefix atoms
tasks -C ~/d/beliefs init --prefix beliefs
tasks -C ~/d/nodes init --prefix nodes
tasks -C ~/d/mindful/v3 init --prefix mind3
tasks -C ~/d/mindful/v6 init --prefix mind6
```

Expected: all six initializations succeed; unrelated registry projects are absent.

- [ ] **Step 2: Require empty checks and exact prefixes in all six projects**

```bash
set -euo pipefail
tasks_portfolio_config=$(</tmp/tasks-final-portfolio.registry-path)
test -d "${tasks_portfolio_config:?portfolio registry unset}"
export XDG_CONFIG_HOME="${tasks_portfolio_config:?portfolio registry unset}"
export TASKS_FORMAT=json
printf '%s %s\n' \
  "$HOME/d/familiar" fam \
  "$HOME/d/atoms" atoms \
  "$HOME/d/beliefs" beliefs \
  "$HOME/d/nodes" nodes \
  "$HOME/d/mindful/v3" mind3 \
  "$HOME/d/mindful/v6" mind6 |
while read -r root prefix; do
  tasks -C "$root" check >"/tmp/$prefix-portfolio-check.json"
  jq -e '.errors == [] and .warnings == []' "/tmp/$prefix-portfolio-check.json"
  tasks -C "$root" prime | jq -e --arg prefix "$prefix" '.prefix == $prefix'
  tasks -C "$root" ready
done
```

Expected: every command passes and every check has zero warnings. The literal paths above are command input only; do not copy them into committed documentation.

- [ ] **Step 3: Require strict global listing success without warnings**

```bash
set -euo pipefail
tasks_portfolio_config=$(</tmp/tasks-final-portfolio.registry-path)
test -d "${tasks_portfolio_config:?portfolio registry unset}"
export XDG_CONFIG_HOME="${tasks_portfolio_config:?portfolio registry unset}"
export TASKS_FORMAT=json
tasks -C ~/d/familiar list --all-projects >/tmp/tasks-portfolio-list.json 2>/tmp/tasks-portfolio-list.err
test ! -s /tmp/tasks-portfolio-list.err
jq -e '.warnings == [] and (.tasks | type == "array")' /tmp/tasks-portfolio-list.json
```

Expected: strict scans of all six projects succeed and stderr is empty.

- [ ] **Step 4: Audit ledgers and task semantics**

For each ledger, rerun its exact coverage comparison. Then verify:

```bash
set -euo pipefail
! rg -n '^\|.*\| pending \|$' \
  ~/d/familiar/docs/plans/2026-08-30-familiar-tasks-migration.md \
  ~/d/atoms/docs/plans/2026-08-30-atoms-tasks-migration.md \
  ~/d/beliefs/docs/plans/2026-08-30-beliefs-tasks-migration.md \
  ~/d/nodes/docs/plans/2026-08-30-nodes-tasks-migration.md \
  ~/d/mindful/v3/docs/plans/2026-08-30-mindful-v3-tasks-migration.md \
  ~/d/mindful/v6/docs/plans/2026-08-30-mindful-v6-tasks-migration.md
```

Expected: no pending row. Inspect every ledger's own classification and require `historical/superseded`. Inspect every `Candidate outcomes` row against its task ID or explicit no-task disposition; confirm completed history was not backfilled, every unresolved claim is an `idea`, every `doing` owner has evidence, and every foreign edge is a real blocker.

- [ ] **Step 5: Run every repository's complete stable gate**

```bash
set -euo pipefail
(cd ~/d/familiar && npm test)
(cd ~/d/atoms/python && uv run pytest && uv run ruff check . && uv run pyright)
(cd ~/d/beliefs/python && uv run --frozen pytest -q && uv run --frozen ruff check . && uv run --frozen pyright)
(cd ~/d/beliefs/ts && npm test && npm run typecheck && npm run check)
(cd ~/d/nodes/python && uv run --frozen pytest -q && uv run --frozen ruff check . && uv run --frozen pyright src)
(cd ~/d/nodes/ts && npm test && npm run typecheck && npm run check)
(cd ~/d/mindful/v3/react && npm run typecheck && npm run lint && npm run test && npm run build)
docker exec mindful_fastapi_v3 uv run pytest tests -v
docker exec mindful_fastapi_v3 uv run pytest tests/test_graph_producers.py -v
docker exec mindful_fastapi_v3 uv run ruff check .
(cd ~/d/mindful/v6 && npm test && npm run typecheck && npm run check)
```

Expected: every established gate passes. Apply the documented Nodes build refresh only for Mindful v6's known stale-core exit 2.

- [ ] **Step 6: Prove and, if necessary, explicitly repair normal registry mappings**

```bash
set -euo pipefail
final_portfolio_config=$(</tmp/tasks-final-portfolio.registry-path)
test "${XDG_CONFIG_HOME-}" != "${final_portfolio_config:?portfolio registry unset}"
TASKS_FORMAT=json tasks -C ~/d/familiar init --prefix fam && TASKS_FORMAT=json tasks -C ~/d/familiar prime | jq -e '.prefix == "fam"'
TASKS_FORMAT=json tasks -C ~/d/atoms init --prefix atoms && TASKS_FORMAT=json tasks -C ~/d/atoms prime | jq -e '.prefix == "atoms"'
TASKS_FORMAT=json tasks -C ~/d/beliefs init --prefix beliefs && TASKS_FORMAT=json tasks -C ~/d/beliefs prime | jq -e '.prefix == "beliefs"'
TASKS_FORMAT=json tasks -C ~/d/nodes init --prefix nodes && TASKS_FORMAT=json tasks -C ~/d/nodes prime | jq -e '.prefix == "nodes"'
TASKS_FORMAT=json tasks -C ~/d/mindful/v3 init --prefix mind3 && TASKS_FORMAT=json tasks -C ~/d/mindful/v3 prime | jq -e '.prefix == "mind3"'
TASKS_FORMAT=json tasks -C ~/d/mindful/v6 init --prefix mind6 && TASKS_FORMAT=json tasks -C ~/d/mindful/v6 prime | jq -e '.prefix == "mind6"'
```

Expected: every repeated init succeeds and every prefix is exact. If a canonical root moved, stop and back up the normal registry:

```bash
set -euo pipefail
tasks_registry_home=${XDG_CONFIG_HOME:-"$HOME/.config"}
tasks_registry_path="$tasks_registry_home/tasks/projects.toml"
test ! -e "$tasks_registry_path.pre-migration-repair"
cp -- "$tasks_registry_path" "$tasks_registry_path.pre-migration-repair"
```

Use `apply_patch` to remove only the stale entries among `fam`, `atoms`, `beliefs`, `nodes`, `mind3`, and `mind6`; preserve every unrelated entry. Then rerun the six CLI commands above and repeat Steps 1–3. This is the explicit normal-registry recovery exception; `tasks init` must never silently rebind a prefix.

- [ ] **Step 7: Create the Tasks completion worktree**

```bash
set -euo pipefail
git -C ~/d/tasks worktree add -b docs/project-migration-complete ~/d/tasks/.worktrees/project-migration-complete main
cd ~/d/tasks/.worktrees/project-migration-complete
```

Expected: a clean worktree based on the then-current `main`, which must already contain the approved design and this plan.

- [ ] **Step 8: Make completion claims truthful**

Use `apply_patch` to change the design status from implementation in progress to implemented with the actual date from `date +%F`. Check only plan boxes whose command and evidence were actually verified. Add concise final commit references or verification results where the ledger contract requires them. Then search outward for stale migration claims:

```bash
set -euo pipefail
cd ~/d/tasks/.worktrees/project-migration-complete
audit_paths=(docs)
for f in README.md AGENTS.md CLAUDE.md; do test ! -f "$f" || audit_paths+=("$f"); done
rg -n 'project.tasks.migration|not implemented|implementation planning|migration.*pending' "${audit_paths[@]}" || { audit_rg_status=$?; test "$audit_rg_status" -eq 1; }
git diff --check
```

Correct propagated user-facing drift in the same change. Do not check a box based only on an earlier unchecked or checked claim.

- [ ] **Step 9: Verify, commit, review, and integrate the completion record**

```bash
set -euo pipefail
cd ~/d/tasks/.worktrees/project-migration-complete
cargo test
git add docs
for f in README.md AGENTS.md CLAUDE.md; do test ! -e "$f" || git add "$f"; done
git diff --cached --check
git commit -m "docs: record project task migrations complete"
git status --short
```

Omit nonexistent pathspecs. Independently review the completion criteria against all six stable trees and the portfolio outputs. Fix any finding and rerun the affected gate before integration:

```bash
set -euo pipefail
tasks_portfolio_config=$(</tmp/tasks-final-portfolio.registry-path)
test -d "${tasks_portfolio_config:?portfolio registry unset}"
git -C ~/d/tasks merge --ff-only docs/project-migration-complete
cd ~/d/tasks
cargo test
git status --short --branch
git worktree remove ~/d/tasks/.worktrees/project-migration-complete
git branch -d docs/project-migration-complete
case "$tasks_portfolio_config" in /tmp/*) rm -r -- "$tasks_portfolio_config" ;; *) exit 1 ;; esac
rm -- /tmp/tasks-final-portfolio.registry-path
```

Expected: Tasks `main` records the implemented design, the plan reflects only verified history, and the complete Tasks suite passes with no failures.
