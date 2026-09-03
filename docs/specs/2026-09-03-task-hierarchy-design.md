# Task hierarchy: tasks as the authority for goals — design

**Status:** approved (2026-09-03, after one review round), not yet implemented; implements
tasks-061851. Supersedes the "no hierarchy"
non-goal in docs/specs/2026-08-29-tasks-design.md §1.

## 1. Problem

The current convention is spec → plan → one task per plan step. Tasks are downstream
leaves; goals live in documents. In a downstream project four pending sub-projects existed
only as a table inside an umbrella design doc. The tracker showed 32 done and 1 todo, so
`prime` and `ready` said the project was nearly finished while four large pieces of work
were outstanding. A reader of the tracker alone could not see what was next.

The fix is to invert the convention: goals are recorded as tasks first, at whatever
granularity is known, and specs and plans are optional attachments fleshed out as each
edge is pushed on. That needs one thing the model lacks today: a task that contains other
tasks.

## 2. Model

One task type, nestable to any depth, through a single new field:

| Field    | Type            | Required | Notes |
|----------|-----------------|----------|-------|
| `parent` | task id         | no       | Same project only. The parent must exist. A task cannot be its own ancestor. |

Everything else about the model is unchanged. There is no stored kind, no epic or
sub-task type, and no profile that pre-sets fields. The reasons:

- Granularity is discovered, not declared. A goal is recorded before anyone knows how it
  decomposes; a leaf that later grows children must not have to change type.
- Kinds drift and structure does not. Root, interior, and leaf are computed from the tree
  and are therefore always true. A stored "umbrella" label on a node with one child is
  simply wrong, and nothing would correct it.
- Every behavioural difference the tool needs is structural (§4), so a kind would carry no
  behaviour and would only widen the CLI and the JSON contract.

### 2.1 Vocabulary

The words are for people and prose, not for the schema. They name positions in the tree:

| Word | Position | Typical attachment |
|------|----------|--------------------|
| roadmap | the set of open tasks with no parent | none, or a roadmap document as `spec` |
| umbrella | an open task with children | an umbrella design as `spec`; a plan whose `Task N:` headings are its children |
| task | a leaf | a spec and/or a plan step |

These words appear in the skill, in this document, and as `--pretty` section headings.
They never appear as field values.

### 2.2 Containment versus ordering

`parent` and `depends` are different relations and neither is expressed through the other:

- `depends` is ordering: "this cannot start until that is closed". It may cross projects
  and it may point at siblings, cousins, or anything else.
- `parent` is containment: "this is part of that". It is same-project only and forms a
  forest.

A child is not implicitly a dependency of its parent for scheduling purposes, but the
closing rules in §4.2 treat open descendants the way they treat open dependencies.

Because nesting is unbounded, every rule below is stated in terms of **descendants**, not
direct children. A direct-child rule would let a force-closed middle node hide open work
beneath it: with `A → B → C`, force-closing `B` while `C` is open must not make `A`
closable without force. "Open descendant" is computed by walking the subtree.

## 3. Storage

`parent: sci-4f2a9c` is written in the frontmatter after `depends`. Absent means null. No
migration: every existing file already has a null parent and is a root. The serialized
field order becomes `id, title, status, priority, size, owner, created, updated, depends,
parent, tags, spec, plan, step`.

## 4. Behaviour

### 4.1 Validation (write paths and `check`)

Every write path, including `edit` flags and the editor path, rejects:

- a `parent` whose prefix is not this project's (`validation`);
- a `parent` that does not exist locally (`unresolvable_id`);
- a `parent` that would make the task its own ancestor (`cycle`), including `parent`
  equal to the task's own id.

`check` reports the same as errors (`dangling_parent`, `foreign_parent`, `parent_cycle`)
and additionally warns:

- `open_child_of_closed_parent`: an open task whose parent is done or dropped. The tree
  claims the goal is finished while work under it is not.

### 4.2 Closing rules

The open-deps rule of the original design §3.3 becomes the open-work rule: a task may
become `done` only when every dependency **and every descendant** is closed. `--force` on
`done` overrides both, as today. `drop` refuses while any descendant is open and has no
override; an abandoned goal must have its subtree dropped or reparented first, so that
nothing open is silently left under a closed node.

Reopening a closed task (`done`/`dropped` → `todo`) is allowed regardless of its parent's
status; `check` then warns if the parent is closed. Force-closing and reopening are the
only ways an open task ends up under a closed ancestor; the views in §4.3 and §4.4 are
defined so that such a task is never hidden.

### 4.3 Ready and prime

`ready` is for work, and a task with children is a goal, not work. So `ready` gains one
condition: **a task with children is never ready.** The definition becomes: status
`todo`, every dependency closed, no children. A root with no children is an ordinary
task and is ready under the old rule.

Parents surface through a separate list, because the protocol has an agent `start` a goal
before brainstorming or decomposing it, which leaves the goal `doing`. A `todo`-only rule
would therefore never show it again. `prime` gains:

- `closeout`: every open task (`todo`, `doing`, or `blocked`) that has at least one child
  and no open descendant. That is the explicit close-out: someone confirms the goal is met
  and runs `done`, or adds the children that are still missing. Nothing closes
  automatically.
- `roadmap`: the open forest, as nested nodes, identical to `tasks tree` with no
  arguments (§4.4). Roots are the roadmap; their subtrees show how each goal is
  decomposed and how far along it is. A project with no hierarchy gets a flat list of its
  open tasks here, which is the honest answer to "what is this project trying to do".

A reader of `prime` alone can now see a goal with four open children even when `ready` is
empty, and a `doing` goal whose children have all closed even though `ready` will never
list it.

Parents do not inherit or propagate anything else. A `blocked` parent does not block its
children; priority and size are per task; a child's owner is its own.

### 4.4 Commands

```
tasks add <title> [... existing flags ...] [--parent ID]
    --parent is validated before anything is written (§4.1).

tasks edit <id> [... existing flags ...] [--parent ID | --no-parent]
    Reparent or detach. The editor path accepts the same by editing the field.

tasks list [... existing flags ...] [--parent ID]
    Direct children of ID, subject to the other filters.

tasks tree [<id>] [--all]
    The hierarchy as nested nodes: the whole forest, or the subtree under <id>. This is
    the read side of parent, as graph is of depends. Without --all the forest is pruned
    to nodes that are open or have an open descendant, so a closed ancestor of open work
    stays visible as context, with its closed status, rather than hiding the work
    beneath it. --all includes every task. Roots and siblings are in ready order
    (priority, size, created); a parent precedes its children.

tasks show <id>
    Additionally reports the parent (id, title, status) and the direct children.

tasks done / tasks drop
    As §4.2.

tasks ready / tasks prime
    As §4.3.

tasks check
    As §4.1, plus §4.5.
```

`graph` keeps rendering dependencies only.

### 4.5 Reverse drift

Today `check` verifies task → heading: a `step` must still exist in its plan. The
downstream failure was the other direction: work named in a document with nothing
tracking it. `check` adds one warning:

- `unlinked_step`: for every plan file that at least one task links, every heading whose
  text starts with `Task <digits>:` and that no task references as `step`.

It is a warning, not an error, because a plan may legitimately carry steps that were
finished before the project adopted the tracker. It is limited to the `Task N:`
convention because that is the only heading shape the tool can recognise; a table of
sub-projects in a design doc is still invisible to it, and the remedy for that is the
skill's protocol (§5), not more parsing.

## 5. Skill and session protocol

The skill's rules change in these places:

- **Tasks first.** A goal that is committed work is recorded as a `todo` task with a body
  the moment it is known, however large. `idea` stays reserved for uncommitted thoughts;
  a roadmap item that the project intends to do is not an idea.
- **Decompose with `--parent`.** When a goal is split into pieces that are parts of it,
  add each piece with `--parent <goal>`. Use `dep` only for ordering between the pieces.
  The old recipe of splitting through `dep` alone still works but leaves the tree flat and
  loses the roadmap view; prefer `--parent`.
- **Never pick a task with children as work.** `ready` already excludes them; the rule is
  stated so that an agent reading `list` does not start an umbrella by hand. `start` on
  an umbrella means "I am decomposing or designing this", and it is expected to stay
  `doing` until close-out.
- **Close out explicitly.** When an umbrella appears in `prime`'s `closeout` list, confirm
  the goal is met and `done` it with a message, or add the missing children.
- **Brainstorming** attaches, rather than creates: it runs against an existing task, and
  after approval the spec is attached with `edit <id> --spec <topic>`. Deliverables the
  spec identifies become children of that task.
- **writing-plans** writes the plan for a task and attaches it with `edit <id> --plan`;
  its `Task N:` headings become children with `--parent <id> --plan --step`. `check`'s
  `unlinked_step` warning then names any heading without a task.
- **`prime`** is read for `roadmap` and `closeout` as well as `ready`; the roadmap is the
  answer to "what is this project trying to do", `closeout` is "which goals need a
  verdict", and `ready` is "what can I do now".

## 6. JSON contract changes

All additions; no existing field changes meaning.

```
Task        += parent: string|null
TaskSummary += parent: string|null,
               child_count: int,              direct children, any status
               open_descendant_count: int     open tasks anywhere in the subtree
show        += parent: { id, title, status }|null,
               children: [{ id, title, status }]
TreeNode     = TaskSummary + { children: [TreeNode] }
tree        -> { nodes: [TreeNode], warnings }
prime       += roadmap: [TreeNode],           the open forest, pruned as tree (§4.4)
               closeout: [TaskSummary]        see §4.3, ready order
check       += kinds dangling_parent, foreign_parent, parent_cycle (errors);
               open_child_of_closed_parent, unlinked_step (warnings)
```

The counts are named so that no field changes type between shapes: `children` is always
an array of nodes and only appears in `TreeNode` and `show`.

`--pretty` renders `tree` as an indented list. For `prime` it prints `closeout:` and
`roadmap:` sections above `ready:`; to keep a flat project's `prime` readable, the
roadmap section prints the subtree of every root that has children and then one line
counting the childless roots, which `ready:` already lists.

## 7. Original design updates

Land in the same change as the implementation:

- §1 non-goals: replace "No hierarchy (epics/subtasks), no `split` command" with "No task
  kinds; hierarchy is one `parent` field (see the 2026-09-03 hierarchy design)".
- §3.1: add the `parent` row. §3.3: the open-work rule.
- §5: `--parent`, `--no-parent`, `list --parent`, `tree`, the `show`/`prime` additions.
- §5.1: the shapes in §6 above.
- §7: the `unlinked_step` warning. §8: the protocol changes in §5 above, mirrored in
  `skills/tasks/SKILL.md`.

## 8. Testing

Unit: parent cycle detection (including self-parent and a three-deep loop); ready
excludes any task with children; open-descendant counting through a force-closed middle
node (`A → B → C` with `B` done and `C` open gives `A` one open descendant); pruning
keeps a closed ancestor of an open task; preorder and sibling order; serialized field
order.

End to end (`tests/cli.rs`): `add --parent` validates prefix, existence, and cycles
before writing; `done` refuses with an open descendant, including one under a
force-closed child, and succeeds with `--force`; `drop` refuses with an open descendant;
`edit --no-parent` detaches; `tree` nests, prunes, and `--all` includes closed;
`prime.roadmap` lists an open parent while `ready` omits it; a `doing` parent whose
children have all closed appears in `prime.closeout`; `check` reports each new error and
warning kind, including `unlinked_step` for a plan with an unreferenced `Task 3:` heading;
editor path accepts and rejects `parent` like the flags.

## 9. Out of scope

Cross-project parents; automatic closing of parents; inheritance of priority, status, or
owner; parent edges in `graph`; recognising sub-project tables in design docs; renaming
`depends`. Each is a separate task if it ever earns one.
