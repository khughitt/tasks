# Existing-project Tasks Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Audit Familiar, Atoms, Science, Nodes, Mindful v3, and Mindful v6 against their repositories, correct current documentation drift, and create forward-only Tasks stores for every evidence-backed remaining outcome.

**Architecture:** Migrate one repository at a time in a fresh worktree, landing a documentation reconciliation commit before a Tasks initialization commit. Use CLI-built temporary registries for migration and portfolio validation, integrate each repository before beginning the next, then reconcile only the cross-project dependencies that had to be deferred.

**Tech Stack:** Rust `tasks` CLI, Git worktrees, Markdown, TOML, POSIX shell, `jq`, npm, uv, and Docker Compose.

**Spec:** `docs/specs/2026-08-30-project-tasks-migration-design.md`

## Global Constraints

- Migrate in this order: Familiar (`fam`), Atoms (`atoms`), Science (`sci`), Nodes (`nodes`), Mindful v3 (`mind3`), Mindful v6 (`mind6`).
- Migrate and reconcile only one repository at a time; merge and verify it before creating the next worktree.
- Inspect existing branches, linked worktrees, and uncommitted state read-only. Make all migration writes in a fresh `.worktrees/` worktree.
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

## Shared Migration Ledger Contract

Every repository ledger contains these exact sections:

1. `Scope and evidence` — stable HEAD, Tasks source commit, audit date, and prefix.
2. `Git state inspected` — every local branch, linked worktree, and dirty path reviewed read-only.
3. `Document classification` — one row for each tracked file under `docs/` plus existing root `README.md`, `AGENTS.md`, and `CLAUDE.md`.
4. `Drift corrections` — evidence, correction, and outward-grep result for every changed claim.
5. `Candidate outcomes` — outcome, evidence, sources, active state, size, proposed status, blockers, disposition, and task ID.
6. `Deferred foreign dependencies` — local task, future project/outcome, evidence, and `pending` or `reconciled`; write `None` when empty.
7. `Verification` — exact command, result, and commit containing the result.

Use exactly one classification per document: `authority/current`, `active delivery`, or `historical/superseded`. Each repository task supplies its literal ledger path and exact coverage command; expected output is empty.

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

All CLI calls in a migration worktree use the same temporary `XDG_CONFIG_HOME`. Every mutation that records ownership or a note also sets `TASKS_OWNER` explicitly.

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
cd ~/d/tasks
git status --short --branch
git rev-parse HEAD
CARGO_HOME=/tmp/tasks-migration-cargo cargo test
```

Expected: the worktree has no unexplained changes and all 65 current tests pass. Record the exact commit for every repository ledger.

- [ ] **Step 2: Install the reviewed binary once**

```bash
cd ~/d/tasks
cargo install --locked --path . --force
command -v tasks
tasks --version
```

Expected: installation succeeds and `tasks --version` executes from the installed binary.

- [ ] **Step 3: Verify the user-level Tasks skill**

```bash
if test -L ~/.agents/skills/tasks && ! test -e ~/.agents/skills/tasks; then
  printf 'broken Tasks skill link; reconcile it explicitly\n' >&2
  exit 1
fi
if ! test -e ~/.agents/skills/tasks; then
  mkdir -p ~/.agents/skills
  ln -s ~/d/tasks/skills/tasks ~/.agents/skills/tasks
fi
test -e ~/.agents/skills/tasks
test -ef ~/.agents/skills/tasks ~/d/tasks/skills/tasks
```

If the path exists but `test -ef` fails, stop and reconcile it explicitly rather than overwriting it.

- [ ] **Step 4: Inventory all stable roots without modifying them**

```bash
for repo in ~/d/familiar ~/d/atoms ~/d/science ~/d/nodes ~/d/mindful/v3 ~/d/mindful/v6; do
  git -C "$repo" status --short --branch
  git -C "$repo" worktree list --porcelain
  git -C "$repo" branch --format='%(refname:short) %(objectname)'
  git -C "$repo" check-ignore -q .worktrees || printf 'NOT_IGNORED %s\n' "$repo"
done
```

Expected: every root is identified, existing dirty state is recorded but untouched, and `.worktrees/` is ignored. Stop before migration if a required correction would overlap unrelated dirty work.

- [ ] **Step 5: Inspect the normal Tasks registry read-only**

```bash
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
git -C ~/d/familiar worktree add -b chore/tasks-migration-fam ~/d/familiar/.worktrees/tasks-migration-fam main
git -C ~/d/familiar/.worktrees/tasks-migration-fam status --short --branch
```

Expected: a clean branch based on the current `main` commit recorded in the ledger.

- [ ] **Step 2: Establish the unmodified baseline**

```bash
cd ~/d/familiar/.worktrees/tasks-migration-fam
npm test
```

Expected: the repository's existing gate passes. Stop and report a baseline failure before changing documentation.

- [ ] **Step 3: Audit Git state and all project documents**

Read `README.md`, `docs/surfaces.md`, every tracked file under `docs/`, and existing root guidance. For every status header, checkbox, path, current-behavior claim, and remaining-work claim, verify against code/tests/configuration, commit ancestry, and the read-only branch/worktree inventory in that order. Write every file into `docs/plans/2026-08-30-familiar-tasks-migration.md` using the shared ledger contract.

- [ ] **Step 4: Correct evidence-backed drift**

Use `apply_patch` for the ledger and current documentation. Preserve historical rationale, make uncertainty explicit, and grep each corrected claim through user-facing and active-delivery documents:

```bash
rg -n 'Status:|to.?do|unchecked|supersed|README|surfaces|docs/(specs|plans|designs)' README.md AGENTS.md CLAUDE.md docs 2>/dev/null
comm -3 \
  <({ rg --files docs; for f in README.md AGENTS.md CLAUDE.md; do test ! -f "$f" || echo "$f"; done; } | sort -u) \
  <(sed -n '/^## Document classification$/,/^## /p' docs/plans/2026-08-30-familiar-tasks-migration.md | awk -F'`' '/^\| `/{print $2}' | sort -u)
git diff --check
```

Expected: the coverage comparison and `git diff --check` produce no output.

- [ ] **Step 5: Review and commit the documentation reconciliation**

Compare every ledger assertion with its cited evidence, run `npm test`, then commit only the ledger and drift corrections:

```bash
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
tasks_migration_config=$(mktemp -d)
export tasks_migration_config
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/familiar/.worktrees/tasks-migration-fam init --prefix fam
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/familiar/.worktrees/tasks-migration-fam prime
```

Expected: `prime` reports prefix `fam`.

- [ ] **Step 7: Create and verify the reviewed Familiar outcomes**

Apply the shared task-creation procedure to every ledger row marked `create`. Use the same `tasks_migration_config` for every call, record every generated ID, and leave completed/abandoned rows as `no task`. Add the shared agent guidance after task creation.

- [ ] **Step 8: Run the pilot's Tasks and repository gates**

```bash
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/familiar/.worktrees/tasks-migration-fam check >/tmp/fam-check.json
jq -e '.errors == [] and .warnings == []' /tmp/fam-check.json
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/familiar/.worktrees/tasks-migration-fam prime | jq -e '.prefix == "fam"'
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/familiar/.worktrees/tasks-migration-fam ready
npm test
git diff --check
```

Expected: all commands pass; the JSON arrays are empty.

- [ ] **Step 9: Commit and independently review the Tasks migration**

```bash
git add tasks AGENTS.md docs/plans/2026-08-30-familiar-tasks-migration.md
git diff --cached --check
git commit -m "chore(tasks): initialize project task tracking"
git status --short
```

Review both migration commits against the design, ledger evidence, task files, and baseline diff. Fix load-bearing findings through the CLI or `apply_patch`, rerun affected gates, and use conventional fixup commits before integration.

- [ ] **Step 10: Integrate, register, and clean up the pilot**

```bash
git -C ~/d/familiar merge --ff-only chore/tasks-migration-fam
cd ~/d/familiar
npm test
tasks init --prefix fam
tasks init --prefix fam
tasks prime | jq -e '.prefix == "fam"'
git status --short --branch
git worktree remove ~/d/familiar/.worktrees/tasks-migration-fam
git branch -d chore/tasks-migration-fam
rm -r -- "$tasks_migration_config"
```

Expected: `main` contains both commits, the normal registry maps `fam` to the stable checkout, and the migration worktree is gone. If the pilot exposes a contradiction in the design or ledger contract, stop, amend the design and this plan, review those changes, and only then begin Atoms.

### Task 3: Migrate Atoms

**Files:**

- Create: `~/d/atoms/.worktrees/tasks-migration-atoms/docs/plans/2026-08-30-atoms-tasks-migration.md`
- Create through CLI: `~/d/atoms/.worktrees/tasks-migration-atoms/tasks/.config.toml`
- Create through CLI: `~/d/atoms/.worktrees/tasks-migration-atoms/tasks/atoms-*.md`
- Modify: evidence-backed drift under `README.md` and `docs/`
- Modify: `AGENTS.md`

**Interfaces:**

- Consumes: integrated Familiar and the approved pilot procedure.
- Produces: integrated `atoms` tasks and a stable producer for Science dependencies.

- [ ] **Step 1: Create the Atoms worktree and run its baseline**

```bash
git -C ~/d/atoms worktree add -b chore/tasks-migration-atoms ~/d/atoms/.worktrees/tasks-migration-atoms main
cd ~/d/atoms/.worktrees/tasks-migration-atoms/python
uv run pytest
uv run ruff check .
uv run pyright
```

Expected: all existing gates pass before edits.

- [ ] **Step 2: Audit Atoms and write its ledger**

From the worktree root, read `AGENTS.md` first and treat its named authority design and obligation ledger as current anchors. Classify every document, verify A1–A9 and certification/adoption claims against code/tests/history, and distinguish Atoms delivery from Science-consumer work. Write `docs/plans/2026-08-30-atoms-tasks-migration.md` with the shared contract.

- [ ] **Step 3: Correct drift, prove coverage, and commit documentation**

```bash
cd ~/d/atoms/.worktrees/tasks-migration-atoms
rg -n 'Status:|A[1-9]|obligation|certif|adopt|Science|supersed' README.md AGENTS.md CLAUDE.md docs 2>/dev/null
comm -3 \
  <({ rg --files docs; for f in README.md AGENTS.md CLAUDE.md; do test ! -f "$f" || echo "$f"; done; } | sort -u) \
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
tasks_migration_config=$(mktemp -d)
export tasks_migration_config
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/familiar init --prefix fam
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/atoms/.worktrees/tasks-migration-atoms init --prefix atoms
```

- [ ] **Step 5: Create Atoms tasks and record future Science blockers**

Apply the shared task-creation procedure. Create an A9-related task only when the repository evidence proves remaining delivery work. Add resolvable dependencies now; record any verified blocker pointing to not-yet-migrated Science as `pending` in `Deferred foreign dependencies` without creating a dangling edge. Add the shared agent guidance.

- [ ] **Step 6: Verify, commit, and review Atoms**

```bash
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/atoms/.worktrees/tasks-migration-atoms check >/tmp/atoms-check.json
jq -e '.errors == [] and .warnings == []' /tmp/atoms-check.json
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/atoms/.worktrees/tasks-migration-atoms prime | jq -e '.prefix == "atoms"'
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/atoms/.worktrees/tasks-migration-atoms ready
cd ~/d/atoms/.worktrees/tasks-migration-atoms/python
uv run pytest
uv run ruff check .
uv run pyright
cd ..
git add tasks AGENTS.md docs/plans/2026-08-30-atoms-tasks-migration.md
git diff --cached --check
git commit -m "chore(tasks): initialize project task tracking"
git status --short
```

Independently review both commits and correct findings before integration.

- [ ] **Step 7: Integrate, register, and clean up Atoms**

```bash
git -C ~/d/atoms merge --ff-only chore/tasks-migration-atoms
cd ~/d/atoms/python && uv run pytest && uv run ruff check . && uv run pyright
cd ..
tasks init --prefix atoms
tasks init --prefix atoms
tasks prime | jq -e '.prefix == "atoms"'
git status --short --branch
git worktree remove ~/d/atoms/.worktrees/tasks-migration-atoms
git branch -d chore/tasks-migration-atoms
rm -r -- "$tasks_migration_config"
```

### Task 4: Migrate Science

**Files:**

- Create: `~/d/science/.worktrees/tasks-migration-sci/docs/plans/2026-08-30-science-tasks-migration.md`
- Create through CLI: `~/d/science/.worktrees/tasks-migration-sci/tasks/.config.toml`
- Create through CLI: `~/d/science/.worktrees/tasks-migration-sci/tasks/sci-*.md`
- Modify: evidence-backed drift under `README.md` and `docs/`
- Create: `AGENTS.md` if still absent; otherwise modify it

**Interfaces:**

- Consumes: integrated Familiar and Atoms, including resolvable `fam-*` and `atoms-*` IDs.
- Produces: integrated `sci` tasks and explicit repository guidance.

- [ ] **Step 1: Create the Science worktree and run both baselines**

```bash
git -C ~/d/science worktree add -b chore/tasks-migration-sci ~/d/science/.worktrees/tasks-migration-sci main
cd ~/d/science/.worktrees/tasks-migration-sci/python
uv run --frozen pytest -q
uv run --frozen ruff check .
uv run --frozen pyright
cd ../ts
npm test
npm run typecheck
npm run check
```

Expected: all six gates pass before edits.

- [ ] **Step 2: Audit Science and write its ledger**

Read the README, current roadmap and adoption ledgers, guide, active plans, code, and tests. Classify every document; verify adoption state, cut-12 claims, and unfinished work against all local branches/worktrees. Separate Science outcomes from already-owned Atoms outcomes. Write `docs/plans/2026-08-30-science-tasks-migration.md` with the shared contract.

- [ ] **Step 3: Correct drift, add guidance, and commit documentation**

Create `AGENTS.md` if absent with the repository's authority and gate summary. Add the shared Tasks guidance only after initialization in Step 5. Then run:

```bash
cd ~/d/science/.worktrees/tasks-migration-sci
rg -n 'Status:|roadmap|adopt|cut.?12|guide|Atoms|supersed' README.md AGENTS.md CLAUDE.md docs 2>/dev/null
comm -3 \
  <({ rg --files docs; for f in README.md AGENTS.md CLAUDE.md; do test ! -f "$f" || echo "$f"; done; } | sort -u) \
  <(sed -n '/^## Document classification$/,/^## /p' docs/plans/2026-08-30-science-tasks-migration.md | awk -F'`' '/^\| `/{print $2}' | sort -u)
git diff --check
cd python && uv run --frozen pytest -q && uv run --frozen ruff check . && uv run --frozen pyright
cd ../ts && npm test && npm run typecheck && npm run check
cd ..
git add README.md AGENTS.md docs
test ! -e CLAUDE.md || git add CLAUDE.md
git diff --cached --check
git commit -m "docs: reconcile project status for tasks migration"
```

- [ ] **Step 4: Initialize Science with earlier projects resolvable**

```bash
tasks -C ~/d/science/.worktrees/tasks-migration-sci prime
```

Expected: explicit `no_project` failure.

```bash
tasks_migration_config=$(mktemp -d)
export tasks_migration_config
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/familiar init --prefix fam
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/atoms init --prefix atoms
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/science/.worktrees/tasks-migration-sci init --prefix sci
```

- [ ] **Step 5: Create Science tasks and dependencies**

Apply the shared task-creation procedure. Add only blockers proven by the Science evidence; use existing Atoms IDs where delivery truly depends on them. Record blockers targeting Nodes or either Mindful repository as pending rather than dangling. Ensure `AGENTS.md` contains the shared guidance.

- [ ] **Step 6: Verify, commit, and independently review Science**

```bash
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/science/.worktrees/tasks-migration-sci check >/tmp/sci-check.json
jq -e '.errors == [] and .warnings == []' /tmp/sci-check.json
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/science/.worktrees/tasks-migration-sci prime | jq -e '.prefix == "sci"'
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/science/.worktrees/tasks-migration-sci ready
cd ~/d/science/.worktrees/tasks-migration-sci/python
uv run --frozen pytest -q
uv run --frozen ruff check .
uv run --frozen pyright
cd ../ts
npm test
npm run typecheck
npm run check
cd ..
git add tasks AGENTS.md docs/plans/2026-08-30-science-tasks-migration.md
git diff --cached --check
git commit -m "chore(tasks): initialize project task tracking"
git status --short
```

Independently review the full two-commit migration and fix findings before integration.

- [ ] **Step 7: Integrate, register, and clean up Science**

```bash
git -C ~/d/science merge --ff-only chore/tasks-migration-sci
cd ~/d/science/python && uv run --frozen pytest -q && uv run --frozen ruff check . && uv run --frozen pyright
cd ../ts && npm test && npm run typecheck && npm run check
cd ..
tasks init --prefix sci
tasks init --prefix sci
tasks prime | jq -e '.prefix == "sci"'
git status --short --branch
git worktree remove ~/d/science/.worktrees/tasks-migration-sci
git branch -d chore/tasks-migration-sci
rm -r -- "$tasks_migration_config"
```

### Task 5: Migrate Nodes

**Files:**

- Create: `~/d/nodes/.worktrees/tasks-migration-nodes/docs/plans/2026-08-30-nodes-tasks-migration.md`
- Create through CLI: `~/d/nodes/.worktrees/tasks-migration-nodes/tasks/.config.toml`
- Create through CLI: `~/d/nodes/.worktrees/tasks-migration-nodes/tasks/nodes-*.md`
- Modify: evidence-backed drift under `README.md` and `docs/`
- Modify: `AGENTS.md`

**Interfaces:**

- Consumes: integrated Familiar, Atoms, and Science.
- Produces: integrated `nodes` tasks and stable blocker IDs for both Mindful repositories.

- [ ] **Step 1: Create the Nodes worktree and run both baselines**

```bash
git -C ~/d/nodes worktree add -b chore/tasks-migration-nodes ~/d/nodes/.worktrees/tasks-migration-nodes main
cd ~/d/nodes/.worktrees/tasks-migration-nodes/python
uv run --frozen pytest -q
uv run --frozen ruff check .
uv run --frozen pyright src
cd ../ts
npm test
npm run typecheck
npm run check
```

Expected: all six gates pass before edits.

- [ ] **Step 2: Audit Nodes and write its ledger**

Read `AGENTS.md` first and treat `docs/STANDARD.md` as the authority it names. Verify Python/TypeScript parity claims, current APIs, tests, active plans, and README statements. Classify every document and express parity as one outcome when it ships as one result rather than splitting it by language. Write `docs/plans/2026-08-30-nodes-tasks-migration.md`.

- [ ] **Step 3: Correct drift, prove coverage, and commit documentation**

```bash
cd ~/d/nodes/.worktrees/tasks-migration-nodes
rg -n 'Status:|STANDARD|parity|Python|TypeScript|Mindful|supersed' README.md AGENTS.md CLAUDE.md docs 2>/dev/null
comm -3 \
  <({ rg --files docs; for f in README.md AGENTS.md CLAUDE.md; do test ! -f "$f" || echo "$f"; done; } | sort -u) \
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
tasks_migration_config=$(mktemp -d)
export tasks_migration_config
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/familiar init --prefix fam
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/atoms init --prefix atoms
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/science init --prefix sci
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/nodes/.worktrees/tasks-migration-nodes init --prefix nodes
```

- [ ] **Step 5: Create Nodes tasks and dependencies**

Apply the shared task-creation procedure. Keep cross-language delivery together when acceptance requires parity. Record verified blockers targeting either future Mindful migration as pending; do not create dangling IDs. Add the shared agent guidance.

- [ ] **Step 6: Verify, commit, and independently review Nodes**

```bash
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/nodes/.worktrees/tasks-migration-nodes check >/tmp/nodes-check.json
jq -e '.errors == [] and .warnings == []' /tmp/nodes-check.json
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/nodes/.worktrees/tasks-migration-nodes prime | jq -e '.prefix == "nodes"'
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/nodes/.worktrees/tasks-migration-nodes ready
cd ~/d/nodes/.worktrees/tasks-migration-nodes/python
uv run --frozen pytest -q
uv run --frozen ruff check .
uv run --frozen pyright src
cd ../ts
npm test
npm run typecheck
npm run check
cd ..
git add tasks AGENTS.md docs/plans/2026-08-30-nodes-tasks-migration.md
git diff --cached --check
git commit -m "chore(tasks): initialize project task tracking"
git status --short
```

Independently review the complete migration and fix findings before integration.

- [ ] **Step 7: Integrate, register, and clean up Nodes**

```bash
git -C ~/d/nodes merge --ff-only chore/tasks-migration-nodes
cd ~/d/nodes/python && uv run --frozen pytest -q && uv run --frozen ruff check . && uv run --frozen pyright src
cd ../ts && npm test && npm run typecheck && npm run check
cd ~/d/nodes
tasks init --prefix nodes
tasks init --prefix nodes
tasks prime | jq -e '.prefix == "nodes"'
git status --short --branch
git worktree remove ~/d/nodes/.worktrees/tasks-migration-nodes
git branch -d chore/tasks-migration-nodes
rm -r -- "$tasks_migration_config"
```

### Task 6: Migrate Mindful v3

**Files:**

- Create: `~/d/mindful/v3/.worktrees/tasks-migration-mind3/docs/plans/2026-08-30-mindful-v3-tasks-migration.md`
- Create through CLI: `~/d/mindful/v3/.worktrees/tasks-migration-mind3/tasks/.config.toml`
- Create through CLI: `~/d/mindful/v3/.worktrees/tasks-migration-mind3/tasks/mind3-*.md`
- Modify: evidence-backed drift under `README.md` and `docs/`
- Modify: `AGENTS.md`

**Interfaces:**

- Consumes: integrated Familiar, Atoms, Science, and Nodes.
- Produces: integrated `mind3` tasks and stable v3 IDs for Mindful v6 dependencies.

- [ ] **Step 1: Create the v3 worktree and record service state**

```bash
git -C ~/d/mindful/v3 worktree add -b chore/tasks-migration-mind3 ~/d/mindful/v3/.worktrees/tasks-migration-mind3 main
cd ~/d/mindful/v3/.worktrees/tasks-migration-mind3
docker compose ps
```

Record whether the required stack was already running. Start it only if the repository's existing gate requires it and it is absent; do not recreate healthy services unnecessarily.

- [ ] **Step 2: Run the v3 baseline gates**

```bash
cd ~/d/mindful/v3/.worktrees/tasks-migration-mind3/react
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
cd ~/d/mindful/v3/.worktrees/tasks-migration-mind3
rg -n 'Status:|roadmap|moderni[sz]|v3|v6|import|supersed' README.md AGENTS.md CLAUDE.md docs 2>/dev/null
comm -3 \
  <({ rg --files docs; for f in README.md AGENTS.md CLAUDE.md; do test ! -f "$f" || echo "$f"; done; } | sort -u) \
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
tasks_migration_config=$(mktemp -d)
export tasks_migration_config
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/familiar init --prefix fam
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/atoms init --prefix atoms
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/science init --prefix sci
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/nodes init --prefix nodes
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/mindful/v3/.worktrees/tasks-migration-mind3 init --prefix mind3
```

- [ ] **Step 6: Create v3 tasks and dependencies**

Apply the shared task-creation procedure. Mark a task `doing` only with a verified owner token; never treat an old branch alone as ownership. Add direct Nodes blockers when proven. Record blockers targeting Mindful v6 as pending. Add the shared agent guidance.

- [ ] **Step 7: Verify, commit, and independently review v3**

```bash
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/mindful/v3/.worktrees/tasks-migration-mind3 check >/tmp/mind3-check.json
jq -e '.errors == [] and .warnings == []' /tmp/mind3-check.json
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/mindful/v3/.worktrees/tasks-migration-mind3 prime | jq -e '.prefix == "mind3"'
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/mindful/v3/.worktrees/tasks-migration-mind3 ready
cd ~/d/mindful/v3/.worktrees/tasks-migration-mind3/react
npm run typecheck
npm run lint
npm run test
npm run build
cd ..
docker exec mindful_fastapi_v3 uv run pytest tests -v
docker exec mindful_fastapi_v3 uv run pytest tests/test_graph_producers.py -v
docker exec mindful_fastapi_v3 uv run ruff check .
git add tasks AGENTS.md docs/plans/2026-08-30-mindful-v3-tasks-migration.md
git diff --cached --check
git commit -m "chore(tasks): initialize project task tracking"
git status --short
```

Independently review both commits and fix findings before integration.

- [ ] **Step 8: Integrate, register, and clean up v3**

```bash
git -C ~/d/mindful/v3 merge --ff-only chore/tasks-migration-mind3
cd ~/d/mindful/v3/react && npm run typecheck && npm run lint && npm run test && npm run build
cd ..
docker exec mindful_fastapi_v3 uv run pytest tests -v
docker exec mindful_fastapi_v3 uv run pytest tests/test_graph_producers.py -v
docker exec mindful_fastapi_v3 uv run ruff check .
tasks init --prefix mind3
tasks init --prefix mind3
tasks prime | jq -e '.prefix == "mind3"'
git status --short --branch
git worktree remove ~/d/mindful/v3/.worktrees/tasks-migration-mind3
git branch -d chore/tasks-migration-mind3
rm -r -- "$tasks_migration_config"
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
git -C ~/d/mindful/v6 worktree add -b chore/tasks-migration-mind6 ~/d/mindful/v6/.worktrees/tasks-migration-mind6 main
cd ~/d/mindful/v6/.worktrees/tasks-migration-mind6
npm test
npm run typecheck
npm run check
```

If a gate exits 2 solely because the Nodes core build is stale, run `npm run build` in `~/d/nodes/ts`, rerun all three v6 gates, and record that prerequisite in the ledger. Treat every other failure as a baseline failure.

- [ ] **Step 2: Audit Mindful v6 and write its ledger**

Read `AGENTS.md`, `docs/ARCHITECTURE.md`, and `docs/FORMATS.md` first as standing authority. Classify all documents; verify the v3 import boundary, Nodes integration, sprint/history claims, and all active plans against code/tests/current Git state. Write `docs/plans/2026-08-30-mindful-v6-tasks-migration.md`.

- [ ] **Step 3: Correct drift, prove coverage, and commit documentation**

```bash
cd ~/d/mindful/v6/.worktrees/tasks-migration-mind6
rg -n 'Status:|ARCHITECTURE|FORMATS|v3|v6|Nodes|sprint|import|supersed' README.md AGENTS.md CLAUDE.md docs 2>/dev/null
comm -3 \
  <({ rg --files docs; for f in README.md AGENTS.md CLAUDE.md; do test ! -f "$f" || echo "$f"; done; } | sort -u) \
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
tasks_migration_config=$(mktemp -d)
export tasks_migration_config
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/familiar init --prefix fam
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/atoms init --prefix atoms
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/science init --prefix sci
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/nodes init --prefix nodes
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/mindful/v3 init --prefix mind3
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/mindful/v6/.worktrees/tasks-migration-mind6 init --prefix mind6
```

- [ ] **Step 5: Create v6 tasks and dependencies**

Apply the shared task-creation procedure. Add direct dependencies on existing Nodes and Mindful v3 tasks only when they block delivery. Any dependency targeting earlier projects is now resolvable; treat failure to resolve as a registry or evidence error rather than omitting it. Add the shared agent guidance.

- [ ] **Step 6: Verify, commit, and independently review v6**

```bash
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/mindful/v6/.worktrees/tasks-migration-mind6 check >/tmp/mind6-check.json
jq -e '.errors == [] and .warnings == []' /tmp/mind6-check.json
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/mindful/v6/.worktrees/tasks-migration-mind6 prime | jq -e '.prefix == "mind6"'
XDG_CONFIG_HOME="$tasks_migration_config" tasks -C ~/d/mindful/v6/.worktrees/tasks-migration-mind6 ready
cd ~/d/mindful/v6/.worktrees/tasks-migration-mind6
npm test
npm run typecheck
npm run check
git add tasks AGENTS.md docs/plans/2026-08-30-mindful-v6-tasks-migration.md
git diff --cached --check
git commit -m "chore(tasks): initialize project task tracking"
git status --short
```

Independently review both commits and fix findings before integration.

- [ ] **Step 7: Integrate, register, and clean up v6**

```bash
git -C ~/d/mindful/v6 merge --ff-only chore/tasks-migration-mind6
cd ~/d/mindful/v6
npm test
npm run typecheck
npm run check
tasks init --prefix mind6
tasks init --prefix mind6
tasks prime | jq -e '.prefix == "mind6"'
git status --short --branch
git worktree remove ~/d/mindful/v6/.worktrees/tasks-migration-mind6
git branch -d chore/tasks-migration-mind6
rm -r -- "$tasks_migration_config"
```

### Task 8: Reconcile Deferred Cross-Project Dependencies

**Files:**

- Modify through CLI, only when pending rows exist: one or more of `tasks/fam-*.md`, `tasks/atoms-*.md`, `tasks/sci-*.md`, `tasks/nodes-*.md`, `tasks/mind3-*.md`, and `tasks/mind6-*.md` in the affected reconciliation worktree
- Modify, only when pending rows exist: the affected repository's `docs/plans/2026-08-30-*-tasks-migration.md`

**Interfaces:**

- Consumes: all six integrated task stores and every ledger's `Deferred foreign dependencies` table.
- Produces: committed dependency edges for every pending row, or a verified no-op when no row is pending.

- [ ] **Step 1: Build the exact six-project portfolio registry through the CLI**

```bash
tasks_portfolio_config=$(mktemp -d)
export tasks_portfolio_config
XDG_CONFIG_HOME="$tasks_portfolio_config" tasks -C ~/d/familiar init --prefix fam
XDG_CONFIG_HOME="$tasks_portfolio_config" tasks -C ~/d/atoms init --prefix atoms
XDG_CONFIG_HOME="$tasks_portfolio_config" tasks -C ~/d/science init --prefix sci
XDG_CONFIG_HOME="$tasks_portfolio_config" tasks -C ~/d/nodes init --prefix nodes
XDG_CONFIG_HOME="$tasks_portfolio_config" tasks -C ~/d/mindful/v3 init --prefix mind3
XDG_CONFIG_HOME="$tasks_portfolio_config" tasks -C ~/d/mindful/v6 init --prefix mind6
```

Expected: six successful `init` calls. Do not write `projects.toml` directly.

- [ ] **Step 2: Enumerate pending rows exactly**

```bash
for ledger in \
  ~/d/familiar/docs/plans/2026-08-30-familiar-tasks-migration.md \
  ~/d/atoms/docs/plans/2026-08-30-atoms-tasks-migration.md \
  ~/d/science/docs/plans/2026-08-30-science-tasks-migration.md \
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

If no row prints, record reconciliation as a verified no-op and continue to Task 9. Otherwise process the printed repositories one at a time in portfolio order.

- [ ] **Step 3: Define the reconciliation gate for all possible affected projects**

```bash
run_reconciliation_gate() {
  case "$1" in
    fam)
      (cd "$2" && npm test)
      ;;
    atoms)
      (cd "$2/python" && uv run pytest && uv run ruff check . && uv run pyright)
      ;;
    sci)
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

Expected: the function returns 0 only when the affected repository's complete gate passes.

- [ ] **Step 4: Reconcile one affected repository in a fresh worktree**

Resolve the repository, prefix, ledger, and worktree from this fixed map:

| Repository | Prefix | Reconciliation worktree | Ledger |
|------------|--------|-------------------------|--------|
| `~/d/familiar` | `fam` | `~/d/familiar/.worktrees/tasks-reconciliation-fam` | `docs/plans/2026-08-30-familiar-tasks-migration.md` |
| `~/d/atoms` | `atoms` | `~/d/atoms/.worktrees/tasks-reconciliation-atoms` | `docs/plans/2026-08-30-atoms-tasks-migration.md` |
| `~/d/science` | `sci` | `~/d/science/.worktrees/tasks-reconciliation-sci` | `docs/plans/2026-08-30-science-tasks-migration.md` |
| `~/d/nodes` | `nodes` | `~/d/nodes/.worktrees/tasks-reconciliation-nodes` | `docs/plans/2026-08-30-nodes-tasks-migration.md` |
| `~/d/mindful/v3` | `mind3` | `~/d/mindful/v3/.worktrees/tasks-reconciliation-mind3` | `docs/plans/2026-08-30-mindful-v3-tasks-migration.md` |
| `~/d/mindful/v6` | `mind6` | `~/d/mindful/v6/.worktrees/tasks-reconciliation-mind6` | `docs/plans/2026-08-30-mindful-v6-tasks-migration.md` |

Set `repo`, `prefix`, `worktree`, and `ledger_rel` to one literal row, then run:

```bash
git -C "$repo" worktree add -b "chore/tasks-reconciliation-$prefix" "$worktree" main
run_reconciliation_gate "$prefix" "$worktree"
```

Expected: the fresh worktree baseline passes. Do not create the next reconciliation worktree until this one is merged and removed.

- [ ] **Step 5: Add every pending edge through the CLI**

For each pending row in that ledger, copy its literal local and foreign IDs into one input line, then run:

```bash
read -r local_id foreign_id
XDG_CONFIG_HOME="$tasks_portfolio_config" TASKS_OWNER=migration tasks -C "$worktree" dep "$local_id" --on "$foreign_id"
XDG_CONFIG_HOME="$tasks_portfolio_config" tasks -C "$worktree" show "$local_id"
```

If the edge fails resolution or acyclicity, stop; do not weaken or omit it. Use `apply_patch` to change that exact row from `pending` to `reconciled` and record the new edge.

- [ ] **Step 6: Verify and commit the affected reconciliation**

```bash
XDG_CONFIG_HOME="$tasks_portfolio_config" tasks -C "$worktree" check >/tmp/reconciliation-check.json
jq -e '.errors == [] and .warnings == []' /tmp/reconciliation-check.json
run_reconciliation_gate "$prefix" "$worktree"
git -C "$worktree" diff --check
git -C "$worktree" add tasks "$ledger_rel"
git -C "$worktree" diff --cached --check
git -C "$worktree" commit -m "chore(tasks): reconcile cross-project dependencies"
git -C "$worktree" status --short
```

Expected: one commit contains both the CLI-generated dependency edit and the reconciled ledger row.

- [ ] **Step 7: Review, integrate, and remove the affected reconciliation**

Independently review the edge evidence, full reachable graph, ledger update, and commit diff. Fix findings and rerun Step 6, then:

```bash
git -C "$repo" merge --ff-only "chore/tasks-reconciliation-$prefix"
run_reconciliation_gate "$prefix" "$repo"
XDG_CONFIG_HOME="$tasks_portfolio_config" tasks -C "$repo" check >/tmp/reconciliation-stable-check.json
jq -e '.errors == [] and .warnings == []' /tmp/reconciliation-stable-check.json
cd "$repo"
git worktree remove "$worktree"
git branch -d "chore/tasks-reconciliation-$prefix"
```

Repeat Steps 4–7 for the next affected repository only after this stable check passes.

- [ ] **Step 8: Prove reconciliation is complete**

Rerun Step 2. Expected: no pending row prints. Keep `tasks_portfolio_config` for Task 9 or remove it and create a fresh one there.

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
tasks_portfolio_config=$(mktemp -d)
export tasks_portfolio_config
XDG_CONFIG_HOME="$tasks_portfolio_config" tasks -C ~/d/familiar init --prefix fam
XDG_CONFIG_HOME="$tasks_portfolio_config" tasks -C ~/d/atoms init --prefix atoms
XDG_CONFIG_HOME="$tasks_portfolio_config" tasks -C ~/d/science init --prefix sci
XDG_CONFIG_HOME="$tasks_portfolio_config" tasks -C ~/d/nodes init --prefix nodes
XDG_CONFIG_HOME="$tasks_portfolio_config" tasks -C ~/d/mindful/v3 init --prefix mind3
XDG_CONFIG_HOME="$tasks_portfolio_config" tasks -C ~/d/mindful/v6 init --prefix mind6
```

Expected: all six initializations succeed; unrelated registry projects are absent.

- [ ] **Step 2: Require empty checks and exact prefixes in all six projects**

```bash
printf '%s %s\n' \
  "$HOME/d/familiar" fam \
  "$HOME/d/atoms" atoms \
  "$HOME/d/science" sci \
  "$HOME/d/nodes" nodes \
  "$HOME/d/mindful/v3" mind3 \
  "$HOME/d/mindful/v6" mind6 |
while read -r root prefix; do
  XDG_CONFIG_HOME="$tasks_portfolio_config" tasks -C "$root" check >"/tmp/$prefix-portfolio-check.json"
  jq -e '.errors == [] and .warnings == []' "/tmp/$prefix-portfolio-check.json"
  XDG_CONFIG_HOME="$tasks_portfolio_config" tasks -C "$root" prime | jq -e --arg prefix "$prefix" '.prefix == $prefix'
  XDG_CONFIG_HOME="$tasks_portfolio_config" tasks -C "$root" ready
done
```

Expected: every command passes and every check has zero warnings. The literal paths above are command input only; do not copy them into committed documentation.

- [ ] **Step 3: Require strict global listing success without warnings**

```bash
XDG_CONFIG_HOME="$tasks_portfolio_config" tasks -C ~/d/familiar list --all-projects >/tmp/tasks-portfolio-list.json 2>/tmp/tasks-portfolio-list.err
test ! -s /tmp/tasks-portfolio-list.err
jq -e '.warnings == [] and (.tasks | type == "array")' /tmp/tasks-portfolio-list.json
```

Expected: strict scans of all six projects succeed and stderr is empty.

- [ ] **Step 4: Audit ledgers and task semantics**

For each ledger, rerun its exact coverage comparison. Then verify:

```bash
! rg -n '^\|.*\| pending \|$' \
  ~/d/familiar/docs/plans/2026-08-30-familiar-tasks-migration.md \
  ~/d/atoms/docs/plans/2026-08-30-atoms-tasks-migration.md \
  ~/d/science/docs/plans/2026-08-30-science-tasks-migration.md \
  ~/d/nodes/docs/plans/2026-08-30-nodes-tasks-migration.md \
  ~/d/mindful/v3/docs/plans/2026-08-30-mindful-v3-tasks-migration.md \
  ~/d/mindful/v6/docs/plans/2026-08-30-mindful-v6-tasks-migration.md
```

Expected: no pending row. Inspect every `Candidate outcomes` row against its task ID or explicit no-task disposition; confirm completed history was not backfilled, every unresolved claim is an `idea`, every `doing` owner has evidence, and every foreign edge is a real blocker.

- [ ] **Step 5: Run every repository's complete stable gate**

```bash
(cd ~/d/familiar && npm test)
(cd ~/d/atoms/python && uv run pytest && uv run ruff check . && uv run pyright)
(cd ~/d/science/python && uv run --frozen pytest -q && uv run --frozen ruff check . && uv run --frozen pyright)
(cd ~/d/science/ts && npm test && npm run typecheck && npm run check)
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
tasks -C ~/d/familiar init --prefix fam && tasks -C ~/d/familiar prime | jq -e '.prefix == "fam"'
tasks -C ~/d/atoms init --prefix atoms && tasks -C ~/d/atoms prime | jq -e '.prefix == "atoms"'
tasks -C ~/d/science init --prefix sci && tasks -C ~/d/science prime | jq -e '.prefix == "sci"'
tasks -C ~/d/nodes init --prefix nodes && tasks -C ~/d/nodes prime | jq -e '.prefix == "nodes"'
tasks -C ~/d/mindful/v3 init --prefix mind3 && tasks -C ~/d/mindful/v3 prime | jq -e '.prefix == "mind3"'
tasks -C ~/d/mindful/v6 init --prefix mind6 && tasks -C ~/d/mindful/v6 prime | jq -e '.prefix == "mind6"'
```

Expected: every repeated init succeeds and every prefix is exact. If a canonical root moved, stop and back up the normal registry:

```bash
tasks_registry_home=${XDG_CONFIG_HOME:-"$HOME/.config"}
tasks_registry_path="$tasks_registry_home/tasks/projects.toml"
test ! -e "$tasks_registry_path.pre-migration-repair"
cp -- "$tasks_registry_path" "$tasks_registry_path.pre-migration-repair"
```

Use `apply_patch` to remove only the stale entries among `fam`, `atoms`, `sci`, `nodes`, `mind3`, and `mind6`; preserve every unrelated entry. Then rerun the six CLI commands above and repeat Steps 1–3. This is the explicit normal-registry recovery exception; `tasks init` must never silently rebind a prefix.

- [ ] **Step 7: Create the Tasks completion worktree**

```bash
git -C ~/d/tasks worktree add -b docs/project-migration-complete ~/d/tasks/.worktrees/project-migration-complete main
cd ~/d/tasks/.worktrees/project-migration-complete
```

Expected: a clean worktree based on the then-current `main`, which must already contain the approved design and this plan.

- [ ] **Step 8: Make completion claims truthful**

Use `apply_patch` to change the design status from approved/not implemented to implemented with the actual date from `date +%F`. Check only plan boxes whose command and evidence were actually verified. Add concise final commit references or verification results where the ledger contract requires them. Then search outward for stale migration claims:

```bash
rg -n 'project.tasks.migration|not implemented|implementation planning|migration.*pending' README.md AGENTS.md CLAUDE.md docs 2>/dev/null
git diff --check
```

Correct propagated user-facing drift in the same change. Do not check a box based only on an earlier unchecked or checked claim.

- [ ] **Step 9: Verify, commit, review, and integrate the completion record**

```bash
CARGO_HOME=/tmp/tasks-migration-cargo cargo test
git add docs
for f in README.md AGENTS.md CLAUDE.md; do test ! -e "$f" || git add "$f"; done
git diff --cached --check
git commit -m "docs: record project task migrations complete"
git status --short
```

Omit nonexistent pathspecs. Independently review the completion criteria against all six stable trees and the portfolio outputs. Fix any finding and rerun the affected gate before integration:

```bash
git -C ~/d/tasks merge --ff-only docs/project-migration-complete
cd ~/d/tasks
CARGO_HOME=/tmp/tasks-migration-cargo cargo test
git status --short --branch
git worktree remove ~/d/tasks/.worktrees/project-migration-complete
git branch -d docs/project-migration-complete
rm -r -- "$tasks_portfolio_config"
```

Expected: Tasks `main` records the implemented design, the plan reflects only verified history, and all 65 current Tasks tests pass.
