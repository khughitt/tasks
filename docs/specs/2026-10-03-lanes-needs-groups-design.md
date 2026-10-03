# Lanes, needs, and project groups — design

**Status:** draft, revised after review round 1, 2026-10-03. Task: tasks-ece1e2.
Waiting ideas: tasks-9bdd68 (focus marker), tasks-77dbc6 (project groups),
tasks-e02860 (lanes and shared resources). Brief:
`docs/notes/2026-09-30-work-selection-brief.md`.

## 1. Outcome

A project can say, inside the tracker, which efforts run side by side and what makes them
contend. Anyone who runs `prime` sees the lanes, what each is for, and the step each could
take now. The picker stops handing out a step whose shared resource another session is
using. Several projects can be read together as a named group.

Three additions, each usable alone:

- **Lanes:** a goal marked `lane: true` is an effort meant to proceed alongside other
  lanes. `prime` and the new `tasks lanes` show each lane with its guidance and next step.
  Every task row names the lane it belongs to.
- **Needs:** a task lists the shared resources its work uses (`needs: [quiet]`), drawn
  from a vocabulary the project declares. A need declared `exclusive` serves one session
  at a time. A session can also say which needs it cannot meet (`TASKS_WITHOUT=quiet` on a
  host in ordinary use), and the picker hides those steps.
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
design made the same choice. Halt is a tag because any task may become one.

The parallel design rejected a label on every task. This design does not reintroduce that
label: membership comes from the existing `parent` tree, so only the lane goal itself is
marked.

Records are strict (`format.rs` rejects unknown keys). An older binary therefore fails to
parse a record carrying `lane:` or `needs:`. That is the existing precedent for every
added field: hosts run the same installed build.

### 3.2 A lane is a goal

One helper, `is_goal(task) = has_children || task.lane`, replaces the bare
`has_children` test at every site that treats goals specially:

- readiness (`query.rs`);
- parked candidates (`parked.rs`);
- the defer and recurrence validation (`hierarchy.rs`).

A lane is therefore:

- never ready, even with no children;
- never a parked candidate;
- never deferred or recurring.

Closeout is unchanged: a lane closes when it has at least one child and no open descendant.

### 3.3 Validation

**No nested lanes.** No lane may have a lane as an ancestor. A lane inside a lane would
make "one pick per lane" ambiguous; sub-efforts inside a lane are ordinary child goals. The
check runs on every write that can create nesting:

- `add --lane --parent <p>`;
- `edit --lane` on a task that has a lane ancestor or a lane descendant;
- `add --parent` and `edit --parent` moving a lane, or a subtree containing one, under a
  lane;
- `check`, for records written by hand or merged.

Every refusal uses the error kind `nested_lane`.

**A childless lane** is a warning from `check`, not an error. A lane is often filed before
its steps exist.

### 3.4 Pausing a lane

A lane whose status is `blocked` is **paused**. Its descendants are not eligible for
`ready`, `next`, `prime`'s ready list, or the lanes view, and the lanes view shows the lane
as `paused`. `tasks block <lane> "<reason>"` pauses it and `unblock` resumes it. Both are
existing commands, and the reason records why.

This is the one place an ancestor's status gates a descendant. It applies to lanes only;
an ordinary blocked goal keeps today's behaviour.

`shelve` is not the pause mechanism. It refuses a goal with unshelved descendants, and
`unshelve` returns tasks to `idea`. A shelved lane is gone from every view, as any shelved
task is.

### 3.5 Guidance

A lane's **guidance** is the first paragraph of its body. It skips leading blank lines and
leading heading lines (any level), and runs up to the next blank line. An empty body gives
`null`. The paragraph is meant to say why the lane exists and what its first milestone is.

No new field holds it. The body is already where a goal says what it is for, and the skill
tells authors to lead with that paragraph (§9).

### 3.6 Lane membership on every row

`TaskSummary` and the `show` shape gain `lane: "<id>"`: the nearest lane ancestor, or the
task's own id when it is a lane. The key is omitted when there is none. Every `ready`,
`next` or `list` row then says which effort it belongs to, without opening the lanes view.

The boolean field is visible as `lane_goal: true` on the lane's own row. It is always
present, like `parallel`. Pretty tables mark lane rows with `≡`.

### 3.7 Order

Lanes are listed in ready order of the lane goals: priority, size, created, id
(`query::ready_order`). A lane's priority is therefore its rank in the lanes view. It sets
which lane wins a contested exclusive need (§5) and which lane is shown first. It does not
reorder `ready` or `next` (§5.4).

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

The vocabulary is committed and syncs between hosts, like `[tags]` and `[feedback]`.

The names of **exclusive** needs form one namespace shared by every project on the host.
The hold gate (§4.4) matches them by name across projects, because the resources they
stand for (an idle host, a GPU, a device) belong to the machine. The skill's convention:
an exclusive need names a host resource, and two projects that use the same name mean the
same resource.

Non-exclusive needs stay per project. They only describe a step and filter it.

### 4.2 The field

`needs: [quiet, owner]` is a new optional frontmatter list. It is written only when
non-empty and placed after `lane`. Two places set it:

- `add --need <n>`, repeatable;
- `edit --need <n>` (appends), `--rm-need <n>` and `--no-needs`, mirroring the tag flags.

Validation:

- **On write:** every name must be declared in the project's vocabulary. `add` and `edit`
  refuse an undeclared name with the error kind `unknown_need`.
- **On `check`:** a record naming an undeclared need is an error.
- **On read:** the need is carried as-is, so a worktree on an older config can still list
  tasks.

`TaskSummary` gains `needs`, sparse (omitted when empty).

The quiet park has its own `needs` recipe (`idle` or `headless`), which says what "idle"
means for one parked run. It stays `park.needs`, nested under `park` in JSON. The task
field is top-level `needs`. Merging the two is out of scope (§11).

### 4.3 Session availability: `--without` and `TASKS_WITHOUT`

A session declares the needs it cannot meet:

- `--without <need>` (repeatable) on `ready`, `next`, `prime` and `lanes`;
- `TASKS_WITHOUT` (comma-separated) for the whole session.

The effective set is the union of the two, so the flag adds to the variable. An empty
variable means the variable contributes nothing. A task needing any name in the set is
hidden from those views.

The strictness differs:

- **The flag is strict.** A name that no project in scope declares is refused with
  `unknown_need`, because it is a typo.
- **The variable is lenient.** A name that a project does not declare hides nothing in
  that project, and there is no error. The variable is set once per host and applies to
  every project, including those with no such need.

The intended use: a host in ordinary use sets `TASKS_WITHOUT=quiet` in its session
environment, and a TTY session handed the idle host runs without it. Availability is
opt-in. Without the variable, a `quiet` step is still offered, and the hold gate (§4.4)
still prevents two sessions from taking it.

`list --need <n>` and `ready --need <n>` filter on the field, using the shared
`TaskFilter` all-of rule, like `--tag`.

### 4.4 Exclusive holds

A **hold** is a claim's use of an exclusive need. The claim entry in the store gains
`holds: [<need>...]`: the task's needs that its own project declares exclusive, with any
undeclared name ignored. Because the holds live in the claim stores, any view can see
them without opening another checkout.

**Recorded on every acquire.** `holds` is computed whenever a claim is acquired: on
`start`, and on `edit --status doing` or an editor save that moves to `doing`. These are
every path that builds `ClaimIntent::Acquire`. It is also recomputed when a save changes
the `needs` of a task its saver claims. A change to the vocabulary takes effect at the
next acquire or `needs` save.

**Held only while live.** A hold exists only while its claim is live, by the claims
design's liveness rule. A `doing` status alone does not hold, and neither does a park: a
run parked while waiting for the quiet host holds nothing until a session claims it again.
A claim without a pid (TTL-only) holds for its TTL. The skill tells a long run to
heartbeat with `tasks note`.

**Blocks other sessions only.** A task is held back when:

- its own project declares the need exclusive, and
- a live claim belonging to **another session** holds a need of that name.

A session that holds `quiet` can start a second task needing `quiet`; it is already using
the idle host.

**Which stores are read.** The gate reads every claim store in the state directory
(`claims/*.toml`), not only those in scope, so that a capture claimed in another project
holds the host here too. A store that cannot be read gives the warning
`hold state unknown for <prefix>` and contributes no holds, following the halt pattern.
It does not fail the command.

**Atomic across projects.** An acquire that would record a non-empty `holds` takes one
host-wide lock, `claims/holds.lock`, around the hold check and the claim save. Two `start`s
in different projects then cannot both win the same need. The project's mutation lock is
taken first and the holds lock second, everywhere, so the order is fixed.

**Older binaries.** An older binary that saves the claim store drops `holds`, because
unknown keys are ignored and dropped on save. The effect is a weaker gate, never a false
block. This is documented, not engineered around.

### 4.5 Where holds gate

- **`ready`, `next`, and the `ready` list in `prime`** drop a task that is held back
  (§4.4). Parked-agent candidates in `next` go through the same gate. Ordering is
  unchanged. Warnings are aggregated per need and holder:
  `<n> task(s) wait for <need>, held by <holder id> (<session>)`.
- **Acquire** refuses a held-back task with the error kind `need_held`, on every acquire
  path. This mirrors halt's `guard_new_start`:
  - `--force --reason "<why>"` overrides;
  - `--force` without `--reason` is refused, as under halt;
  - `--force` keeps its existing meaning of taking over another session's claim on the
    same task.

  The override writes a note on the acquired task:
  `need override: acquired while <need> held by <holder>: <reason>`. A matching note goes
  on the holder only when the holder is in the same project, as halt does. A cross-project
  write would need the other project's lock.

Needs change eligibility only through holds (§4.4) and `--without` (§4.3). A task that
needs `owner` is still ready: the need tells whoever picks it what the step will ask of
them.

## 5. The lanes view

`tasks lanes [--without <n>]... [--max-complexity <c>] [scope]` and the new `lanes` key of
`prime` share one builder.

### 5.1 The builder

The builder covers every open, unshelved lane in scope, in lane order (§3.7).

**Paused lanes.** A `blocked` lane is `paused` (§3.4). It lists its active work and
nothing else.

**Every other lane** goes through four steps:

1. **Collect steps.** The lane's descendants that `next` would consider, before the hold
   gate:
   - parked-agent candidates in the subtree first, newest park first;
   - then `ready_tasks` rows in ready order.

   The dependency, claim, user-park, defer, halt, complexity-cutoff and `--without` gates
   apply. Holds are applied in step 3, not here.
2. **Collect active work.** Descendants with a live claim.
3. **Pick one step.** The pick is the first step whose exclusive needs are not held by
   either of these:
   - a live claim of another session (`by: "claim"`);
   - the pick of an earlier lane in this same view (`by: "pick"`).

   The picks across lanes form a set of steps that can run at the same time, as far as
   declared needs can tell. Earlier lanes win contested needs because lane order is the
   person's ranking.
4. **Classify** each open descendant that is not a step into exactly one cause, so the row
   explains itself:

   | Cause | Descendant |
   |---|---|
   | `active` | has a live claim |
   | `held` | a step not picked because its exclusive need is held (counted, and listed in `held`) |
   | `without` | hidden by `--without` / `TASKS_WITHOUT` |
   | `cutoff` | above the complexity cutoff |
   | `halt` | stopped by a halt |
   | `deferred` | deferred |
   | `user` | parked waiting on a person |
   | `blocked` | status `blocked` |
   | `depends` | has an open dependency |
   | `goal` | an open sub-goal (its own descendants are counted) |
   | `other` | anything else: `idea` status, or `doing` without a live claim |

   A descendant matching several causes counts once, under the first cause in table order.

### 5.2 States

| State | Meaning |
|---|---|
| `paused` | the lane is `blocked` |
| `ready` | a pick exists |
| `held` | steps exist, but every one waits for an exclusive need held by a claim or an earlier pick |
| `waiting` | no step exists, and some open descendants remain; `causes` says why |
| `empty` | no open descendant |

Active work is reported in every state. A lane can be `ready` while another of its tasks is
already claimed.

### 5.3 Output

**Pretty `prime`** prints a `lanes:` block before `roadmap:`, one row per lane: id,
priority, title, then the state and either the pick (`→ <id> <title>`) or its main cause
(`held: quiet ← <holder>`, `waiting: 2 user, 1 deferred`). **`tasks lanes --pretty`** adds
the guidance line and the active claims under each row.

**JSON:** `prime` gains `lanes: [LaneRow]`, always present and empty when the scope has no
lanes. `tasks lanes` returns `{lanes: [LaneRow], warnings}`. The `LaneRow` shape:

```json
{"lane": TaskSummary, "guidance": "…" | null, "state": "ready",
 "pick": TaskSummary | null,
 "steps": 3,
 "active": [TaskSummary],
 "held": [{"id": "…", "need": "quiet", "holder": "…", "by": "claim"}],
 "causes": {"user": 2, "deferred": 1}}
```

`held` and `causes` are omitted when empty, and `causes` lists only non-zero counts,
following the sparse rule.

### 5.4 Selecting within one lane, and what `next` does

`--under <ref>` is a new shared filter for descendants of a task at any depth. It is
accepted by `list`, `ready` and `next`; this design gives `next` its first filter.
`next --under <lane>` is how a session committed to one lane picks its next step.
`--parent` keeps its direct-child meaning.

Without `--under`, `next` does not reorder by lane. It still takes parked-agent resumes
first, then ready order, with §4.3–§4.5 applied. Priority stays the urgency signal, and an
urgent bug outside every lane still wins. The lanes view answers a different question:
what to run in parallel.

## 6. Project groups

### 6.1 Declaration

Groups are declared in the registry, beside `projects` and `aliases`:

```toml
[groups]
verifiably = ["nodes", "atoms", "beliefs"]
```

The registry is host-local and not synced (main design §6), so each host declares its own
groups. An older binary that saves the registry drops `groups`, for the same reason as
§4.4. That is accepted.

### 6.2 Commands

- `tasks group set <name> <prefix>...` creates or replaces a group.
- `tasks group rm <name>` deletes a group.
- `tasks groups` lists each group with the reachability of its members.

### 6.3 Rules

- A group must have at least one member.
- Members are live registered prefixes. `set` resolves a retired alias to its live prefix
  and stores that.
- Groups may overlap.
- Group names use the tag grammar. A name must not equal a registered prefix or a retired
  alias, and `init`/`register` refuse a prefix that equals a group name.
- `rename` rewrites the prefix inside every group.
- `unregister` removes the prefix from every group, and deletes any group it leaves empty,
  with a warning.
- A registry whose group names an unregistered prefix fails to load with the existing
  `config` error kind, naming the group and the prefix. A misspelled group named on the
  command line is `unknown_group`.

### 6.4 Scope

`--group <name>` joins `--project` and `--all-projects` in `ScopeArgs` and conflicts with
both. It is accepted by list, ready, next, prime, tree, tags, sample, lanes and claims.
`quiet`, which has its own scope flags, gains `--group` as well.

The scope records the requested members, so `Scope::All` carries a member set. Views that
warn about projects that are absent or unreachable (`halt_snapshots`, the scope
resolution) warn only about requested members. The hold gate is the exception: it reads
every claim store (§4.4) and warns only for stores it cannot read.

In JSON, `prime` adds `group: "<name>"` when scoped by group, and `prefix` is null as under
`--all-projects`.

## 7. Decisions on the waiting ideas

- **tasks-9bdd68 (focus marker):** not built as specified. Its parts are covered as
  follows:
  - "Show the focused tree in `prime`" is the lanes block, ordered by lane priority.
  - "Prefer the focused goal in `ready`/`next`" becomes `next --under <lane>` for a
    session committed to one effort. The default picker deliberately stays unchanged
    (§5.4).
  - The host-local, uncommitted storage it proposed is replaced by a committed field
    every session sees.

  A single focus also has the failure this design exists to fix: when the focused goal
  blocks, nothing names the next effort. The lanes view does. Propose dropping it as
  superseded, with this spec as the evidence.
- **tasks-77dbc6 (project groups):** §6 implements it as sketched: registry-declared,
  overlapping allowed, named "groups".
- **tasks-e02860 (lanes and shared resources):** §3–§5.

## 8. JSON changes

All changes are additive. The main design §5.1 gains `+=` lines:

- `Task`, `TaskSummary` and the `show` shape += `lane_goal` (bool, always present),
  `lane` (nearest lane id, sparse) and `needs` (list, sparse).
- `ClaimInfo` += `holds` (list, sparse). This reaches `TaskSummary.claim`, the rows of
  `prime.doing`, and `tasks claims`.
- `prime` += `lanes` (always present) and `group` (only under `--group`).
- New payloads: `lanes`, `groups`, `group set|rm`.
- New error kinds: `unknown_need`, `need_held`, `unknown_group`, `nested_lane`.

## 9. Documentation and skills

- **`skills/tasks/SKILL.md`:**
  - lanes: when to make one, leading the body with the guidance paragraph, pausing with
    `block`/`unblock`, and `next --under`;
  - needs: declaring the vocabulary, the shared namespace of exclusive names,
    `TASKS_WITHOUT` for a host in ordinary use, the `need_held` override, and
    heartbeating a long hold;
  - groups;
  - the §3.4 field checklist items.
- **`skills/scope/SKILL.md`:** a scope pass that creates a cluster goal may mark it a lane
  when it is a standalone effort, and records the needs of the tasks it scopes.
- **README** add/edit block and the `prime` description.

## 10. Testing

These are end-to-end in `tests/cli.rs` with `TestEnv`, following the fixtures of the picker,
park and halt tests.

**Lanes**
- The field round-trips.
- Nesting is refused on each write path: `add --lane --parent`, `edit --lane` above or
  below a lane, and re-parenting a subtree that contains a lane under a lane. `check`
  reports nesting written by hand.
- A childless lane is never ready, never a parked candidate, and refuses `defer`/`every`;
  `check` warns about it.
- A `blocked` lane hides its descendants from `ready` and `next` and shows as `paused`;
  `unblock` restores them.
- A shelved lane is absent from the view.
- `lane` names the nearest lane ancestor on `ready` and `next` rows.
- Guidance skips blank and heading lines; an empty body gives `null`.

**Lanes view**
- The pick follows `next`'s order inside the lane, with parked candidates first.
- Each state appears: `paused`, `ready`, `held`, `waiting`, `empty`.
- The causes partition counts every open descendant once.
- Two lanes whose heads need the same exclusive resource: the earlier lane picks it, and
  the later lane picks its next step without it or is `held` with `by: "pick"`.

**Needs**
- An undeclared need is refused on `add` and `edit`; `check` errors on a need removed from
  the vocabulary.
- `--need` filters on the field.
- `--without` hides tasks and refuses an undeclared name.
- `TASKS_WITHOUT` hides tasks, and is ignored without error in a project that does not
  declare the name.
- The flag and the variable combine as a union.

**Holds**
- `start`, `edit --status doing` and an editor save each record `holds`, and each refuses
  with `need_held` when another session holds the need.
- `--force --reason` overrides and writes the note(s); `--force` alone is refused.
- A second task needing the held resource is absent from `ready` and `next`, with one
  aggregated warning, across two registered projects.
- The holding session itself can start a second task with the same need.
- A park releases the hold. A dead claim holds nothing.
- `edit --rm-need` on a claimed task updates `holds`.
- An unreadable claim store warns and does not fail `ready`.
- Two concurrent `start`s in different projects contending for one need: exactly one
  succeeds.

**Groups**
- `set`, `rm` and `groups` work, and an alias resolves to its live prefix on `set`.
- `rename` and `unregister` keep groups consistent; `register` refuses a prefix equal to a
  group name.
- `--group` scopes `prime`, `ready` and `quiet` to the members, and conflicts with
  `--project`.
- Under `--group`, halt warnings name only members.
- A registry naming an unregistered member fails to load with `config`.

All existing picker, park, halt and filter tests pass unchanged.

## 11. Not in this design

- **File-overlap conflicts between lanes:** branches rewriting the same files. Needs cover
  declared resources. Code ownership stays a judgement the person makes when creating a
  lane, and what checkout-ownership design tasks-ab8d2d covers.
- **Capacity above one:** a need serving two holders at once. Wait for a real case.
- **Needs hidden by default** (a need offered only with an explicit `--with`). Availability
  is opt-in through `TASKS_WITHOUT` (§4.3). Revisit if sessions keep being handed steps
  they cannot run.
- **Merging the park's `needs` recipe with task needs,** or having `tasks quiet` list todo
  tasks that need an exclusive host resource.
- **Cross-project lanes.** A lane's members stay in its project, as `parent` already
  requires. A group view lists each project's lanes together.
- **Lane-aware ordering of `next`** without `--under` (§5.4).

## 12. Alternatives considered

- **A reserved `lane` tag,** following the halt precedent. Rejected in §3.1.
- **A separate lanes file** (`tasks/lanes.toml`) listing lanes and their order. Rejected:
  it duplicates what goals already hold (body, children, closeout), and the file would need
  its own sync and validation.
- **Pairwise "lane A runs alongside lane B" marks.** Rejected: n² marks that go stale.
  Shared resources are declared once per step, and the parallel set is derived from them.
- **Exclusivity declared in host config** rather than the project vocabulary. Rejected: a
  project's steps would then need a per-host setup before the gate worked on a fresh
  machine. The shared-namespace convention (§4.1) gives the same host-wide match.
- **Pausing a lane through `shelve`.** Rejected in §3.4.
- **A single host-local focus** (tasks-9bdd68 as written). Rejected in §7.
- **Lane preference in default `next`.** Rejected in §5.4. It can be added later as a
  tie-break without changing the lane contract.

## 13. Implementation slices

1. **Needs:** vocabulary, field, `--need`, `--without`/`TASKS_WITHOUT`, check.
2. **Exclusive holds:** claim `holds` on every acquire path, the host-wide lock, the
   picker gate, and the `need_held` refusal and override. Depends on 1.
3. **Lanes:** the field, `is_goal`, the nesting check, pausing, the `lane` membership
   key, `--under` (including on `next`), the lanes view, and the `prime` section. Depends
   on 2 for held states. It can land with needs-less states first if 2 slips.
4. **Project groups.** Independent of 1–3.

Each slice updates the skill and the main design addenda for what it ships.
