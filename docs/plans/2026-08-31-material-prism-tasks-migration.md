# Material and Prism Tasks Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Audit Material and Prism against their repositories, correct current
documentation drift, and extend the completed Tasks portfolio with forward-only
`material` and `prism` task stores.

**Architecture:** Migrate Material and then Prism in separate worktrees, using a
CLI-built temporary registry that grows from the completed six-project baseline. Each
repository lands reviewed documentation and Tasks commits, then a reviewed integration
ledger commit; after both land, reconcile only a deferred Material-to-Prism blocker and
run a fresh exact-eight portfolio gate.

**Tech Stack:** Rust `tasks` CLI, Git worktrees, Markdown, TOML, zsh, `jq`, Cargo,
uv/MkDocs, npm/Node.js, Lua, and Qt 6 `qmllint`.

**Spec:** `docs/specs/2026-08-31-material-prism-tasks-migration-design.md`

**Status:** implemented on 2026-09-01. Material is integrated at
`8047b6ca14ec1e2a0760a79f5d9d4883a9fc2519`, Prism is integrated at
`b05b4af214ee714faa6fbd41bbf9121e902e3f59`, reconciliation was a reviewed no-op,
and Task 5 records the fresh exact-eight completion and cleanup evidence.

## Global Constraints

- Migrate in order: Material (`material`) from `~/d/niri-material` branch
  `materials-26.04`, then Prism (`prism`) from `~/d/prism` branch `main`.
- Migrate and reconcile one repository at a time. Merge, verify, and clean up the
  current repository before creating the next migration or reconciliation worktree.
- Treat the approved starting commits as guards: Material's stable branch
  `7e94d71af195d5f5062d9b51ec80bca513af8ae3` is three commits ahead of
  `origin/materials-26.04`; its active debug branch remains separately pinned at
  `048814893b9bc4924927468ce1f539b7764d942b`; Prism is
  `d20111c2182adcbbb2bd3b76356d6e1557cb1e12`. Stop and amend the evidence if either
  stable branch moves before its task starts.
- Preserve `~/d/niri-material/.worktrees/overview-drag-frost` byte-for-byte. It must
  remain on `debug/overview-drag-frost` at
  `048814893b9bc4924927468ce1f539b7764d942b` with only
  `niri-config/src/lib.rs`, `src/layout/tile.rs`,
  `src/render_helpers/effect_buffer.rs`, `src/render_helpers/material.rs`, and
  `src/render_helpers/xray.rs` modified. The two intervening stable commits are
  documentation-only evidence for the same open frost investigation; tracked docs
  remain 111.
- Inspect existing branches, worktrees, and dirty state read-only. All migration writes
  occur in fresh `.worktrees/` worktrees; never stash, clean, repurpose, or modify active
  work. The sole stable-checkout untracked-state exception is Prism's required `npm ci`,
  which replaces ignored `node_modules/` reproducibly; require clean Git status before
  and after it.
- Do not deploy, restart or mutate a live compositor or shell, run a GPU client, perform
  physical DRM acceptance, or rerun historical nested-Winit, burn-in, or subjective
  visual evidence.
- Do not create tasks for completed, abandoned, speculative, or externally owned
  history. Create tasks only for evidence-backed unfinished outcomes; use `idea` when
  evidence is inconclusive.
- Size tasks as independently shippable outcomes using `xs`, `s`, `m`, `l`, or `xl`.
  Never turn implementation steps into tasks.
- `tasks add` creates only `idea` or `todo`. Create `doing` by adding `todo` and then
  running `tasks start`; create `blocked` by adding `todo` and then `tasks block`.
- Set `TASKS_OWNER=migration` for migration-authored notes. Use
  `TASKS_OWNER=debug/overview-drag-frost` only for the verified active Material outcome.
  Never inherit ownership from a migration branch or shell.
- Give every created task the complete initial tag set in its single `tasks add` call,
  including `migration`. Later `edit --tag` replaces all tags.
- Task bodies state outcome, acceptance evidence, source documents, and uncertainty.
  They contain no bare `## Notes` line. Note and close/block messages are single-line.
- Add dependencies only for genuine delivery blockers. A Material-to-not-yet-created
  Prism blocker stays in the Material ledger until Task 4; Prism-to-Material blockers
  resolve during Task 3.
- Use structured `spec`, `plan`, and `step` only for compatible documents under
  `docs/specs/**` and `docs/plans/**`; cite all other source paths in the task body.
- Create and mutate `tasks/*.md` only through the CLI. Build every temporary registry
  through `tasks init`; never hand-edit its `projects.toml`.
- Pin `TASKS_FORMAT=json` for machine-consumed output. Every `tasks check` gate requires
  both `.errors == []` and `.warnings == []`; exit zero alone is insufficient.
- The exact-eight temporary portfolio excludes normal-registry project `aut`. Leave its
  mapping and every other unrelated mapping untouched.
- Do not add CI enforcement. Use conventional commits with no attribution trailers,
  stage named paths, and preserve unrelated user changes.
- Use `~/d/...` notation in committed docs; never record machine-resolved home paths.
- Treat every checkbox as a fresh shell invocation. Begin command blocks with
  `set -euo pipefail`, set the working directory explicitly, and restore cross-step state
  only from the named `/tmp/*.registry-path` control file.

## Shared Migration Ledger Contract

Each repository ledger contains these sections:

1. `Scope and evidence` — stable HEAD, Tasks source commit, audit date, prefix, and
   authority roots.
2. `Git state inspected` — every local branch, linked worktree, dirty path, and preserved
   active-work fingerprint reviewed read-only.
3. `Document classification` — exact tracked-document coverage plus the ledger and
   current root guidance, using `authority/current`, `active delivery`, or
   `historical/superseded`.
4. `Drift corrections` — claim, evidence, correction, and outward-grep result.
5. `Candidate outcomes` — outcome, evidence, sources, active state, size, proposed
   status, blockers, disposition, and task ID.
6. `Deferred foreign dependencies` — local task, future project/outcome, evidence, and
   `pending` or `reconciled`; write `None` when empty.
7. `Verification` — exact command, result, and commit containing the recorded result.

Material's classification uses one literal bulk row:

```markdown
| `docs/wiki/**` | `historical/superseded` | Inherited upstream wiki; unchanged except listed per-file exceptions. |
```

It uses a second literal bulk row for the six docs-build files:

```markdown
| `docs build tooling` | `authority/current` | `docs/.gitignore`, `docs/hooks/**`, `docs/mkdocs.yaml`, `docs/pyproject.toml`, and `docs/uv.lock`; verified by the strict docs gate. |
```

Every other non-wiki path—including
`docs/superpowers/plans/2026-08-29-material-backdrop-blur.md`—and every changed wiki
exception receives an explicit row. The NUL-delimited comparison in Task 2 proves that
the two bulk rules plus explicit rows cover the exact tracked set without classifying a
lockfile as a project document or relying on Git's quoted display.

After stable verification and canonical registration, update the ledger in the still-open
migration worktree. If no dependency row is pending, mark the migration complete and
classify the ledger `historical/superseded`. If a Material-to-Prism row is pending, record
initial integration but keep the ledger `active delivery` until Task 4 closes it.

## Shared Agent Guidance

Append this section to Material's `.agents/AGENTS.md` and create Prism's root
`AGENTS.md` with the same section:

```markdown
## Tasks workflow

- Run `tasks prime` at the start of a work session and `tasks ready` before choosing work.
- Run `tasks start ID` before implementation, add concise notes as evidence changes, and close the task with a one-line result in the same commit as the work.
- Never edit `tasks/*.md` directly; use the `tasks` CLI for every task mutation.
- Before completion, run `tasks check`. Require zero errors and report every warning. Registration-only `unreachable_dep` and `cycle_unverifiable` warnings are environmental on machines without all referenced projects; resolve every other warning.
```

## Shared Task-Creation Procedure

For every reviewed candidate row marked `create`:

1. Run one `tasks add` with the row's literal title, body, size, `idea` or `todo` status,
   and complete repeated `--tag` set.
2. Add every already-resolvable blocker with `tasks dep ID --on BLOCKER_ID`.
3. For verified active work, run `TASKS_OWNER=debug/overview-drag-frost tasks start ID`;
   otherwise retain `todo` or `idea`.
4. For an actual non-task obstruction, run
   `TASKS_OWNER=migration tasks block ID 'one-line reason from the reviewed row'`.
5. Copy the emitted ID into the candidate row with `apply_patch`; never infer an ID or
   edit generated task Markdown.
6. Run `tasks show ID` and compare every field with the reviewed row before continuing.

Every command uses the task's temporary `XDG_CONFIG_HOME`. Re-read and validate the
control path on every step, set `TASKS_FORMAT=json`, and set `TASKS_OWNER` explicitly on
every owner/note mutation.

---

### Task 1: Pin Tasks and Preflight the Eight-Project Boundary

**Files:**

- Read: `Cargo.toml`, `src/`, `skills/tasks/SKILL.md`
- Inspect: `~/d/niri-material`, its linked worktrees, and `~/d/prism`
- Inspect: the six completed portfolio roots and the normal Tasks registry
- Create outside Git: `/tmp/tasks-material-active-worktree.sha256`
- Create outside Git: `/tmp/tasks-material-docs-environment.root`

**Interfaces:**

- Produces: one tested installed `tasks` binary, the recorded Tasks source commit,
  verified stable roots, and a byte fingerprint protecting Material's active debug work.

- [ ] **Step 1: Verify and install the reviewed Tasks source**

```bash
set -euo pipefail
cd ~/d/tasks
test "$(git branch --show-current)" = main
git show main:docs/specs/2026-08-31-material-prism-tasks-migration-design.md >/dev/null
git show main:docs/plans/2026-08-31-material-prism-tasks-migration.md >/dev/null
cmp docs/specs/2026-08-31-material-prism-tasks-migration-design.md \
  <(git show main:docs/specs/2026-08-31-material-prism-tasks-migration-design.md)
cmp docs/plans/2026-08-31-material-prism-tasks-migration.md \
  <(git show main:docs/plans/2026-08-31-material-prism-tasks-migration.md)
git status --short --branch
test -z "$(git status --porcelain=v1)"
git rev-parse HEAD
cargo test
cargo install --locked --path . --force
command -v tasks
tasks --version
```

Expected: `main` contains the reviewed design and this exact plan, the complete suite
passes, and the installed binary reports its version. Record the exact source commit in
both migration ledgers. Before execution, merge the documentation branch into `main`,
then remove its worktree and merged branch; do not begin from the documentation branch.

- [ ] **Step 2: Verify both user-level Tasks skill links**

```bash
set -euo pipefail
for skill_link in "$HOME/.agents/skills/tasks" "$HOME/.claude/skills/tasks"; do
  test -e "$skill_link"
  test "$skill_link" -ef "$HOME/d/tasks/skills/tasks"
done
```

Expected: both links resolve to the reviewed Tasks skill. Stop rather than overwriting a
different existing path.

- [ ] **Step 3: Verify runtime prerequisites and create the external docs environment**

```bash
set -euo pipefail
command -v git
command -v uv
command -v node
command -v npm
command -v lua
node -e 'if (Number(process.versions.node.split(".")[0]) < 20) process.exit(1)'
git ls-remote https://github.com/chinatsu/pygments HEAD >/dev/null
test ! -e /tmp/tasks-material-docs-environment.root
tasks_docs_root=$(TMPDIR=/tmp mktemp -d)
printf '%s\n' "$tasks_docs_root" >/tmp/tasks-material-docs-environment.root
test -d "$tasks_docs_root"
```

Expected: Node is at least 20, Lua and uv are available, the pinned Pygments Git source
is reachable, and the guarded temporary root is ready. Every Material docs gate sets
`UV_PROJECT_ENVIRONMENT` to its `venv` child so `uv sync` cannot create
`docs/.venv/`.

- [ ] **Step 4: Guard the Material stable and active worktrees**

```bash
set -euo pipefail
repo="$HOME/d/niri-material"
active="$repo/.worktrees/overview-drag-frost"
test "$(git -C "$repo" branch --show-current)" = materials-26.04
test "$(git -C "$repo" rev-parse HEAD)" = 7e94d71af195d5f5062d9b51ec80bca513af8ae3
test "$(git -C "$repo" rev-list --count origin/materials-26.04..HEAD)" = 3
test -z "$(git -C "$repo" status --porcelain=v1)"
test "$(git -C "$active" branch --show-current)" = debug/overview-drag-frost
test "$(git -C "$active" rev-parse HEAD)" = 048814893b9bc4924927468ce1f539b7764d942b
git -C "$active" diff --name-only | sort | cmp - <(printf '%s\n' \
  niri-config/src/lib.rs \
  src/layout/tile.rs \
  src/render_helpers/effect_buffer.rs \
  src/render_helpers/material.rs \
  src/render_helpers/xray.rs | sort)
test -z "$(git -C "$active" diff --cached --name-only)"
test -z "$(git -C "$active" ls-files --others --exclude-standard)"
(
  cd "$active"
  sha256sum \
    niri-config/src/lib.rs \
    src/layout/tile.rs \
    src/render_helpers/effect_buffer.rs \
    src/render_helpers/material.rs \
    src/render_helpers/xray.rs \
    > /tmp/tasks-material-active-worktree.sha256
)
git -C "$repo" worktree list --porcelain
git -C "$repo" check-ignore -q .worktrees
```

Expected: every assertion passes and the checksum file records the five dirty files
without changing them.

- [ ] **Step 5: Guard Prism and inventory all portfolio roots**

```bash
set -euo pipefail
test "$(git -C ~/d/prism branch --show-current)" = main
test "$(git -C ~/d/prism rev-parse HEAD)" = d20111c2182adcbbb2bd3b76356d6e1557cb1e12
test -z "$(git -C ~/d/prism status --porcelain=v1)"
git -C ~/d/prism worktree list --porcelain
git -C ~/d/prism check-ignore -q .worktrees
for repo in \
  ~/d/familiar ~/d/atoms ~/d/beliefs ~/d/nodes \
  ~/d/mindful/v3 ~/d/mindful/v6
do
  test -f "$repo/tasks/.config.toml"
  git -C "$repo" status --short --branch
done
```

Expected: Prism is unchanged at the approved commit, `.worktrees/` is ignored in both
new repositories, and the completed six-project roots remain available.

- [ ] **Step 6: Inspect the normal registry read-only**

```bash
set -euo pipefail
tasks_registry_home=${XDG_CONFIG_HOME:-"$HOME/.config"}
tasks_registry_path="$tasks_registry_home/tasks/projects.toml"
test -f "$tasks_registry_path"
sed -n '1,200p' "$tasks_registry_path"
rg -q '^aut = ' "$tasks_registry_path"
for prefix in fam atoms beliefs nodes mind3 mind6; do
  rg -q "^$prefix = " "$tasks_registry_path"
done
```

Expected: the prior six mappings and unrelated `aut` mapping are visible. A pre-existing
`material` or `prism` mapping must already name the correct canonical stable root or the
migration stops for explicit reconciliation.

### Task 2: Audit and Migrate Material

**Files:**

- Create: `~/d/niri-material/.worktrees/tasks-migration-material/docs/plans/2026-08-31-material-tasks-migration.md`
- Create through CLI: `~/d/niri-material/.worktrees/tasks-migration-material/tasks/.config.toml`
- Create through CLI: `~/d/niri-material/.worktrees/tasks-migration-material/tasks/material-*.md`
- Modify: `~/d/niri-material/.worktrees/tasks-migration-material/.agents/AGENTS.md`
- Modify only when evidence proves drift: `docs/materials/README.md`,
  `docs/materials/2026-08-29-material-backdrop-blur-design.md`,
  `docs/materials/plans/2026-08-28-v1-daily-driver-rollout.md`, and other explicit
  exception paths recorded in the ledger

**Interfaces:**

- Consumes: Task 1's Tasks commit, exact stable HEAD, and active-work checksum.
- Produces: an integrated `material` store, a complete Material ledger, and a stable
  Material checkout available to Prism.

- [ ] **Step 1: Create the Material migration worktree**

```bash
set -euo pipefail
git -C ~/d/niri-material worktree add \
  -b chore/tasks-migration-material \
  ~/d/niri-material/.worktrees/tasks-migration-material \
  materials-26.04
test "$(git -C ~/d/niri-material/.worktrees/tasks-migration-material rev-parse HEAD)" = \
  7e94d71af195d5f5062d9b51ec80bca513af8ae3
test -z "$(git -C ~/d/niri-material/.worktrees/tasks-migration-material status --porcelain=v1)"
(
  cd ~/d/niri-material/.worktrees/overview-drag-frost
  sha256sum --check /tmp/tasks-material-active-worktree.sha256
)
```

Expected: a clean isolated branch and unchanged active debug files.

- [ ] **Step 2: Establish the Material baseline**

```bash
set -euo pipefail
cd ~/d/niri-material/.worktrees/tasks-migration-material
cargo test --all --exclude niri-visual-tests -- --nocapture
cargo clippy --all --all-targets
cargo fmt --all -- --check
tasks_docs_root=$(</tmp/tasks-material-docs-environment.root)
test -d "${tasks_docs_root:?docs environment root unset}"
export UV_PROJECT_ENVIRONMENT="$tasks_docs_root/venv"
(cd docs && uv sync --locked --all-extras --dev && uv run mkdocs build)
test -z "$(git status --porcelain=v1)"
```

Expected: the source-neutral migration gate passes and strict MkDocs validates all wiki
links. Stop on a failure unless the unchanged baseline proves a documented environmental
exception; do not launch a visual client.

- [ ] **Step 3: Audit Git state and classify every Material document**

Read all of `docs/materials/**`, relevant root claims, code/tests/configuration governing
those claims, and the active debug diff. Classify the 83 tracked `docs/wiki/**` files with
the bulk rule; deep-read only Material-relevant wiki exceptions. Record all 111 tracked
docs, the new ledger, `README.md`, and `.agents/AGENTS.md` through the shared ledger
contract.

The candidate table must explicitly settle:

- the active overview-drag frost defect as `create`, status `doing`, owner
  `debug/overview-drag-frost`;
- default-preserves-v1 DRM acceptance as `todo` if it remains outstanding, or `no task`
  with completion evidence;
- noise/saturation follow-up as `idea` only if current evidence establishes a real
  unresolved design question, otherwise `no task`;
- every deployment/burn-in status mismatch and every externally owned outcome.

Do not mutate the active worktree or run live acceptance.

- [ ] **Step 4: Correct drift and prove exact NUL-safe coverage**

Use `apply_patch` for the ledger and evidence-backed current-document corrections. Then:

```bash
set -euo pipefail
cd ~/d/niri-material/.worktrees/tasks-migration-material
ledger=docs/plans/2026-08-31-material-tasks-migration.md
rg -qF '| `docs/wiki/**` | `historical/superseded` |' "$ledger"
rg -qF '| `docs build tooling` | `authority/current` |' "$ledger"
{
  git ls-files -z docs
  printf '%s\0' "$ledger"
  for doc_path in README.md AGENTS.md CLAUDE.md .agents/AGENTS.md; do
    test ! -e "$doc_path" || printf '%s\0' "$doc_path"
  done
} | sort -z -u > /tmp/material-docs.actual
{
  git ls-files -z docs/wiki
  git ls-files -z \
    docs/.gitignore docs/hooks docs/mkdocs.yaml docs/pyproject.toml docs/uv.lock
  sed -n '/^## Document classification$/,/^## /p' "$ledger" |
    awk -F'`' '/^\| `/{if ($2 != "docs/wiki/**" && $2 != "docs build tooling") print $2}' |
    while IFS= read -r doc_path; do printf '%s\0' "$doc_path"; done
} | sort -z -u > /tmp/material-docs.classified
cmp /tmp/material-docs.actual /tmp/material-docs.classified
audit_paths=(docs README.md .agents/AGENTS.md)
rg -n 'Status:|\[ \]|outstanding|incomplete|pending|burn-in|overview.*frost|noise|saturation' \
  "${audit_paths[@]}" || { audit_rg_status=$?; test "$audit_rg_status" -eq 1; }
git diff --check
```

Expected: `cmp` and `git diff --check` produce no output. Every corrected claim's outward
matches are reconciled or explained in the ledger.

- [ ] **Step 5: Run gates and commit the Material documentation reconciliation**

```bash
set -euo pipefail
cd ~/d/niri-material/.worktrees/tasks-migration-material
cargo test --all --exclude niri-visual-tests -- --nocapture
cargo clippy --all --all-targets
cargo fmt --all -- --check
tasks_docs_root=$(</tmp/tasks-material-docs-environment.root)
test -d "${tasks_docs_root:?docs environment root unset}"
export UV_PROJECT_ENVIRONMENT="$tasks_docs_root/venv"
(cd docs && uv sync --locked --all-extras --dev && uv run mkdocs build)
git add docs README.md
git diff --cached --check
git diff --cached --name-only
git commit -m "docs: reconcile project status for tasks migration"
git status --short
```

Expected: one documentation-only commit. The staged-name review must contain only the
ledger and evidence-backed drift corrections.

- [ ] **Step 6: Initialize the seven-entry Material migration registry**

```bash
set -euo pipefail
set +e
TASKS_FORMAT=json tasks -C ~/d/niri-material/.worktrees/tasks-migration-material prime \
  >/tmp/material-pre-init.json 2>/tmp/material-pre-init.err
prime_status=$?
set -e
test "$prime_status" -ne 0
rg -q 'no_project|not initialized' /tmp/material-pre-init.err
test ! -e /tmp/tasks-migration-material.registry-path
tasks_migration_config=$(TMPDIR=/tmp mktemp -d)
printf '%s\n' "$tasks_migration_config" >/tmp/tasks-migration-material.registry-path
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
tasks -C ~/d/familiar init --prefix fam
tasks -C ~/d/atoms init --prefix atoms
tasks -C ~/d/beliefs init --prefix beliefs
tasks -C ~/d/nodes init --prefix nodes
tasks -C ~/d/mindful/v3 init --prefix mind3
tasks -C ~/d/mindful/v6 init --prefix mind6
tasks -C ~/d/niri-material/.worktrees/tasks-migration-material init --prefix material
test "$(rg -c '^[a-z][a-z0-9]* = ' "$tasks_migration_config/tasks/projects.toml")" -eq 7
tasks -C ~/d/niri-material/.worktrees/tasks-migration-material prime |
  jq -e '.prefix == "material" and .warnings == []'
```

Expected: the uninitialized check fails explicitly, then the CLI creates exactly seven
registry mappings and a `material` project.

- [ ] **Step 7: Create the active overview task and every other reviewed outcome**

Use `apply_patch` to create three controls from the reviewed overview candidate row:

- `/tmp/tasks-material-overview.title` contains its literal title;
- `/tmp/tasks-material-overview.size` contains its literal size token;
- `/tmp/tasks-material-overview.body` contains its complete literal task body with no
  trailing spaces or tabs.

Then create the task from those reviewed values:

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-material.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
cd ~/d/niri-material/.worktrees/tasks-migration-material
overview_title=$(</tmp/tasks-material-overview.title)
overview_size=$(</tmp/tasks-material-overview.size)
overview_body=$(</tmp/tasks-material-overview.body)
test -n "${overview_title:?overview title unset}"
case "${overview_size:?overview size unset}" in xs|s|m|l|xl) ;; *) exit 1 ;; esac
test -n "${overview_body:?overview body unset}"
overview_id=$(tasks add "$overview_title" \
  --status todo --size "$overview_size" --tag migration --body "$overview_body" |
  jq -er '.id')
TASKS_OWNER=debug/overview-drag-frost tasks start "$overview_id" |
  jq -e --arg id "$overview_id" '.id == $id'
tasks show "$overview_id" | jq -e \
  --arg id "$overview_id" \
  --arg title "$overview_title" \
  --arg size "$overview_size" \
  --arg body "$overview_body" \
  '.task.id == $id and .task.title == $title and .task.size == $size and .task.body == $body and .task.status == "doing" and .task.owner == "debug/overview-drag-frost" and (.task.tags | index("migration")) != null'
rm -- \
  /tmp/tasks-material-overview.title \
  /tmp/tasks-material-overview.size \
  /tmp/tasks-material-overview.body
```

Record `overview_id` in the exact candidate row with `apply_patch`. Apply the shared
task-creation procedure to every other row marked `create`; do not create the DRM or
noise/saturation task unless Step 3's current evidence supports that disposition. Add
already-resolvable blockers to the six baseline projects. Record a Material-to-Prism
blocker only as a `pending` ledger row. Append the shared Tasks guidance to
`.agents/AGENTS.md` with `apply_patch`.

- [ ] **Step 8: Verify and commit the Material Tasks store**

Rerun Step 4's exact coverage comparison after the ledger and guidance updates, then:

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-material.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
cd ~/d/niri-material/.worktrees/tasks-migration-material
tasks check >/tmp/material-check.json
jq -e '.errors == [] and .warnings == []' /tmp/material-check.json
tasks prime | jq -e '.prefix == "material" and .warnings == []'
tasks ready | jq -e '.warnings == []'
test ! -e tasks/projects.toml
git diff --check
git add tasks .agents/AGENTS.md docs/plans/2026-08-31-material-tasks-migration.md
git diff --cached --check
git commit -m "chore(tasks): initialize project task tracking"
git status --short
```

Expected: one Tasks/guidance/ledger commit and no relative registry file.

- [ ] **Step 9: Independently review both Material commits**

Review document evidence, exact coverage, every task field, ownership, dependency
semantics, the active-work fingerprint, and the two-commit diff against the design. Fix
documentation with `apply_patch` and task state only through the CLI under the same
temporary registry. Rerun Steps 2, 4, and 8 after fixes; use conventional fix commits
before integration rather than rewriting unrelated history.

- [ ] **Step 10: Integrate, register, finalize, and clean up Material**

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-material.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
git -C ~/d/niri-material merge --ff-only chore/tasks-migration-material
cd ~/d/niri-material
cargo test --all --exclude niri-visual-tests -- --nocapture
cargo clippy --all --all-targets
cargo fmt --all -- --check
tasks_docs_root=$(</tmp/tasks-material-docs-environment.root)
test -d "${tasks_docs_root:?docs environment root unset}"
export UV_PROJECT_ENVIRONMENT="$tasks_docs_root/venv"
(cd docs && uv sync --locked --all-extras --dev && uv run mkdocs build)
TASKS_FORMAT=json tasks init --prefix material |
  jq -e '.prefix == "material" and .warnings == []'
TASKS_FORMAT=json tasks init --prefix material |
  jq -e '.prefix == "material" and .warnings == []'
TASKS_FORMAT=json tasks check >/tmp/material-stable-check.json
jq -e '.errors == [] and .warnings == []' /tmp/material-stable-check.json
TASKS_FORMAT=json tasks prime | jq -e '.prefix == "material"'
TASKS_FORMAT=json tasks ready
(
  cd .worktrees/overview-drag-frost
  sha256sum --check /tmp/tasks-material-active-worktree.sha256
  git diff --name-only | sort | cmp - <(printf '%s\n' \
    niri-config/src/lib.rs \
    src/layout/tile.rs \
    src/render_helpers/effect_buffer.rs \
    src/render_helpers/material.rs \
    src/render_helpers/xray.rs | sort)
  test -z "$(git diff --cached --name-only)"
  test -z "$(git ls-files --others --exclude-standard)"
)
git status --short --branch
```

Keep the migration worktree and temporary registry. Use `apply_patch` there to record
the exact integrated commits and stable-gate/registration evidence. If no deferred Prism
row exists, mark the ledger complete and `historical/superseded`; otherwise record initial
integration, retain `active delivery`, and name Task 4 as the closure path. Rerun Step 4's
coverage check and `git diff --check`, then:

```bash
set -euo pipefail
cd ~/d/niri-material/.worktrees/tasks-migration-material
git add docs/plans/2026-08-31-material-tasks-migration.md
git diff --cached --check
git commit -m "docs: record tasks migration integration"
git status --short
```

Independently review this ledger-only diff and correct any finding before the second
fast-forward. Then:

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-material.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
git -C ~/d/niri-material merge --ff-only chore/tasks-migration-material
(
  cd ~/d/niri-material/.worktrees/overview-drag-frost
  sha256sum --check /tmp/tasks-material-active-worktree.sha256
  git diff --name-only | sort | cmp - <(printf '%s\n' \
    niri-config/src/lib.rs \
    src/layout/tile.rs \
    src/render_helpers/effect_buffer.rs \
    src/render_helpers/material.rs \
    src/render_helpers/xray.rs | sort)
  test -z "$(git diff --cached --name-only)"
  test -z "$(git ls-files --others --exclude-standard)"
)
git -C ~/d/niri-material worktree remove ~/d/niri-material/.worktrees/tasks-migration-material
git -C ~/d/niri-material branch -d chore/tasks-migration-material
case "$tasks_migration_config" in /tmp/*) rm -r -- "$tasks_migration_config" ;; *) exit 1 ;; esac
rm -- /tmp/tasks-migration-material.registry-path
git -C ~/d/niri-material status --short --branch
```

Expected: Material contains the three required reviewed migration commits plus any
review-fix commits, the normal registry maps `material` to the stable checkout, the active
debug work is unchanged, and only its pre-existing five dirty files remain in that
separate worktree.

### Task 3: Audit and Migrate Prism

**Files:**

- Create: `~/d/prism/.worktrees/tasks-migration-prism/docs/plans/2026-08-31-prism-tasks-migration.md`
- Create through CLI: `~/d/prism/.worktrees/tasks-migration-prism/tasks/.config.toml`
- Create through CLI when outcomes remain: `~/d/prism/.worktrees/tasks-migration-prism/tasks/prism-*.md`
- Create: `~/d/prism/.worktrees/tasks-migration-prism/AGENTS.md`
- Modify only when evidence proves drift: `README.md` and the 11 tracked files under
  `docs/`, especially the native-material-sink and debug-backdrop specs/plans

**Interfaces:**

- Consumes: integrated Material and the completed six-project stores.
- Produces: an integrated `prism` store and complete Prism ledger with all Material
  dependencies resolved during creation.

- [ ] **Step 1: Create the Prism migration worktree and baseline**

```bash
set -euo pipefail
git -C ~/d/prism worktree add \
  -b chore/tasks-migration-prism \
  ~/d/prism/.worktrees/tasks-migration-prism \
  main
test "$(git -C ~/d/prism/.worktrees/tasks-migration-prism rev-parse HEAD)" = \
  d20111c2182adcbbb2bd3b76356d6e1557cb1e12
cd ~/d/prism/.worktrees/tasks-migration-prism
command -v node
command -v npm
command -v lua
node -e 'if (Number(process.versions.node.split(".")[0]) < 20) process.exit(1)'
npm ci
npm test
/usr/lib/qt6/bin/qmllint integrations/debug-backdrop/shell.qml
test -z "$(git status --porcelain=v1)"
```

Expected: Node and Lua tests pass; Qt 6 lint exits zero with exactly
`Type PanelWindow is not creatable. [uncreatable-type]` at
`integrations/debug-backdrop/shell.qml:9:9`. No unresolved `qs.*` warning is part of the
accepted current baseline.

- [ ] **Step 2: Audit all Prism documents and synthesize outcomes**

Deep-audit all 11 tracked docs and `README.md` against code, tests, current Git ancestry,
and the integrated Material interface. Verify every implemented/deployed claim,
`glass.backdropBlur`, unchecked historical demo, native-material acceptance boundary,
and ordered debug-backdrop handoff. Write the Prism ledger with the shared contract.

Completed or Material-owned outcomes receive `no task`. Unsettled Prism-owned delivery
receives `todo`; unresolved design receives `idea`. Any Prism-to-Material blocker names
an existing resolvable `material-*` ID. Prism may validly produce zero task records if
the evidence shows no actionable Prism-owned remainder.

- [ ] **Step 3: Correct drift, prove coverage, and commit documentation**

```bash
set -euo pipefail
cd ~/d/prism/.worktrees/tasks-migration-prism
ledger=docs/plans/2026-08-31-prism-tasks-migration.md
{
  git ls-files -z docs
  printf '%s\0' "$ledger"
  for doc_path in README.md AGENTS.md CLAUDE.md .agents/AGENTS.md; do
    test ! -e "$doc_path" || printf '%s\0' "$doc_path"
  done
} | sort -z -u > /tmp/prism-docs.actual
sed -n '/^## Document classification$/,/^## /p' "$ledger" |
  awk -F'`' '/^\| `/{print $2}' |
  while IFS= read -r doc_path; do printf '%s\0' "$doc_path"; done |
  sort -z -u > /tmp/prism-docs.classified
cmp /tmp/prism-docs.actual /tmp/prism-docs.classified
audit_paths=(docs README.md)
rg -n 'Status:|\[ \]|outstanding|pending|deployed|burn-in|backdropBlur|backdrop-blur' \
  "${audit_paths[@]}" || { audit_rg_status=$?; test "$audit_rg_status" -eq 1; }
command -v node
command -v npm
command -v lua
node -e 'if (Number(process.versions.node.split(".")[0]) < 20) process.exit(1)'
npm ci
npm test
/usr/lib/qt6/bin/qmllint integrations/debug-backdrop/shell.qml
git diff --check
git add docs README.md
git diff --cached --check
git commit -m "docs: reconcile project status for tasks migration"
git status --short
```

Expected: exact coverage, one documentation commit, green tests, and Qt 6 lint exit zero.

- [ ] **Step 4: Initialize the exact-eight Prism migration registry**

```bash
set -euo pipefail
set +e
TASKS_FORMAT=json tasks -C ~/d/prism/.worktrees/tasks-migration-prism prime \
  >/tmp/prism-pre-init.json 2>/tmp/prism-pre-init.err
prime_status=$?
set -e
test "$prime_status" -ne 0
rg -q 'no_project|not initialized' /tmp/prism-pre-init.err
test ! -e /tmp/tasks-migration-prism.registry-path
tasks_migration_config=$(TMPDIR=/tmp mktemp -d)
printf '%s\n' "$tasks_migration_config" >/tmp/tasks-migration-prism.registry-path
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
tasks -C ~/d/familiar init --prefix fam
tasks -C ~/d/atoms init --prefix atoms
tasks -C ~/d/beliefs init --prefix beliefs
tasks -C ~/d/nodes init --prefix nodes
tasks -C ~/d/mindful/v3 init --prefix mind3
tasks -C ~/d/mindful/v6 init --prefix mind6
tasks -C ~/d/niri-material init --prefix material
tasks -C ~/d/prism/.worktrees/tasks-migration-prism init --prefix prism
test "$(rg -c '^[a-z][a-z0-9]* = ' "$tasks_migration_config/tasks/projects.toml")" -eq 8
tasks -C ~/d/prism/.worktrees/tasks-migration-prism prime |
  jq -e '.prefix == "prism" and .warnings == []'
```

- [ ] **Step 5: Create reviewed Prism outcomes and guidance**

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-prism.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
cd ~/d/prism/.worktrees/tasks-migration-prism
```

Apply the shared task-creation procedure to every `create` row. Add every genuine
Prism-to-Material blocker immediately with `tasks dep`; no such edge is deferred. Create
root `AGENTS.md` with the shared guidance and add its `authority/current` classification
row to the ledger. If the candidate table contains no `create` row, retain only
`tasks/.config.toml` and record the evidence-backed zero-task result.

- [ ] **Step 6: Verify and commit the Prism Tasks store**

Rerun Step 3's coverage comparison now that `AGENTS.md` exists, then:

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-prism.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
export XDG_CONFIG_HOME="${tasks_migration_config:?migration registry unset}"
export TASKS_FORMAT=json
cd ~/d/prism/.worktrees/tasks-migration-prism
tasks check >/tmp/prism-check.json
jq -e '.errors == [] and .warnings == []' /tmp/prism-check.json
tasks prime | jq -e '.prefix == "prism" and .warnings == []'
tasks ready | jq -e '.warnings == []'
command -v node
command -v npm
command -v lua
node -e 'if (Number(process.versions.node.split(".")[0]) < 20) process.exit(1)'
npm ci
npm test
/usr/lib/qt6/bin/qmllint integrations/debug-backdrop/shell.qml
test ! -e tasks/projects.toml
git diff --check
git add tasks AGENTS.md docs/plans/2026-08-31-prism-tasks-migration.md
git diff --cached --check
git commit -m "chore(tasks): initialize project task tracking"
git status --short
```

- [ ] **Step 7: Independently review both Prism commits**

Review all status evidence, candidate dispositions, exact coverage, task fields,
cross-project edges, zero-task evidence if applicable, and the two-commit diff. Fix docs
with `apply_patch` and task records only through the CLI. Rerun Steps 1, 3, and 6 after
fixes and use conventional fix commits before integration.

- [ ] **Step 8: Integrate, register, finalize, and clean up Prism**

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-prism.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
git -C ~/d/prism merge --ff-only chore/tasks-migration-prism
cd ~/d/prism
command -v node
command -v npm
command -v lua
node -e 'if (Number(process.versions.node.split(".")[0]) < 20) process.exit(1)'
npm ci
npm test
/usr/lib/qt6/bin/qmllint integrations/debug-backdrop/shell.qml
TASKS_FORMAT=json tasks init --prefix prism |
  jq -e '.prefix == "prism" and .warnings == []'
TASKS_FORMAT=json tasks init --prefix prism |
  jq -e '.prefix == "prism" and .warnings == []'
TASKS_FORMAT=json tasks check >/tmp/prism-stable-check.json
jq -e '.errors == [] and .warnings == []' /tmp/prism-stable-check.json
TASKS_FORMAT=json tasks prime | jq -e '.prefix == "prism"'
TASKS_FORMAT=json tasks ready
test -z "$(git status --porcelain=v1)"
git status --short --branch
```

Use `apply_patch` in the still-open migration worktree to record the exact integrated
commits and stable evidence, mark the Prism migration complete, and classify its ledger
`historical/superseded`. Rerun Step 3's coverage comparison and `git diff --check`, then:

```bash
set -euo pipefail
cd ~/d/prism/.worktrees/tasks-migration-prism
git add docs/plans/2026-08-31-prism-tasks-migration.md
git diff --cached --check
git commit -m "docs: record tasks migration integration"
git status --short
```

Independently review the ledger-only diff, fix findings, and rerun its gates. Then:

```bash
set -euo pipefail
tasks_migration_config=$(</tmp/tasks-migration-prism.registry-path)
test -d "${tasks_migration_config:?migration registry unset}"
git -C ~/d/prism merge --ff-only chore/tasks-migration-prism
git -C ~/d/prism worktree remove ~/d/prism/.worktrees/tasks-migration-prism
git -C ~/d/prism branch -d chore/tasks-migration-prism
case "$tasks_migration_config" in /tmp/*) rm -r -- "$tasks_migration_config" ;; *) exit 1 ;; esac
rm -- /tmp/tasks-migration-prism.registry-path
git -C ~/d/prism status --short --branch
```

Expected: Prism contains the three required reviewed migration commits plus any review
fixes, and its normal mapping resolves the stable checkout.

### Task 4: Reconcile a Deferred Material-to-Prism Dependency

**Files:**

- Modify through CLI only if pending: `~/d/niri-material/.worktrees/tasks-reconciliation-material/tasks/material-*.md`
- Modify only if pending: `~/d/niri-material/.worktrees/tasks-reconciliation-material/docs/plans/2026-08-31-material-tasks-migration.md`

**Interfaces:**

- Consumes: both integrated extension stores and Material's deferred-dependency table.
- Produces: one reviewed reconciliation commit closing every pending Material row, or a
  verified no-op.

- [ ] **Step 1: Build a fresh exact-eight reconciliation registry**

```bash
set -euo pipefail
test ! -e /tmp/tasks-material-prism-reconciliation.registry-path
tasks_portfolio_config=$(TMPDIR=/tmp mktemp -d)
printf '%s\n' "$tasks_portfolio_config" >/tmp/tasks-material-prism-reconciliation.registry-path
export XDG_CONFIG_HOME="${tasks_portfolio_config:?portfolio registry unset}"
export TASKS_FORMAT=json
tasks -C ~/d/familiar init --prefix fam
tasks -C ~/d/atoms init --prefix atoms
tasks -C ~/d/beliefs init --prefix beliefs
tasks -C ~/d/nodes init --prefix nodes
tasks -C ~/d/mindful/v3 init --prefix mind3
tasks -C ~/d/mindful/v6 init --prefix mind6
tasks -C ~/d/niri-material init --prefix material
tasks -C ~/d/prism init --prefix prism
test "$(rg -c '^[a-z][a-z0-9]* = ' "$tasks_portfolio_config/tasks/projects.toml")" -eq 8
```

- [ ] **Step 2: Enumerate pending Material rows**

```bash
set -euo pipefail
ledger=~/d/niri-material/docs/plans/2026-08-31-material-tasks-migration.md
test -r "$ledger"
set +e
pending_rows=$(sed -n '/^## Deferred foreign dependencies$/,/^## /p' "$ledger" |
  rg '^\|.*\| pending \|$')
pending_status=$?
set -e
test "$pending_status" -eq 0 || test "$pending_status" -eq 1
test -z "$pending_rows" || printf '%s\n' "$pending_rows"
```

If no row prints, verify Material's status and self-classification are complete and
`historical/superseded`, then skip to Step 6. Otherwise continue; every printed edge must
be reconciled in the one Material worktree.

- [ ] **Step 3: Create and baseline the Material reconciliation worktree**

```bash
set -euo pipefail
git -C ~/d/niri-material worktree add \
  -b chore/tasks-reconciliation-material \
  ~/d/niri-material/.worktrees/tasks-reconciliation-material \
  materials-26.04
cd ~/d/niri-material/.worktrees/tasks-reconciliation-material
cargo test --all --exclude niri-visual-tests -- --nocapture
cargo clippy --all --all-targets
cargo fmt --all -- --check
tasks_docs_root=$(</tmp/tasks-material-docs-environment.root)
test -d "${tasks_docs_root:?docs environment root unset}"
export UV_PROJECT_ENVIRONMENT="$tasks_docs_root/venv"
(cd docs && uv sync --locked --all-extras --dev && uv run mkdocs build)
test -z "$(git status --porcelain=v1)"
```

- [ ] **Step 4: Add each pending edge and close the ledger**

For each pending row, use `apply_patch` to create
`/tmp/tasks-material-prism-edge.zsh` with exactly two literal assignments, `local_id=`
and `foreign_id=`, copied from the reviewed Material and Prism candidate rows. Source
those assignments in the same invocation that resolves and creates the edge:

```bash
set -euo pipefail
source /tmp/tasks-material-prism-edge.zsh
tasks_portfolio_config=$(</tmp/tasks-material-prism-reconciliation.registry-path)
test -d "${tasks_portfolio_config:?portfolio registry unset}"
export XDG_CONFIG_HOME="${tasks_portfolio_config:?portfolio registry unset}"
export TASKS_FORMAT=json
test -n "${local_id:?local task ID unset}"
test -n "${foreign_id:?foreign task ID unset}"
tasks -C ~/d/niri-material/.worktrees/tasks-reconciliation-material show "$local_id" |
  jq -e --arg id "$local_id" '.task.id == $id'
tasks -C ~/d/prism show "$foreign_id" |
  jq -e --arg id "$foreign_id" '.task.id == $id'
TASKS_OWNER=migration tasks \
  -C ~/d/niri-material/.worktrees/tasks-reconciliation-material \
  dep "$local_id" --on "$foreign_id" |
  jq -e --arg id "$local_id" '.id == $id'
rm -- /tmp/tasks-material-prism-edge.zsh
```

If resolution or acyclicity fails, stop. Use `apply_patch` to change that ledger row to
`reconciled`, record the exact IDs and verification, mark the migration complete, and
classify the ledger `historical/superseded` after the final row.

- [ ] **Step 5: Verify, commit, review, and integrate reconciliation**

```bash
set -euo pipefail
tasks_portfolio_config=$(</tmp/tasks-material-prism-reconciliation.registry-path)
test -d "${tasks_portfolio_config:?portfolio registry unset}"
export XDG_CONFIG_HOME="${tasks_portfolio_config:?portfolio registry unset}"
export TASKS_FORMAT=json
cd ~/d/niri-material/.worktrees/tasks-reconciliation-material
tasks check >/tmp/material-reconciliation-check.json
jq -e '.errors == [] and .warnings == []' /tmp/material-reconciliation-check.json
cargo test --all --exclude niri-visual-tests -- --nocapture
cargo clippy --all --all-targets
cargo fmt --all -- --check
tasks_docs_root=$(</tmp/tasks-material-docs-environment.root)
test -d "${tasks_docs_root:?docs environment root unset}"
export UV_PROJECT_ENVIRONMENT="$tasks_docs_root/venv"
(cd docs && uv sync --locked --all-extras --dev && uv run mkdocs build)
ledger=docs/plans/2026-08-31-material-tasks-migration.md
rg -qF '| `docs/wiki/**` | `historical/superseded` |' "$ledger"
rg -qF '| `docs build tooling` | `authority/current` |' "$ledger"
{
  git ls-files -z docs
  printf '%s\0' "$ledger"
  for doc_path in README.md AGENTS.md CLAUDE.md .agents/AGENTS.md; do
    test ! -e "$doc_path" || printf '%s\0' "$doc_path"
  done
} | sort -z -u > /tmp/material-docs.actual
{
  git ls-files -z docs/wiki
  git ls-files -z \
    docs/.gitignore docs/hooks docs/mkdocs.yaml docs/pyproject.toml docs/uv.lock
  sed -n '/^## Document classification$/,/^## /p' "$ledger" |
    awk -F'`' '/^\| `/{if ($2 != "docs/wiki/**" && $2 != "docs build tooling") print $2}' |
    while IFS= read -r doc_path; do printf '%s\0' "$doc_path"; done
} | sort -z -u > /tmp/material-docs.classified
cmp /tmp/material-docs.actual /tmp/material-docs.classified
git diff --check
git add tasks docs/plans/2026-08-31-material-tasks-migration.md
git diff --cached --check
git commit -m "chore(tasks): reconcile cross-project dependencies"
git status --short
```

Independently review the edge evidence, reachable graph, ledger closure, and commit diff.
Fix findings through the CLI or `apply_patch`, rerun the full step, then:

```bash
set -euo pipefail
tasks_portfolio_config=$(</tmp/tasks-material-prism-reconciliation.registry-path)
test -d "${tasks_portfolio_config:?portfolio registry unset}"
export XDG_CONFIG_HOME="${tasks_portfolio_config:?portfolio registry unset}"
export TASKS_FORMAT=json
git -C ~/d/niri-material merge --ff-only chore/tasks-reconciliation-material
cd ~/d/niri-material
cargo test --all --exclude niri-visual-tests -- --nocapture
cargo clippy --all --all-targets
cargo fmt --all -- --check
tasks_docs_root=$(</tmp/tasks-material-docs-environment.root)
test -d "${tasks_docs_root:?docs environment root unset}"
export UV_PROJECT_ENVIRONMENT="$tasks_docs_root/venv"
(cd docs && uv sync --locked --all-extras --dev && uv run mkdocs build)
tasks -C ~/d/niri-material check >/tmp/material-reconciliation-stable-check.json
jq -e '.errors == [] and .warnings == []' /tmp/material-reconciliation-stable-check.json
(
  cd .worktrees/overview-drag-frost
  sha256sum --check /tmp/tasks-material-active-worktree.sha256
  git diff --name-only | sort | cmp - <(printf '%s\n' \
    niri-config/src/lib.rs \
    src/layout/tile.rs \
    src/render_helpers/effect_buffer.rs \
    src/render_helpers/material.rs \
    src/render_helpers/xray.rs | sort)
  test -z "$(git diff --cached --name-only)"
  test -z "$(git ls-files --others --exclude-standard)"
)
git -C ~/d/niri-material worktree remove ~/d/niri-material/.worktrees/tasks-reconciliation-material
git -C ~/d/niri-material branch -d chore/tasks-reconciliation-material
```

- [ ] **Step 6: Prove reconciliation complete and clean controls**

```bash
set -euo pipefail
ledger=~/d/niri-material/docs/plans/2026-08-31-material-tasks-migration.md
test -r "$ledger"
set +e
sed -n '/^## Deferred foreign dependencies$/,/^## /p' "$ledger" |
  rg '^\|.*\| pending \|$'
pending_status=$?
set -e
test "$pending_status" -eq 1
rg -q 'historical/superseded' "$ledger"
tasks_portfolio_config=$(</tmp/tasks-material-prism-reconciliation.registry-path)
test -d "${tasks_portfolio_config:?portfolio registry unset}"
case "$tasks_portfolio_config" in /tmp/*) rm -r -- "$tasks_portfolio_config" ;; *) exit 1 ;; esac
rm -- /tmp/tasks-material-prism-reconciliation.registry-path
test ! -e /tmp/tasks-material-prism-edge.zsh || rm -- /tmp/tasks-material-prism-edge.zsh
```

### Task 5: Run the Exact-Eight Gate and Record Completion

**Files:**

- Modify: `docs/specs/2026-08-31-material-prism-tasks-migration-design.md`
- Modify: `docs/plans/2026-08-31-material-prism-tasks-migration.md`
- Inspect: both new ledgers/stores/guidance files and all eight stable task stores

**Interfaces:**

- Consumes: eight integrated projects with no pending dependency row.
- Produces: final portfolio evidence and truthful implemented status in this design and
  plan.

Execution evidence from 2026-09-01:

- the fresh registry contained exactly `fam`, `atoms`, `beliefs`, `nodes`, `mind3`,
  `mind6`, `material`, and `prism`; all eight `check`, `prime`, and `ready` commands
  reported no warnings, and the global list returned 41 tasks with no warnings;
- exact ledger coverage was 114/114 paths for Material and 14/14 for Prism, with no
  pending dependency row; CLI inspection matched all five created tasks to their
  reviewed candidate rows, including `material-e88df7` as `doing` under
  `debug/overview-drag-frost` and `material-cad932` as the sole unresolved `idea`;
- Material tests, clippy, and strict MkDocs completed successfully. Stable rustfmt
  1.9.0 exited 1 only for untouched `src/protocols/foreign_toplevel.rs`; its
  pre-migration, integrated, and current SHA-256 is
  `41891c1ce0b3e4e5c59f51db9f82009ab2a677b12f50a8a2d9b60e443652ce50`;
- Prism's 140 tests passed and `qmllint` exited zero with exactly the accepted
  `Type PanelWindow is not creatable. [uncreatable-type]` warning at line 9:9; the
  Tasks suite passed all 65 tests;
- normal `material` and `prism` registration was already canonical and idempotent;
  the normal registry checksum and `aut` mapping were unchanged, and the active
  Material worktree passed its five-file checksum and dirty-state guards.
- on 2026-09-01, reviewed completion commit `ced9c4559b188c9f2af7e03d12d4fb44a18985bb`
  was fast-forwarded to Tasks `main`; all 65 Tasks tests passed there, the completion
  worktree and merged branch were removed, and the guarded final registry, shared docs
  environment, active-work checksum, and every planned temporary output were removed.

- [x] **Step 1: Build a fresh exact-eight final registry**

```bash
set -euo pipefail
test ! -e /tmp/tasks-material-prism-final.registry-path
tasks_portfolio_config=$(TMPDIR=/tmp mktemp -d)
printf '%s\n' "$tasks_portfolio_config" >/tmp/tasks-material-prism-final.registry-path
export XDG_CONFIG_HOME="${tasks_portfolio_config:?portfolio registry unset}"
export TASKS_FORMAT=json
tasks -C ~/d/familiar init --prefix fam
tasks -C ~/d/atoms init --prefix atoms
tasks -C ~/d/beliefs init --prefix beliefs
tasks -C ~/d/nodes init --prefix nodes
tasks -C ~/d/mindful/v3 init --prefix mind3
tasks -C ~/d/mindful/v6 init --prefix mind6
tasks -C ~/d/niri-material init --prefix material
tasks -C ~/d/prism init --prefix prism
test "$(rg -c '^[a-z][a-z0-9]* = ' "$tasks_portfolio_config/tasks/projects.toml")" -eq 8
```

- [x] **Step 2: Require warning-free checks and exact prefixes in all eight projects**

```bash
set -euo pipefail
tasks_portfolio_config=$(</tmp/tasks-material-prism-final.registry-path)
test -d "${tasks_portfolio_config:?portfolio registry unset}"
export XDG_CONFIG_HOME="${tasks_portfolio_config:?portfolio registry unset}"
export TASKS_FORMAT=json
while read -r root prefix; do
  tasks -C "$root" check >"/tmp/$prefix-final-check.json"
  jq -e '.errors == [] and .warnings == []' "/tmp/$prefix-final-check.json"
  tasks -C "$root" prime | jq -e --arg prefix "$prefix" '.prefix == $prefix and .warnings == []'
  tasks -C "$root" ready | jq -e '.warnings == []'
done <<EOF
$HOME/d/familiar fam
$HOME/d/atoms atoms
$HOME/d/beliefs beliefs
$HOME/d/nodes nodes
$HOME/d/mindful/v3 mind3
$HOME/d/mindful/v6 mind6
$HOME/d/niri-material material
$HOME/d/prism prism
EOF
```

- [x] **Step 3: Require global listing success and no warnings**

```bash
set -euo pipefail
tasks_portfolio_config=$(</tmp/tasks-material-prism-final.registry-path)
test -d "${tasks_portfolio_config:?portfolio registry unset}"
export XDG_CONFIG_HOME="${tasks_portfolio_config:?portfolio registry unset}"
export TASKS_FORMAT=json
tasks -C ~/d/familiar list --all-projects \
  >/tmp/tasks-material-prism-list.json \
  2>/tmp/tasks-material-prism-list.err
test ! -s /tmp/tasks-material-prism-list.err
jq -e '.warnings == [] and (.tasks | type == "array")' /tmp/tasks-material-prism-list.json
```

- [x] **Step 4: Audit both ledgers and task semantics**

Run both exact coverage comparisons from the stable checkouts:

```bash
set -euo pipefail
(
  cd ~/d/niri-material
  ledger=docs/plans/2026-08-31-material-tasks-migration.md
  rg -qF '| `docs/wiki/**` | `historical/superseded` |' "$ledger"
  rg -qF '| `docs build tooling` | `authority/current` |' "$ledger"
  {
    git ls-files -z docs
    printf '%s\0' "$ledger"
    for doc_path in README.md AGENTS.md CLAUDE.md .agents/AGENTS.md; do
      test ! -e "$doc_path" || printf '%s\0' "$doc_path"
    done
  } | sort -z -u > /tmp/material-docs.actual
  {
    git ls-files -z docs/wiki
    git ls-files -z \
      docs/.gitignore docs/hooks docs/mkdocs.yaml docs/pyproject.toml docs/uv.lock
    sed -n '/^## Document classification$/,/^## /p' "$ledger" |
      awk -F'`' '/^\| `/{if ($2 != "docs/wiki/**" && $2 != "docs build tooling") print $2}' |
      while IFS= read -r doc_path; do printf '%s\0' "$doc_path"; done
  } | sort -z -u > /tmp/material-docs.classified
  cmp /tmp/material-docs.actual /tmp/material-docs.classified
)
(
  cd ~/d/prism
  ledger=docs/plans/2026-08-31-prism-tasks-migration.md
  {
    git ls-files -z docs
    printf '%s\0' "$ledger"
    for doc_path in README.md AGENTS.md CLAUDE.md .agents/AGENTS.md; do
      test ! -e "$doc_path" || printf '%s\0' "$doc_path"
    done
  } | sort -z -u > /tmp/prism-docs.actual
  sed -n '/^## Document classification$/,/^## /p' "$ledger" |
    awk -F'`' '/^\| `/{print $2}' |
    while IFS= read -r doc_path; do printf '%s\0' "$doc_path"; done |
    sort -z -u > /tmp/prism-docs.classified
  cmp /tmp/prism-docs.actual /tmp/prism-docs.classified
)
```

Then require no pending rows without treating read errors as absence:

```bash
set -euo pipefail
material_ledger=~/d/niri-material/docs/plans/2026-08-31-material-tasks-migration.md
prism_ledger=~/d/prism/docs/plans/2026-08-31-prism-tasks-migration.md
test -r "$material_ledger"
test -r "$prism_ledger"
set +e
rg '^\|.*\| pending \|$' "$material_ledger" "$prism_ledger"
pending_status=$?
set -e
test "$pending_status" -eq 1
rg -q 'historical/superseded' "$material_ledger"
rg -q 'historical/superseded' "$prism_ledger"
```

Inspect every candidate row against its task ID or explicit no-task disposition. Confirm
completed history was not backfilled, every unresolved claim is an `idea`, the overview
task is `doing` with the verified owner, every foreign edge is a real blocker, and both
guidance files contain the shared Tasks section.

- [x] **Step 5: Run both repository gates and the Tasks suite**

```bash
set -euo pipefail
(
  cd ~/d/niri-material
  cargo test --all --exclude niri-visual-tests -- --nocapture
  cargo clippy --all --all-targets
  rustfmt --version | tee /tmp/material-final-rustfmt.version
  set +e
  cargo fmt --all -- --check \
    >/tmp/material-final-fmt.out \
    2>/tmp/material-final-fmt.err
  fmt_status=$?
  set -e
  printf '%s\n' "$fmt_status" >/tmp/material-final-fmt.status
  test "$fmt_status" -eq 1
  cat /tmp/material-final-fmt.out /tmp/material-final-fmt.err \
    >/tmp/material-final-fmt.combined
  test "$(rg -c '^Diff in ' /tmp/material-final-fmt.combined)" -ge 1
  rg -q '^Diff in .*/src/protocols/foreign_toplevel\.rs:' \
    /tmp/material-final-fmt.combined
  if rg '^Diff in ' /tmp/material-final-fmt.combined |
    rg -v '/src/protocols/foreign_toplevel\.rs:'
  then
    exit 1
  fi
  expected=41891c1ce0b3e4e5c59f51db9f82009ab2a677b12f50a8a2d9b60e443652ce50
  test "$(sha256sum src/protocols/foreign_toplevel.rs | cut -d' ' -f1)" = "$expected"
  test "$(git show 7e94d71af195d5f5062d9b51ec80bca513af8ae3:src/protocols/foreign_toplevel.rs | sha256sum | cut -d' ' -f1)" = "$expected"
  test "$(git show 8047b6ca14ec1e2a0760a79f5d9d4883a9fc2519:src/protocols/foreign_toplevel.rs | sha256sum | cut -d' ' -f1)" = "$expected"
  git diff --quiet \
    7e94d71af195d5f5062d9b51ec80bca513af8ae3 \
    8047b6ca14ec1e2a0760a79f5d9d4883a9fc2519 -- \
    src/protocols/foreign_toplevel.rs
  test -z "$(git status --porcelain=v1)"
  tasks_docs_root=$(</tmp/tasks-material-docs-environment.root)
  test -d "${tasks_docs_root:?docs environment root unset}"
  export UV_PROJECT_ENVIRONMENT="$tasks_docs_root/venv"
  (cd docs && uv sync --locked --all-extras --dev && uv run mkdocs build)
)
(
  cd ~/d/prism
  command -v node
  command -v npm
  command -v lua
  node -e 'if (Number(process.versions.node.split(".")[0]) < 20) process.exit(1)'
  npm ci
  npm test
  set +e
  /usr/lib/qt6/bin/qmllint integrations/debug-backdrop/shell.qml \
    >/tmp/prism-final-qmllint.out \
    2>/tmp/prism-final-qmllint.err
  qmllint_status=$?
  set -e
  test "$qmllint_status" -eq 0
  test ! -s /tmp/prism-final-qmllint.out
  cmp /tmp/prism-final-qmllint.err <(printf '%s\n' \
    'Warning: integrations/debug-backdrop/shell.qml:9:9: Type PanelWindow is not creatable. [uncreatable-type]' \
    '        PanelWindow {' \
    '        ^^^^^^^^^^^')
  test -z "$(git status --porcelain=v1)"
)
(
  cd ~/d/tasks
  cargo test
)
(
  cd ~/d/niri-material/.worktrees/overview-drag-frost
  sha256sum --check /tmp/tasks-material-active-worktree.sha256
  git diff --name-only | sort | cmp - <(printf '%s\n' \
    niri-config/src/lib.rs \
    src/layout/tile.rs \
    src/render_helpers/effect_buffer.rs \
    src/render_helpers/material.rs \
    src/render_helpers/xray.rs | sort)
  test -z "$(git diff --cached --name-only)"
  test -z "$(git ls-files --others --exclude-standard)"
)
```

Expected: every executable gate passes, the exact accepted Prism warning is present,
the captured stable-rustfmt nonzero result is limited to the proved unchanged baseline,
and active Material work remains byte-identical.

- [x] **Step 6: Prove normal mappings without touching `aut`**

```bash
set -euo pipefail
final_portfolio_config=$(</tmp/tasks-material-prism-final.registry-path)
test "${XDG_CONFIG_HOME-}" != "${final_portfolio_config:?portfolio registry unset}"
TASKS_FORMAT=json tasks -C ~/d/niri-material init --prefix material |
  jq -e '.prefix == "material" and .warnings == []'
TASKS_FORMAT=json tasks -C ~/d/niri-material prime | jq -e '.prefix == "material"'
TASKS_FORMAT=json tasks -C ~/d/prism init --prefix prism |
  jq -e '.prefix == "prism" and .warnings == []'
TASKS_FORMAT=json tasks -C ~/d/prism prime | jq -e '.prefix == "prism"'
tasks_registry_home=${XDG_CONFIG_HOME:-"$HOME/.config"}
tasks_registry_path="$tasks_registry_home/tasks/projects.toml"
rg -q '^aut = ' "$tasks_registry_path"
```

If a new root moved, stop and back up the normal registry before repair:

```bash
set -euo pipefail
tasks_registry_home=${XDG_CONFIG_HOME:-"$HOME/.config"}
tasks_registry_path="$tasks_registry_home/tasks/projects.toml"
backup_path="$tasks_registry_path.pre-material-prism-repair"
test ! -e "$backup_path"
cp -- "$tasks_registry_path" "$backup_path"
```

Use `apply_patch` to remove only stale `material` or `prism` entries, rerun the four CLI
commands, then repeat Steps 1–3. Never remove or rewrite `aut` or the completed six.

- [x] **Step 7: Create the Tasks completion worktree**

```bash
set -euo pipefail
git -C ~/d/tasks worktree add \
  -b docs/material-prism-tasks-migration-complete \
  ~/d/tasks/.worktrees/material-prism-tasks-migration-complete \
  main
cd ~/d/tasks/.worktrees/material-prism-tasks-migration-complete
```

Expected: a clean worktree whose base already contains this approved design and plan.

- [x] **Step 8: Make the design and plan completion claims truthful**

Use `apply_patch` to record the actual implementation date, integrated commit evidence,
and checked steps whose commands ran. Change the design status to implemented only after
Steps 1–6 pass. Search outward:

```bash
set -euo pipefail
cd ~/d/tasks/.worktrees/material-prism-tasks-migration-complete
audit_paths=(docs)
for f in README.md AGENTS.md CLAUDE.md; do test ! -f "$f" || audit_paths+=("$f"); done
rg -n 'material.prism.*migration|implementation has not started|migration.*pending' \
  "${audit_paths[@]}" || { audit_rg_status=$?; test "$audit_rg_status" -eq 1; }
git diff --check
```

Correct propagated current-document drift in the same commit. Do not check a plan box
from an earlier claim; require command evidence from this execution.

- [x] **Step 9: Verify, commit, review, integrate, and clean up**

```bash
set -euo pipefail
cd ~/d/tasks/.worktrees/material-prism-tasks-migration-complete
cargo test
git add docs
for f in README.md AGENTS.md CLAUDE.md; do test ! -e "$f" || git add "$f"; done
git diff --cached --check
git commit -m "docs: record material and prism task migrations complete"
git status --short
```

Independently review every completion criterion against the eight stable trees and fresh
portfolio outputs. Fix findings and rerun affected gates before integration. Then:

```bash
set -euo pipefail
tasks_portfolio_config=$(</tmp/tasks-material-prism-final.registry-path)
test -d "${tasks_portfolio_config:?portfolio registry unset}"
git -C ~/d/tasks merge --ff-only docs/material-prism-tasks-migration-complete
cd ~/d/tasks
cargo test
git status --short --branch
git worktree remove ~/d/tasks/.worktrees/material-prism-tasks-migration-complete
git branch -d docs/material-prism-tasks-migration-complete
case "$tasks_portfolio_config" in /tmp/*) rm -r -- "$tasks_portfolio_config" ;; *) exit 1 ;; esac
rm -- /tmp/tasks-material-prism-final.registry-path
tasks_docs_root=$(</tmp/tasks-material-docs-environment.root)
test -d "${tasks_docs_root:?docs environment root unset}"
case "$tasks_docs_root" in /tmp/*) rm -r -- "$tasks_docs_root" ;; *) exit 1 ;; esac
rm -- /tmp/tasks-material-docs-environment.root
rm -- /tmp/tasks-material-active-worktree.sha256
for output_path in \
  /tmp/material-pre-init.json /tmp/material-pre-init.err \
  /tmp/material-docs.actual /tmp/material-docs.classified \
  /tmp/material-check.json /tmp/material-stable-check.json \
  /tmp/prism-pre-init.json /tmp/prism-pre-init.err \
  /tmp/prism-docs.actual /tmp/prism-docs.classified \
  /tmp/prism-check.json /tmp/prism-stable-check.json \
  /tmp/material-reconciliation-check.json \
  /tmp/material-reconciliation-stable-check.json \
  /tmp/tasks-material-prism-list.json /tmp/tasks-material-prism-list.err \
  /tmp/tasks-material-overview.title /tmp/tasks-material-overview.size \
  /tmp/tasks-material-overview.body
do
  test ! -e "$output_path" || rm -- "$output_path"
done
for prefix in fam atoms beliefs nodes mind3 mind6 material prism; do
  output_path="/tmp/$prefix-final-check.json"
  test ! -e "$output_path" || rm -- "$output_path"
done
```

Expected: Tasks `main` truthfully records the completed extension, all temporary controls
and migration worktrees are removed, and no live or unrelated project state changed.
