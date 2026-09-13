# Explicit task process

Status: draft — awaiting user review; no implementation has started.
Task: tasks-61cc5c

## Problem and evidence

The task reports that prism-28e29c ran directly on main because the agent followed
the tasks protocol without entering the brainstorming and worktree workflow. That
incident is the motivating report, not independently reproduced here.

In this checkout at e60925a, no task field records that choice. `Phase::of` in
`src/model.rs` derives a parked task's phase from status and document links; a todo
without links is labelled implementing. It describes a resume position, not a
decision to skip design. `complexity` measures reasoning effort, and its design
explicitly separates that rating from review policy. Neither establishes which
process the person scoping the task intended.

## Decision

Add an optional `process` field with two values, chosen explicitly at scoping:

| Value | Meaning |
|---|---|
| `direct` | The task body or existing reviewed artifacts establish the outcome, approach, and verification. Execute without creating another design spec or implementation plan. |
| `planned` | Resolve the design in a written spec, obtain its review, write an implementation plan, obtain its review, then execute. Reuse existing artifacts after verifying their contents and review state. |

Missing means **unassessed**, not direct or planned. Priority, size, complexity,
status, parent, and document presence never set or change the field implicitly.
Small risky work may need planning; a large mechanical change may be direct.
A plan step can be direct because its reviewed plan supplies the decisions.

Alternatives rejected:

- Derive process from existing fields: fewer annotations, but no existing field
  expresses the decision, and document presence cannot prove approval.
- Record it only in a note or task body: no schema change, but picking sessions
  would still have to discover and interpret prose that ready rows omit.

This is a process choice, not a switch that disables all Superpowers skills.
Debugging, applicable checks, and review remain necessary on either path.

## Workspace and instruction policy

Separate isolation from planning: both paths use an isolated worktree when changing
repository code. Reuse the task's existing worktree when resuming; otherwise create
one with `git worktree add` under `.worktrees/`, then run `just setup` when defined.
Planned work creates it before drafting the spec, following the existing user rule.
Read-only investigation and task-record maintenance alone need no new worktree.
An explicit user instruction to work in place still wins.

This deliberately extends the existing brainstorming-and-planning worktree rule to
direct code changes. Selecting direct saves document ceremony, not isolation.

The CLI stores and reports the selection; it does not launch skills, create
worktrees, infer approval from files, or police Git branches. The shipped tasks
protocol explains how to act on it. This repo's `AGENTS.md` explicitly adopts that
protocol for choosing direct versus planned, including the worktree rule, so a
generic skill trigger does not silently select a different path.

A field is not authority to override higher-priority instructions. Other projects
must adopt the same policy in their own agent instructions before relying on direct
to waive their mandatory brainstorming rules. Shared harness configuration changes
and edits to other projects are outside this repository change; the README must
state this adoption requirement rather than promise universal enforcement.

## Record, commands, and output

- Follow the existing `Complexity` pattern: `Process::{Direct, Planned}`, optional
  on `Task`, with validated parsing and an omitted frontmatter key when unset.
- Add `add --process direct|planned`, `edit --process direct|planned`, and
  `edit --no-process`; the two edit forms conflict. Complete both values.
  Editor saves validate the field through the existing record parser. Invalid
  values fail before any record write. Clearing the field restores unassessed.
- Add `process` as a string or null to serialized `Task`, `TaskSummary`, and
  `ParkedRow`. Existing wrappers then carry it in ready, next, prime, show, list,
  and parked/resume output. Unresolved parked rows use null. Do not rename or
  reinterpret `phase` or any existing key.
- Pretty summary rows gain a process column (`direct`, `planned`, or `-`);
  pretty show/next identify the value as `Process: …`, using `unassessed` for null.
  Parked rows show process alongside their existing phase.
- Process does not change readiness, ordering, selection, or claim handling.
  Add a `process_missing` check warning for todo/doing records without it, including
  goals and plan steps. Ideas, shelved records, and closed records do not warn.
  Starting an unassessed task remains allowed by the CLI.

No process filter, default setting, inheritance, additional approval-state field,
or Git enforcement is needed for this change.

## Agent protocol and rollout

Before starting implementation, read the process and state the chosen path and
workspace. If unassessed, read the full task and relevant code, choose and record a
value with a short reason, then follow it. Do not silently treat null as direct.
Ideas still need scoping before implementation, even if they already carry a value.

Scoping sets process alongside size and complexity. A planner explicitly assigns
process to each step child; curation may fill a missing choice with evidence.
If direct work reveals an unresolved design decision or expands beyond its stated
scope, note the evidence, change it to planned, and prepare the reviewable spec.
Bounded implementation choices already covered by the task do not force escalation.

Update `skills/tasks/SKILL.md`, `skills/scope/SKILL.md`, and
`skills/curate/SKILL.md`, this repo's `AGENTS.md`, README, and the base tasks design
reference together. Preserve the two written-artifact review gates. Attachments
are pointers to inspect, never proof of review or completion. Keep existing phase
output as a resume hint, and document that it does not authorize the next phase.

Existing records remain readable and unchanged until explicitly assessed. No bulk
backfill and no historical-task rewrite are part of this feature. Correct this
spec's status in the implementation's landing change.

## Verification and acceptance

Use the existing tests in `tests/cli.rs` and record/model tests, without new tooling:

- Set, edit, clear, and round-trip both values; missing survives unrelated writes;
  invalid values and conflicting flags fail without modifying a task.
- Ready, next (ready and parked paths), prime, show, and parked JSON expose the
  same stored choice; pretty output identifies it. Existing picker ordering and
  eligibility remain unchanged, including when process is missing.
- Check warns for unassessed todo/doing records and stays quiet for assessed,
  idea, shelved, and closed records. Completion offers the two values.
- Review the protocol against three examples: a direct small fix still gets a
  worktree; a planned change reaches both document reviews; an unassessed task
  gets an explicit decision before implementation. An existing plan link alone
  neither assigns process nor proves its approval.

Run `just gate` and reinstall with `cargo install --path .` after CLI changes.
The current deliverable is this draft; implementation planning follows its review.
