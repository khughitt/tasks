# Lanes, needs, and project groups — design

**Status:** draft, 2026-10-03. Task: tasks-ece1e2. Waiting ideas: tasks-9bdd68 (focus
marker), tasks-77dbc6 (project groups), tasks-e02860 (lanes and shared resources).
Brief: `docs/notes/2026-09-30-work-selection-brief.md`.

## 1. Outcome

A project can say, inside the tracker, which efforts run side by side and what makes them
contend. Anyone who runs `prime` sees the lanes, what each is for, and the step each could
take now. The picker stops handing out a step whose shared resource is already in use.
Several projects can be read together as a named group.

Three additions, each usable alone:

- **Lanes:** a goal marked `lane: true` is an effort meant to proceed alongside other
  lanes. `prime` and the new `tasks lanes` show each lane with its guidance and its next
  step.
- **Needs:** a task lists the shared resources its work uses (`needs: [quiet]`), drawn
  from a vocabulary the project declares. A need declared `exclusive` serves one live
  claim at a time, so a step needing it waits while another session holds it.
- **Project groups:** a named set of registered projects, accepted by `--group <name>`
  wherever `--all-projects` is accepted today.

## 2. Problem

The evidence comes from tasks-e02860. A project with about 70 open tasks was split into six
lanes, so that captures waiting for an idle host would not stall unrelated work. The tracker
could not hold that organisation, so it went into a notes document, made up of:

- a lane table;
- each lane's first milestone;
- the cross-lane constraints;
- ad hoc `needs-quiet`, `needs-nested` and `needs-owner` tags.

The organisation was invisible from `prime`, `ready` and `next`. A session that starts
from the picker never sees it, and so falls back to choosing by priority alone.

The tool can already express part of this:

- Goals group tasks. Dependencies sequence them.
- `parallel` marks a task as safe to run beside other marked tasks.
- A quiet park records that a parked run needs an idle host.

The tool cannot express three things:

- **An effort as a unit of concurrency.** `parallel` is per task, and its design
  (`2026-09-06-parallel-candidates-design.md`) rejected a lane *label on every task*
  because it taxed every `add`.
- **What actually serialises two efforts.** Two efforts can run side by side unless they
  compete for something only one can use at a time: an idle host, a person's review, a
  device. Marking pairs of lanes as parallel scales as n² and goes stale. Declaring what
  each step uses lets the tool work out which lanes can run now.
- **Selection across a subset of projects.** The only choices are one project or all of
  them (tasks-77dbc6).

## 3. Lanes

### 3.1 The field

`lane: true` is a new optional frontmatter key. It is written only when true, placed after
`parallel` in `KEYS`, and set by `add --lane` and `edit --lane` / `--no-lane`. It follows the
§3.4 field checklist of `2026-08-29-tasks-design.md`.

A lane is a goal with a role. It is not a new kind of record. A field fits better than a
reserved tag here: the value is validated, and the tag set stays free-form. The `parallel`
design made the same choice.

The parallel design rejected a label on every task. This design does not reintroduce that
label: membership comes from the existing `parent` tree, so only the lane goal itself is
marked.

### 3.2 Validation

- **No nested lanes.** A lane has no ancestor that is a lane (`hierarchy.rs`). A lane inside
  a lane would make "one pick per lane" ambiguous. Sub-efforts inside a lane are ordinary
  child goals.
- **A lane is never ready itself,** even with no children. Readiness treats `lane: true` as
  a goal. A childless lane is reported by the lanes view as `empty` (§5) and by `check` as a
  warning, not an error.
- **Closeout is unchanged.** It already needs at least one child and no open descendant.
- **Pausing a lane** uses the existing `shelve`. A shelved lane leaves the lanes view.

### 3.3 Guidance

A lane's **guidance** is the first paragraph of its body. That runs up to the first blank
line, skipping a leading `#` heading line if there is one. It is meant to say why the lane
exists and what its first milestone is.

No new field holds it. The body is already where a goal says what it is for, and the skill
tells authors to lead with that paragraph (§9). The JSON carries the paragraph whole;
pretty output prints its first line, cut to the table width.

### 3.4 Order

Lanes are listed in ready order of the lane goals: priority, size, created, id
(`query::ready_order`). A lane's priority is therefore its rank. Raising one lane's priority
is how a person says "this effort first", which covers the single-focus idea (§7).

## 4. Needs

### 4.1 The vocabulary

A project declares its needs in `tasks/.config.toml`:

```toml
[needs.quiet]
meaning = "an idle host: a TTY with the desktop stopped"
exclusive = true

[needs.owner]
meaning = "the owner supplies or judges an image"
```

- Each table uses `deny_unknown_fields`.
- `meaning` is one required line.
- `exclusive` is optional and defaults to false.
- Need names use the tag grammar (lowercase, digits, `-`).

The vocabulary is committed and syncs between hosts, like `[tags]` and `[feedback]`. It
is the project's own: the tool names no resources.

### 4.2 The field

`needs: [quiet, owner]` is a new optional frontmatter list. It is written only when
non-empty and placed after `lane`. Two places set it:

- `add --need <n>`, repeatable.
- `edit --need <n>` (appends), `--rm-need <n>` and `--no-needs`, mirroring the tag flags.

Every name must be declared in the project's vocabulary. Validation happens at three points:

- **On write:** `add` and `edit` refuse an undeclared name with the error kind
  `unknown_need`.
- **On `check`:** a record naming a need that is no longer declared is an error. The
  vocabulary changed under it, so the record must be fixed.
- **On read:** the need is carried as-is. A worktree on an older config can still list
  tasks.

### 4.3 Naming

The quiet park already has a `needs` recipe (`idle` or `headless`). That field says what
"idle" means for one parked run. The task field says which shared resources a step
contends for.

The two are kept apart:

- the park field stays `park.needs`, nested under `park` in JSON;
- the task field is top-level `needs`.

Merging them is left to the dated-resumption design (tasks-157d05) or later (§11).

### 4.4 Exclusive holds

A task **holds** its exclusive needs while it has a **live claim**. A `doing` status alone
does not hold, and neither does a park. A run parked while waiting for the quiet host holds
nothing until a session claims it again.

At `start`, the claim entry gains `holds: [quiet]`. The list is the task's needs that its
own project declares exclusive. Recording the holds in the claim lets any view see them by
reading the claim stores alone, across every registered project, without opening another
checkout. This matters because an idle host is shared by every project on the machine.

A hold matches by name. A task in project B is held back when:

- B declares the need exclusive, and
- any live claim on this host, in any project, holds a need of that name.

An older binary that saves the claim store drops `holds`, because the store ignores
unknown keys. The effect is a weaker gate, never a false block. Every host runs the same
installed build, so this is documented, not engineered around.

### 4.5 Where holds and needs gate

- **`ready`, `next`, the `ready` list in `prime`, and the lanes view** drop a task whose
  exclusive need is held by another task's live claim. Each dropped task gets a warning:
  `<id> waits for <need>, held by <holder id> (<session>)`. Ordering is unchanged.
  Parked-agent candidates in `next` go through the same gate.
- **`start`** refuses a task whose exclusive need is held, with the error kind `need_held`.
  This is the halt precedent: `--force --reason "<why>"` overrides. The override writes a
  note on the started task, `need override: started while <need> held by <holder>: <reason>`,
  and a matching note on the holder.
- **`--without <need>`** (repeatable) on `ready`, `next`, `prime` and `lanes` hides tasks
  that need it. The environment variable `TASKS_WITHOUT` (comma-separated) supplies the
  same set for a whole session, like `TASKS_MAX_COMPLEXITY`. A session on a host in
  ordinary use sets `TASKS_WITHOUT=quiet`. A name that no project in scope declares is
  rejected with `unknown_need`.
- **`list --need <n>`** and **`ready --need <n>`** filter on the field, using the shared
  `TaskFilter` all-of rule, like `--tag`.

Needs never change eligibility in any other way. A task that needs `owner` is still ready.
The need tells whoever picks it what the step will ask of them, and `--without owner` hides
it from a session that is running unattended.

## 5. The lanes view

`tasks lanes [--without <n>]... [--max-complexity <c>] [scope]` and the new `lanes` key of
`prime` share one builder.

For each open, unshelved lane in scope, in lane order (§3.4), the builder:

1. **Collects steps.** The lane's descendants that `next` would consider: parked-agent
   candidates in the subtree first (newest park first), then `ready_tasks` rows in ready
   order. Every existing gate applies: dependencies, claims, user-waiting parks, defer,
   halt, the complexity cutoff, and `--without`.
2. **Collects active work.** Descendants with a live claim.
3. **Picks one step.** The pick is the first step whose exclusive needs are not held:
   - not by a live claim, and
   - not by the pick of an earlier lane in this same view.

   The picks across lanes form a set of steps that can run at the same time, as far as
   declared needs can tell. Earlier lanes win contested resources because lane order is
   the person's ranking.
4. **Assigns a state:**

   | State | Meaning |
   |---|---|
   | `ready` | a pick exists |
   | `held` | steps exist, but every one waits for a held or already-picked exclusive need; the row names the need and its holder |
   | `waiting` | no step is eligible; the row counts open descendants by cause: blocked, deferred, parked on a person, dependency-held |
   | `empty` | no open descendant |

   Active work is shown in every state. A lane can be `ready` while another of its tasks
   is already claimed.

**Pretty `prime`** prints a `lanes:` block before `roadmap:`, one row per lane: id,
priority, title, then the state and the pick (`→ <id> <title>`) or the reason it is held.
**`tasks lanes --pretty`** adds the guidance line and the active claims under each row.

**JSON:** `prime` gains `lanes: [LaneRow]`, always present and empty when the project has
no lanes. `tasks lanes` returns `{lanes: [LaneRow], warnings}`. The `LaneRow` shape:

```json
{"lane": TaskSummary, "guidance": "…", "state": "ready",
 "pick": TaskSummary | null,
 "steps": 3,
 "active": [TaskSummary],
 "held": [{"id": "…", "need": "quiet", "holder": "…"}],
 "waiting": {"blocked": 0, "deferred": 1, "user": 2, "depends": 0}}
```

`held` and `waiting` are omitted when empty, following the sparse rule.

Roadmap rows for lane goals carry a `≡` marker in pretty output. `TaskSummary` gains `lane`
and `needs`, sparse like `parallel`.

**`--under <ref>`** is a new shared filter for `list` and `ready`: descendants of a task, at
any depth. `ready --under <lane>` is the full list for one lane. `--parent` keeps its
direct-child meaning.

The picker does not reorder by lane. `next` remains: parked-agent resumes, then ready order,
with the gates of §4.5 applied. Priority is still the urgency signal, and an urgent bug
outside every lane still wins. The lanes view answers a different question: what to do in
parallel.

## 6. Project groups

Groups are declared in the registry, beside `projects` and `aliases`:

```toml
[groups]
verifiably = ["nodes", "atoms", "beliefs"]
```

### 6.1 Commands

- `tasks group set <name> <prefix>...` creates or replaces a group.
- `tasks group rm <name>` deletes a group.
- `tasks groups` lists each group with the reachability of its members.

### 6.2 Membership rules

- A group must have at least one member.
- Members are live registered prefixes. `set` resolves a retired alias to its live prefix
  and stores that.
- Groups may overlap.
- Group names use the tag grammar and must not collide with a registered prefix.
- `rename` rewrites the prefix inside every group.
- `unregister` removes the prefix from every group, and deletes any group it leaves empty,
  with a warning.
- Loading a registry whose group names an unregistered prefix is a typed error: fail
  early, never skip silently.

### 6.3 Scope

`--group <name>` joins `--project` and `--all-projects` in `ScopeArgs` and conflicts with
both. It is accepted by list, ready, next, prime, tree, tags, sample, quiet and lanes. It
resolves to `Scope::All` over the members in registry order. An unreachable member is
skipped with the existing warning.

In JSON, `prime` adds `group: "<name>"` when scoped by group, and `prefix` is null as
under `--all-projects`.

An older binary that saves the registry drops `groups`, because the registry ignores
unknown keys. That is accepted for the same reason as §4.4.

## 7. Decisions on the waiting ideas

- **tasks-9bdd68 (focus marker):** not built as specified. The single focused goal it
  described, stored outside git, is covered by lanes plus lane priority, which are
  committed and visible to every session. A single focus also has the failure this design
  exists to fix: when the focused goal blocks, nothing names the next effort. The lanes
  view does. Propose dropping it as superseded, with this spec as the evidence.
- **tasks-77dbc6 (project groups):** §6 implements it as sketched: registry-declared,
  overlapping allowed, named "groups".
- **tasks-e02860 (lanes and shared resources):** §3–§5.

## 8. JSON changes

All changes are additive. The main design §5.1 gains `+=` lines:

- `Task` and `TaskSummary` += `lane` (bool, sparse) and `needs` (list, sparse).
- `prime` += `lanes` (always present) and `group` (only under `--group`).
- New payloads: `lanes`, `groups`, `group set|rm`.
- The claims JSON (`tasks claims`) += `holds` on a claim, sparse.

New error kinds: `unknown_need`, `need_held`, `unknown_group`, and `nested_lane` (from
validation).

## 9. Documentation and skills

- **`skills/tasks/SKILL.md`:**
  - lanes: when to make one, leading the body with the guidance paragraph, and pausing a
    lane with `shelve`;
  - needs: declaring the vocabulary, `TASKS_WITHOUT` for a host in ordinary use, and the
    `need_held` override;
  - groups;
  - the §3.4 field checklist items.
- **`skills/scope/SKILL.md`:** a scope pass that creates a cluster goal may mark it a lane
  when it is a standalone effort, and records the needs of the tasks it scopes.
- **README** add/edit block and the `prime` description.

## 10. Testing

These are end-to-end in `tests/cli.rs` with `TestEnv`, following the fixtures of the picker
and park tests.

**Lanes**
- A lane's field round-trips.
- A nested lane is refused.
- A childless lane is never ready, shows as `empty`, and `check` warns about it.
- A shelved lane is absent from the view.
- Lane order follows priority.
- Guidance is the first paragraph, with a leading heading skipped.

**Lanes view**
- The pick follows `next`'s order inside the lane, with parked candidates first.
- `held`, `waiting` and `empty` each appear with their counts.
- Two lanes whose heads need the same exclusive resource: the earlier lane picks it, and
  the later lane picks its next step that does not need it, or is `held`.

**Needs**
- An undeclared need is refused on add and edit.
- `check` errors on a need removed from the vocabulary.
- `--need` filters on the field.
- `--without` and `TASKS_WITHOUT` hide tasks; an unknown name is refused.

**Holds**
- `start` records `holds`.
- A second task needing the held resource is absent from `ready` and `next`, with a
  warning naming the holder, across two registered projects.
- `start` refuses it with `need_held`; `--force --reason` overrides and writes both notes.
- A park releases the hold.
- A dead claim holds nothing.

**Groups**
- `set`, `rm` and `groups` work.
- An alias resolves to its live prefix on `set`.
- `rename` and `unregister` keep groups consistent.
- `--group` scopes `prime` and `ready` to its members and conflicts with `--project`.
- A registry naming an unregistered member fails to load with a typed error.

All existing picker, park, halt and filter tests pass unchanged.

## 11. Not in this design

- **File-overlap conflicts between lanes:** branches rewriting the same files. Needs
  cover declared resources. Code ownership stays a judgement the person makes when
  creating a lane, and what checkout-ownership design tasks-ab8d2d covers.
- **Capacity above one:** a need serving two holders at once. Wait for a real case.
- **Merging the park's `needs` recipe with task needs,** or having `tasks quiet` list
  todo tasks that need an exclusive host resource.
- **Cross-project lanes:** a lane's members stay in its project, as `parent` already
  requires. A group view lists each project's lanes together.
- **Lane-aware ordering of `next`** (§5).

## 12. Alternatives considered

- **A reserved `lane` tag,** following the halt precedent. Rejected: a field is validated
  and keeps tags free-form. Halt is a tag because any task may become one.
- **A separate lanes file** (`tasks/lanes.toml`) listing lanes and their order. Rejected:
  it duplicates what goals already hold (body, children, closeout), and the file would
  need its own sync and validation.
- **Pairwise "lane A runs alongside lane B" marks.** Rejected: n² marks that go stale.
  Shared resources are declared once per step, and the parallel set is derived from them.
- **A single host-local focus** (tasks-9bdd68 as written). Rejected in §7.
- **Lane preference in `next`.** Rejected in §5. It can be added later as a tie-break,
  without changing the lane contract.

## 13. Implementation slices

1. **Needs:** vocabulary, field, `--need`/`--without`/`TASKS_WITHOUT`, check.
2. **Exclusive holds:** claim `holds`, the picker gate, the `start` refusal and override.
   Depends on 1.
3. **Lanes:** the field, validation, `--under`, the lanes view, the `prime` section.
   Depends on 2 for `held` states; can land with needs-less states first if 2 slips.
4. **Project groups.** Independent of 1–3.

Each slice updates the skill and the main design addenda for what it ships.
